use anyhow::{Context, Result};
use ocr_merge::{parse_vtt, serialize_vtt};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const GENERATE_PROMPT: &str = "<image>\nFree OCR.";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum CaptionMode {
    #[default]
    PreferDeepseek,
    AuditOnly,
}

#[derive(Debug)]
struct CropMetadata {
    cue_index: usize,
    start: String,
    end: String,
    confidence: Option<f32>,
    path: PathBuf,
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'static str,
    images: Vec<String>,
    stream: bool,
    keep_alive: &'static str,
    options: GenerateOptions,
}

#[derive(Serialize)]
struct WarmupRequest<'a> {
    model: &'a str,
    keep_alive: &'static str,
}

#[derive(Serialize)]
struct GenerateOptions {
    temperature: f32,
    num_predict: u32,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    name: String,
}

#[derive(Debug)]
pub struct FallbackSummary {
    pub subtitle_updates: usize,
    pub caption_updates: usize,
    pub fallback_kept: usize,
}

pub struct FallbackOptions<'a> {
    pub base_url: &'a str,
    pub model: &'a str,
    pub max_sub_cues: usize,
    pub caption_mode: CaptionMode,
    pub subtitle_threshold: f32,
}

pub struct FallbackPaths<'a> {
    pub crops_dir: &'a Path,
    pub raw_subtitle_path: &'a Path,
    pub raw_caption_path: &'a Path,
    pub corrected_subtitle_path: &'a Path,
    pub corrected_caption_path: &'a Path,
    pub compare_path: &'a Path,
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        encoded.push(TABLE[(a >> 2) as usize] as char);
        encoded.push(TABLE[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

fn normalize_text(text: &str, caption: bool) -> Option<String> {
    let mut normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty()
        || normalized.chars().any(|ch| {
            !(ch.is_alphanumeric()
                || ch.is_whitespace()
                || ('\u{3400}'..='\u{9fff}').contains(&ch)
                || ('\u{f900}'..='\u{faff}').contains(&ch)
                || "《》「」『』，。！？；：、,.!?;:…-—()（）[]【】'\"“”".contains(ch))
        })
    {
        return None;
    }
    if caption {
        let content = normalized
            .strip_prefix('《')
            .and_then(|value| value.strip_suffix('》'))
            .unwrap_or(&normalized)
            .trim_start_matches('《')
            .trim_end_matches('》');
        if content.is_empty() {
            return None;
        }
        normalized = format!("《{}》", content.trim());
    }
    Some(normalized)
}

fn request_ocr(
    client: &DeepseekClient,
    base_url: &str,
    model: &str,
    image_path: &Path,
) -> Result<String> {
    let image = fs::read(image_path)
        .with_context(|| format!("Không thể đọc fallback crop {:?}", image_path))?;
    let request = GenerateRequest {
        model,
        prompt: GENERATE_PROMPT,
        images: vec![encode_base64(&image)],
        stream: false,
        keep_alive: "15m",
        options: GenerateOptions {
            temperature: 0.0,
            num_predict: 64,
        },
    };
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let request_body = serde_json::to_string(&request)?;
    let mut response = client
        .agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send(request_body)
        .context("Ollama /api/generate request failed")?;
    let response_body = response
        .body_mut()
        .read_to_string()
        .context("Ollama response body could not be read")?;
    let body: GenerateResponse =
        serde_json::from_str(&response_body).context("Ollama returned invalid JSON")?;
    Ok(body.response)
}

fn load_crop_metadata(path: &Path) -> Result<Vec<CropMetadata>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Không thể đọc metadata fallback {:?}", path))?;
    content
        .lines()
        .skip(1)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let columns = line.split('\t').collect::<Vec<_>>();
            if columns.len() != 5 && columns.len() != 6 {
                anyhow::bail!("Dòng metadata fallback không hợp lệ: {line}");
            }
            let path_column = columns.len() - 1;
            Ok(CropMetadata {
                cue_index: columns[0].parse().context("cue_index không hợp lệ")?,
                start: columns[1].to_string(),
                end: columns[2].to_string(),
                confidence: if columns[3].is_empty() {
                    None
                } else {
                    Some(columns[3].parse().context("confidence không hợp lệ")?)
                },
                path: if columns[path_column].is_empty() {
                    PathBuf::new()
                } else {
                    path.parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(columns[path_column])
                },
            })
        })
        .collect()
}

fn clean_tsv(value: &str) -> String {
    value.replace(['\t', '\r', '\n'], " ")
}

pub fn run_fallback(
    client: Option<&DeepseekClient>,
    options: &FallbackOptions<'_>,
    paths: &FallbackPaths<'_>,
) -> Result<FallbackSummary> {
    let mut subtitles = parse_vtt(&fs::read_to_string(paths.raw_subtitle_path)?, false);
    let mut captions = parse_vtt(&fs::read_to_string(paths.raw_caption_path)?, true);
    let raw_captions = captions.clone();

    let mut subtitle_crops = load_crop_metadata(&paths.crops_dir.join("subtitles.tsv"))?;
    subtitle_crops.retain(|crop| {
        crop.confidence.unwrap_or(0.0) < options.subtitle_threshold && crop.path.is_file()
    });
    subtitle_crops.sort_by(|a, b| {
        a.confidence
            .unwrap_or(0.0)
            .total_cmp(&b.confidence.unwrap_or(0.0))
    });
    subtitle_crops.truncate(options.max_sub_cues);

    let mut caption_crops = load_crop_metadata(&paths.crops_dir.join("captions.tsv"))?;
    caption_crops.sort_by_key(|crop| crop.cue_index);

    let mut subtitle_updates = 0;
    let mut fallback_kept = 0;
    for crop in subtitle_crops {
        let Some(client) = client else {
            fallback_kept += 1;
            continue;
        };
        let Some(cue) = crop
            .cue_index
            .checked_sub(1)
            .and_then(|i| subtitles.get_mut(i))
        else {
            continue;
        };
        let text = match request_ocr(client, options.base_url, options.model, &crop.path) {
            Ok(text) => text,
            Err(error) => {
                eprintln!(
                    "[DEEPSEEK: WARN] Subtitle cue {} giữ PP-OCRv6: {error:#}",
                    crop.cue_index
                );
                fallback_kept += 1;
                continue;
            }
        };
        match normalize_text(&text, false) {
            Some(text) if text != cue.text => {
                cue.text = text;
                subtitle_updates += 1;
            }
            Some(_) => {}
            None => {
                eprintln!(
                    "[DEEPSEEK: WARN] Subtitle cue {} trả về text rỗng/rác; giữ PP-OCRv6",
                    crop.cue_index
                );
                fallback_kept += 1;
            }
        }
    }

    let mut compare =
        String::from("cue_index\tstart\tend\tppocr_text\tppocr_conf\tdeepseek_text\tstatus\n");
    let mut caption_updates = 0;
    for crop in caption_crops {
        let Some(index) = crop.cue_index.checked_sub(1) else {
            continue;
        };
        let Some(raw_cue) = raw_captions.get(index) else {
            continue;
        };
        let Some(cue) = captions.get_mut(index) else {
            continue;
        };
        let normalized_text = match client {
            Some(client) => {
                match request_ocr(client, options.base_url, options.model, &crop.path) {
                    Ok(text) => match normalize_text(&text, true) {
                        Some(text) => Some(text),
                        None => {
                            eprintln!(
                                "[DEEPSEEK: WARN] Caption cue {} trả về text rỗng/rác; giữ PP-OCRv6",
                                crop.cue_index
                            );
                            None
                        }
                    },
                    Err(error) => {
                        eprintln!(
                            "[DEEPSEEK: WARN] Caption cue {} giữ PP-OCRv6: {error:#}",
                            crop.cue_index
                        );
                        None
                    }
                }
            }
            None => None,
        };
        let (deepseek_text, status) = match normalized_text {
            Some(text) if text == raw_cue.text => (text, "matched"),
            Some(text) if options.caption_mode == CaptionMode::AuditOnly => (text, "audit_diff"),
            Some(text) => {
                cue.text = text.clone();
                caption_updates += 1;
                (text, "updated")
            }
            None => {
                fallback_kept += 1;
                (String::new(), "fallback_kept")
            }
        };
        compare.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            crop.cue_index,
            crop.start,
            crop.end,
            clean_tsv(&raw_cue.text),
            crop.confidence
                .map(|confidence| format!("{confidence:.4}"))
                .unwrap_or_default(),
            clean_tsv(&deepseek_text),
            status
        ));
    }

    fs::write(paths.compare_path, compare)?;
    fs::write(paths.corrected_subtitle_path, serialize_vtt(&subtitles))?;
    fs::write(paths.corrected_caption_path, serialize_vtt(&captions))?;
    Ok(FallbackSummary {
        subtitle_updates,
        caption_updates,
        fallback_kept,
    })
}

pub struct DeepseekClient {
    agent: ureq::Agent,
}

impl DeepseekClient {
    pub fn new(timeout: Duration) -> Self {
        Self {
            agent: create_agent(timeout),
        }
    }
}

fn create_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

fn warm_up(agent: &ureq::Agent, base_url: &str, model: &str) -> Result<()> {
    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
    let body = serde_json::to_string(&WarmupRequest {
        model,
        keep_alive: "15m",
    })?;
    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send(body)
        .context("Ollama model warm-up request failed")?;
    let body = response
        .body_mut()
        .read_to_string()
        .context("Ollama warm-up response could not be read")?;
    let _: serde_json::Value =
        serde_json::from_str(&body).context("Ollama returned invalid warm-up JSON")?;
    Ok(())
}

fn create_startup_agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

fn get_tags(agent: &ureq::Agent, base_url: &str) -> Result<TagsResponse> {
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let mut response = agent.get(&url).call().context("Ollama /api/tags failed")?;
    let body = response
        .body_mut()
        .read_to_string()
        .context("Ollama /api/tags response could not be read")?;
    serde_json::from_str(&body).context("Ollama returned invalid /api/tags JSON")
}

fn model_is_available(models: &[ModelEntry], requested: &str) -> bool {
    models.iter().any(|entry| {
        entry.name == requested
            || (!requested.contains(':') && entry.name == format!("{requested}:latest"))
    })
}

fn is_local_url(base_url: &str) -> bool {
    let Some((scheme, remainder)) = base_url.split_once("://") else {
        return false;
    };
    if scheme != "http" && scheme != "https" {
        return false;
    }
    let authority = remainder.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = host_port.split(':').next().unwrap_or_default();
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1"
}

fn should_start_local_server(base_url: &str, auto_start: bool) -> bool {
    auto_start && is_local_url(base_url)
}

fn find_ollama_binary_with(
    project_root: &Path,
    configured_binary: Option<&Path>,
    local_app_data: Option<&Path>,
    path_directories: &[PathBuf],
) -> Option<PathBuf> {
    #[cfg(windows)]
    let executable = ["ollama.exe", "ollama"];
    #[cfg(not(windows))]
    let executable = ["ollama", "ollama"];

    let project_binary = project_root
        .join("tools")
        .join("ollama")
        .join(executable[0]);
    if project_binary.is_file() {
        return Some(project_binary);
    }
    if let Some(binary) = configured_binary
        && binary.is_file()
    {
        return Some(binary.to_path_buf());
    }
    #[cfg(windows)]
    if let Some(local_app_data) = local_app_data {
        let candidate = local_app_data
            .join("Programs")
            .join("Ollama")
            .join("ollama.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    for directory in path_directories {
        for name in executable {
            let candidate = directory.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn find_ollama_binary(project_root: &Path) -> Option<PathBuf> {
    let configured_binary = std::env::var_os("OLLAMA_BIN").map(PathBuf::from);
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let path_directories =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    find_ollama_binary_with(
        project_root,
        configured_binary.as_deref(),
        local_app_data.as_deref(),
        &path_directories,
    )
}

fn start_ollama(project_root: &Path) -> Result<()> {
    let binary = find_ollama_binary(project_root).context(
        "ollama executable was not found in tools/ollama, OLLAMA_BIN, LOCALAPPDATA, or PATH",
    )?;
    let model_directory = project_root.join("models").join("deepseek-ocr");
    fs::create_dir_all(&model_directory)
        .with_context(|| format!("could not create model directory {model_directory:?}"))?;
    let model_directory = model_directory
        .canonicalize()
        .with_context(|| format!("could not resolve model directory {model_directory:?}"))?;
    let mut command = Command::new(binary);
    command
        .arg("serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env("OLLAMA_MODELS", model_directory);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    command.spawn().context("could not start `ollama serve`")?;
    Ok(())
}

fn prepare_ollama_with<F>(
    probe_agent: &ureq::Agent,
    pull_agent: &ureq::Agent,
    base_url: &str,
    model: &str,
    auto_start: bool,
    start_server: F,
) -> bool
where
    F: FnOnce() -> Result<()>,
{
    let tags = match get_tags(probe_agent, base_url) {
        Ok(tags) => tags,
        Err(initial_error) => {
            if !should_start_local_server(base_url, auto_start) {
                eprintln!(
                    "[DEEPSEEK: WARN] Ollama is unavailable at {base_url}; keeping PP-OCRv6 results ({initial_error:#})."
                );
                return false;
            }
            if let Err(error) = start_server() {
                eprintln!(
                    "[DEEPSEEK: WARN] Could not start Ollama; keeping PP-OCRv6 results ({error:#})."
                );
                return false;
            }

            let deadline = Instant::now() + Duration::from_secs(15);
            let poll_agent = create_startup_agent(Duration::from_millis(500));
            let mut ready_tags = None;
            while Instant::now() < deadline {
                match get_tags(&poll_agent, base_url) {
                    Ok(tags) => {
                        ready_tags = Some(tags);
                        break;
                    }
                    Err(_) => thread::sleep(Duration::from_millis(500)),
                }
            }
            let Some(tags) = ready_tags else {
                eprintln!(
                    "[DEEPSEEK: WARN] Ollama did not become ready within 15s; keeping PP-OCRv6 results."
                );
                return false;
            };
            tags
        }
    };

    if model_is_available(&tags.models, model) {
        println!("[DEEPSEEK] Ollama model '{model}' is already available.");
        return true;
    }
    match pull_model(pull_agent, base_url, model) {
        Ok(()) => {
            println!("[DEEPSEEK] Ollama model '{model}' is ready.");
            true
        }
        Err(error) => {
            eprintln!(
                "[DEEPSEEK: WARN] Could not prepare Ollama model '{model}'; keeping PP-OCRv6 results ({error:#})."
            );
            false
        }
    }
}

fn pull_model(agent: &ureq::Agent, base_url: &str, model: &str) -> Result<()> {
    let url = format!("{}/api/pull", base_url.trim_end_matches('/'));
    let body = serde_json::json!({ "name": model, "stream": false }).to_string();
    let mut response = agent
        .post(&url)
        .header("Content-Type", "application/json")
        .send(body)
        .context("Ollama /api/pull failed")?;
    let response_body = response
        .body_mut()
        .read_to_string()
        .context("Ollama /api/pull response could not be read")?;
    let result: serde_json::Value =
        serde_json::from_str(&response_body).context("Ollama returned invalid /api/pull JSON")?;
    if result.get("error").is_some()
        || result.get("status").and_then(serde_json::Value::as_str) != Some("success")
    {
        anyhow::bail!("Ollama model pull did not complete successfully: {response_body}");
    }
    Ok(())
}

pub fn prepare_ollama(base_url: &str, model: &str, auto_start: bool, project_root: &Path) -> bool {
    let probe_agent = create_startup_agent(Duration::from_secs(2));
    let pull_agent = create_startup_agent(Duration::from_secs(30 * 60));
    let model_ready = prepare_ollama_with(
        &probe_agent,
        &pull_agent,
        base_url,
        model,
        auto_start,
        || start_ollama(project_root),
    );
    if !model_ready {
        return false;
    }
    let warmup_agent = create_startup_agent(Duration::from_secs(60));
    match warm_up(&warmup_agent, base_url, model) {
        Ok(()) => true,
        Err(error) => {
            eprintln!(
                "[DEEPSEEK: WARN] Ollama model warm-up failed; keeping PP-OCRv6 results ({error:#})."
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    struct TestTempDir(PathBuf);

    impl TestTempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ocr-deepseek-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn agent_with_timeout(timeout: Duration) -> ureq::Agent {
        ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .build()
            .into()
    }

    fn client_with_timeout(timeout: Duration) -> DeepseekClient {
        DeepseekClient::new(timeout)
    }

    fn mock_ollama_server(
        responses: Vec<(String, String)>,
    ) -> (String, thread::JoinHandle<Vec<(String, serde_json::Value)>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for (method_path, response_body) in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                        continue;
                    };
                    let header = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
                let header_end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
                let header = String::from_utf8_lossy(&request[..header_end]);
                let request_line = header.lines().next().unwrap_or_default();
                assert!(request_line.starts_with(&method_path));
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let body = &request[header_end + 4..header_end + 4 + content_length];
                let body = if body.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::from_slice(body).unwrap()
                };
                requests.push((request_line.to_string(), body));
                let response_bytes = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response_bytes.as_bytes());
            }
            requests
        });
        (address, handle)
    }

    fn mock_server(
        responses: Vec<String>,
        delay: Duration,
    ) -> (String, thread::JoinHandle<Vec<serde_json::Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    break;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    let Some(header_end) = request.windows(4).position(|w| w == b"\r\n\r\n") else {
                        continue;
                    };
                    let header = String::from_utf8_lossy(&request[..header_end]);
                    let content_length = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if request.len() >= header_end + 4 + content_length {
                        break;
                    }
                }
                let header_end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
                let header = String::from_utf8_lossy(&request[..header_end]);
                let request_target = header
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or_default()
                    .to_string();
                let content_length = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                let body = &request[header_end + 4..header_end + 4 + content_length];
                let mut request_value: serde_json::Value = serde_json::from_slice(body).unwrap();
                request_value["_request_target"] = request_target.into();
                requests.push(request_value);
                thread::sleep(delay);
                let body = serde_json::json!({ "response": response }).to_string();
                let response_bytes = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response_bytes.as_bytes());
            }
            requests
        });
        (address, handle)
    }

    fn fixture() -> (
        TestTempDir,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
        PathBuf,
    ) {
        let temp = TestTempDir::new();
        let crops = temp.path().join("fallback_crops");
        fs::create_dir_all(&crops).unwrap();
        let subtitles = temp.path().join("subtitles.vtt");
        let captions = temp.path().join("captions.vtt");
        let corrected_subtitles = temp.path().join("subtitles.deepseek.vtt");
        let corrected_captions = temp.path().join("captions.deepseek.vtt");
        let compare = temp.path().join("caption_deepseek_compare.tsv");
        (
            temp,
            crops,
            subtitles,
            captions,
            corrected_subtitles,
            corrected_captions,
            compare,
        )
    }

    #[test]
    fn warmup_uses_model_and_keep_alive_without_image_work() {
        let (url, server) = mock_ollama_server(vec![(
            "POST /api/generate".into(),
            r#"{"response":"","done":true}"#.into(),
        )]);
        let agent = agent_with_timeout(Duration::from_secs(2));
        warm_up(&agent, &url, "deepseek-ocr").unwrap();

        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "POST /api/generate HTTP/1.1");
        assert_eq!(requests[0].1["model"], "deepseek-ocr");
        assert_eq!(requests[0].1["keep_alive"], "15m");
        assert_eq!(requests[0].1.as_object().unwrap().len(), 2);
    }

    #[test]
    fn ollama_binary_search_obeys_portable_then_environment_then_default_then_path() {
        let temp = TestTempDir::new();
        let project_root = temp.path().join("project");
        let local_app_data = temp.path().join("local-app-data");
        let path_directory = temp.path().join("path");
        let executable = if cfg!(windows) {
            "ollama.exe"
        } else {
            "ollama"
        };
        let project_binary = project_root.join("tools").join("ollama").join(executable);
        let configured_binary = temp.path().join("configured").join(executable);
        let default_binary = local_app_data
            .join("Programs")
            .join("Ollama")
            .join("ollama.exe");
        let path_binary = path_directory.join(executable);
        for binary in [
            &project_binary,
            &configured_binary,
            &default_binary,
            &path_binary,
        ] {
            fs::create_dir_all(binary.parent().unwrap()).unwrap();
            fs::write(binary, b"test binary").unwrap();
        }

        let path_directories = vec![path_directory];
        assert_eq!(
            find_ollama_binary_with(
                &project_root,
                Some(&configured_binary),
                Some(&local_app_data),
                &path_directories,
            ),
            Some(project_binary.clone())
        );
        fs::remove_file(&project_binary).unwrap();
        assert_eq!(
            find_ollama_binary_with(
                &project_root,
                Some(&configured_binary),
                Some(&local_app_data),
                &path_directories,
            ),
            Some(configured_binary.clone())
        );
        fs::remove_file(&configured_binary).unwrap();
        #[cfg(windows)]
        assert_eq!(
            find_ollama_binary_with(
                &project_root,
                None,
                Some(&local_app_data),
                &path_directories,
            ),
            Some(default_binary.clone())
        );
        #[cfg(windows)]
        fs::remove_file(&default_binary).unwrap();
        assert_eq!(
            find_ollama_binary_with(&project_root, None, None, &path_directories),
            Some(path_binary)
        );
    }

    #[test]
    fn subtitle_fallback_uses_weakest_cue_and_respects_maximum() {
        let (temp, crops, subtitles, captions, corrected_subs, corrected_caps, compare) = fixture();
        let mut vtt = String::from("WEBVTT\n\n");
        for (index, text) in ["one", "two", "three"].iter().enumerate() {
            vtt.push_str(&format!(
                "{}\n00:00:0{}.000 --> 00:00:0{}.500\n{text}\n\n",
                index + 1,
                index + 1,
                index + 1
            ));
            fs::write(crops.join(format!("sub_{:03}.png", index + 1)), b"crop").unwrap();
        }
        fs::write(&subtitles, vtt).unwrap();
        fs::write(&captions, "WEBVTT\n\n").unwrap();
        fs::write(
            crops.join("subtitles.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n1\t1.0\t1.5\t0.60\tsub_001.png\n2\t2.0\t2.5\t0.30\tsub_002.png\n3\t3.0\t3.5\t0.70\tsub_003.png\n",
        )
        .unwrap();
        fs::write(
            crops.join("captions.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n",
        )
        .unwrap();
        let (url, server) = mock_server(vec!["corrected".into()], Duration::ZERO);

        let summary = run_fallback(
            Some(&client_with_timeout(Duration::from_secs(2))),
            &FallbackOptions {
                base_url: &url,
                model: "deepseek-ocr",
                max_sub_cues: 1,
                caption_mode: CaptionMode::PreferDeepseek,
                subtitle_threshold: 0.80,
            },
            &FallbackPaths {
                crops_dir: &crops,
                raw_subtitle_path: &subtitles,
                raw_caption_path: &captions,
                corrected_subtitle_path: &corrected_subs,
                corrected_caption_path: &corrected_caps,
                compare_path: &compare,
            },
        )
        .unwrap();

        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["_request_target"], "/api/generate");
        assert_eq!(requests[0]["prompt"], GENERATE_PROMPT);
        assert_eq!(requests[0]["model"], "deepseek-ocr");
        assert_eq!(requests[0]["stream"], false);
        assert_eq!(requests[0]["keep_alive"], "15m");
        assert!(!requests[0]["images"][0].as_str().unwrap().is_empty());
        assert_eq!(requests[0]["options"]["temperature"], 0.0);
        assert_eq!(requests[0]["options"]["num_predict"], 64);
        assert_eq!(summary.subtitle_updates, 1);
        let corrected = parse_vtt(&fs::read_to_string(corrected_subs).unwrap(), false);
        assert_eq!(corrected[0].text, "one");
        assert_eq!(corrected[1].text, "corrected");
        assert_eq!(corrected[2].text, "three");
        drop(temp);
    }

    #[test]
    fn caption_modes_write_parallel_comparison_and_preserve_raw_vtt() {
        for (mode, expected_status, expected_text) in [
            (CaptionMode::PreferDeepseek, "updated", "《Deep》"),
            (CaptionMode::AuditOnly, "audit_diff", "《Original》"),
        ] {
            let (temp, crops, subtitles, captions, corrected_subs, corrected_caps, compare) =
                fixture();
            fs::write(&subtitles, "WEBVTT\n\n").unwrap();
            let raw = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000 line:20%\n《Original》\n\n";
            fs::write(&captions, raw).unwrap();
            fs::write(
                crops.join("captions.tsv"),
                "cue_index\tstart\tend\tconfidence\tpath\n1\t1.0\t2.0\t0.92\tcap_001.png\n",
            )
            .unwrap();
            fs::write(crops.join("cap_001.png"), b"crop").unwrap();
            fs::write(
                crops.join("subtitles.tsv"),
                "cue_index\tstart\tend\tconfidence\tpath\n",
            )
            .unwrap();
            let (url, server) = mock_server(vec!["  Deep\n ".into()], Duration::ZERO);

            run_fallback(
                Some(&client_with_timeout(Duration::from_secs(2))),
                &FallbackOptions {
                    base_url: &url,
                    model: "deepseek-ocr",
                    max_sub_cues: 4,
                    caption_mode: mode,
                    subtitle_threshold: 0.80,
                },
                &FallbackPaths {
                    crops_dir: &crops,
                    raw_subtitle_path: &subtitles,
                    raw_caption_path: &captions,
                    corrected_subtitle_path: &corrected_subs,
                    corrected_caption_path: &corrected_caps,
                    compare_path: &compare,
                },
            )
            .unwrap();

            assert_eq!(server.join().unwrap().len(), 1);
            assert_eq!(fs::read_to_string(&captions).unwrap(), raw);
            let corrected = parse_vtt(&fs::read_to_string(corrected_caps).unwrap(), true);
            assert_eq!(corrected[0].text, expected_text);
            let compare = fs::read_to_string(compare).unwrap();
            assert!(compare.contains(&format!("\t{expected_status}\n")));
            assert!(compare.contains("《Original》"));
            drop(temp);
        }
    }

    #[test]
    fn normalization_collapses_whitespace_and_preserves_caption_brackets() {
        assert_eq!(
            normalize_text("  《名字  \n  角色》 ", true).as_deref(),
            Some("《名字 角色》")
        );
        assert_eq!(normalize_text(" \n ", false), None);
        assert_eq!(normalize_text("名字\u{fffd}", false), None);
    }

    #[test]
    fn caption_identical_result_is_marked_matched() {
        let (temp, crops, subtitles, captions, corrected_subs, corrected_caps, compare) = fixture();
        fs::write(&subtitles, "WEBVTT\n\n").unwrap();
        fs::write(
            &captions,
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n《Original》\n\n",
        )
        .unwrap();
        fs::write(
            crops.join("captions.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n1\t00:00:01.000\t00:00:02.000\t0.92\tcap_001.png\n",
        )
        .unwrap();
        fs::write(crops.join("cap_001.png"), b"crop").unwrap();
        fs::write(
            crops.join("subtitles.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n",
        )
        .unwrap();
        let (url, server) = mock_server(vec!["Original".into()], Duration::ZERO);

        run_fallback(
            Some(&client_with_timeout(Duration::from_secs(2))),
            &FallbackOptions {
                base_url: &url,
                model: "deepseek-ocr",
                max_sub_cues: 4,
                caption_mode: CaptionMode::PreferDeepseek,
                subtitle_threshold: 0.80,
            },
            &FallbackPaths {
                crops_dir: &crops,
                raw_subtitle_path: &subtitles,
                raw_caption_path: &captions,
                corrected_subtitle_path: &corrected_subs,
                corrected_caption_path: &corrected_caps,
                compare_path: &compare,
            },
        )
        .unwrap();

        assert_eq!(server.join().unwrap().len(), 1);
        assert!(fs::read_to_string(compare).unwrap().contains("\tmatched\n"));
        drop(temp);
    }

    #[test]
    fn stopped_server_keeps_caption_text_and_marks_fallback() {
        let (temp, crops, subtitles, captions, corrected_subs, corrected_caps, compare) = fixture();
        fs::write(&subtitles, "WEBVTT\n\n").unwrap();
        fs::write(
            &captions,
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n《Original》\n\n",
        )
        .unwrap();
        fs::write(
            crops.join("captions.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n1\t1.0\t2.0\t0.92\tcap_001.png\n",
        )
        .unwrap();
        fs::write(crops.join("cap_001.png"), b"crop").unwrap();
        fs::write(
            crops.join("subtitles.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n",
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        run_fallback(
            Some(&client_with_timeout(Duration::from_millis(100))),
            &FallbackOptions {
                base_url: &url,
                model: "deepseek-ocr",
                max_sub_cues: 4,
                caption_mode: CaptionMode::PreferDeepseek,
                subtitle_threshold: 0.80,
            },
            &FallbackPaths {
                crops_dir: &crops,
                raw_subtitle_path: &subtitles,
                raw_caption_path: &captions,
                corrected_subtitle_path: &corrected_subs,
                corrected_caption_path: &corrected_caps,
                compare_path: &compare,
            },
        )
        .unwrap();

        assert!(
            fs::read_to_string(compare)
                .unwrap()
                .contains("\tfallback_kept\n")
        );
        assert_eq!(
            parse_vtt(&fs::read_to_string(corrected_caps).unwrap(), true)[0].text,
            "《Original》"
        );
        drop(temp);
    }

    #[test]
    fn timed_out_server_keeps_caption_text_and_marks_fallback() {
        let (temp, crops, subtitles, captions, corrected_subs, corrected_caps, compare) = fixture();
        fs::write(&subtitles, "WEBVTT\n\n").unwrap();
        fs::write(
            &captions,
            "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\n《Original》\n\n",
        )
        .unwrap();
        fs::write(
            crops.join("captions.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n1\t1.0\t2.0\t0.92\tcap_001.png\n",
        )
        .unwrap();
        fs::write(crops.join("cap_001.png"), b"crop").unwrap();
        fs::write(
            crops.join("subtitles.tsv"),
            "cue_index\tstart\tend\tconfidence\tpath\n",
        )
        .unwrap();
        let (url, server) = mock_server(vec!["《Too late》".into()], Duration::from_millis(200));

        run_fallback(
            Some(&client_with_timeout(Duration::from_millis(30))),
            &FallbackOptions {
                base_url: &url,
                model: "deepseek-ocr",
                max_sub_cues: 4,
                caption_mode: CaptionMode::PreferDeepseek,
                subtitle_threshold: 0.80,
            },
            &FallbackPaths {
                crops_dir: &crops,
                raw_subtitle_path: &subtitles,
                raw_caption_path: &captions,
                corrected_subtitle_path: &corrected_subs,
                corrected_caption_path: &corrected_caps,
                compare_path: &compare,
            },
        )
        .unwrap();

        assert!(
            fs::read_to_string(compare)
                .unwrap()
                .contains("\tfallback_kept\n")
        );
        assert_eq!(
            parse_vtt(&fs::read_to_string(corrected_caps).unwrap(), true)[0].text,
            "《Original》"
        );
        let _ = server.join();
        drop(temp);
    }

    #[test]
    fn detects_local_ollama_urls_without_treating_remote_urls_as_local() {
        assert!(is_local_url("http://127.0.0.1:11434"));
        assert!(is_local_url("http://localhost:11434"));
        assert!(!is_local_url("http://192.168.1.10:11434"));
        assert!(!is_local_url("https://ollama.example:11434"));
        assert!(should_start_local_server("http://127.0.0.1:11434", true));
        assert!(!should_start_local_server(
            "http://ollama.example:11434",
            true
        ));
        assert!(!should_start_local_server("http://127.0.0.1:11434", false));
    }

    #[test]
    fn existing_model_skips_pull_and_remote_urls_never_start_local_server() {
        let (url, server) = mock_ollama_server(vec![(
            "GET /api/tags".into(),
            r#"{"models":[{"name":"deepseek-ocr:latest"}]}"#.into(),
        )]);
        let probe_agent = agent_with_timeout(Duration::from_secs(2));
        let pull_agent = agent_with_timeout(Duration::from_secs(2));
        let spawn_calls = std::sync::atomic::AtomicUsize::new(0);
        assert!(prepare_ollama_with(
            &probe_agent,
            &pull_agent,
            &url,
            "deepseek-ocr",
            true,
            || {
                spawn_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(spawn_calls.load(std::sync::atomic::Ordering::SeqCst), 0);

        assert!(!should_start_local_server(
            "http://ollama.example:11434",
            true
        ));
    }

    #[test]
    fn missing_model_is_pulled_with_non_streaming_request() {
        let (url, server) = mock_ollama_server(vec![
            ("GET /api/tags".into(), r#"{"models":[]}"#.into()),
            ("POST /api/pull".into(), r#"{"status":"success"}"#.into()),
        ]);
        let probe_agent = agent_with_timeout(Duration::from_secs(2));
        let pull_agent = agent_with_timeout(Duration::from_secs(2));
        assert!(prepare_ollama_with(
            &probe_agent,
            &pull_agent,
            &url,
            "deepseek-ocr",
            false,
            || panic!("existing local server must not be spawned"),
        ));
        let requests = server.join().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].0, "POST /api/pull HTTP/1.1");
        assert_eq!(requests[1].1["name"], "deepseek-ocr");
        assert_eq!(requests[1].1["stream"], false);
    }

    #[test]
    fn unavailable_server_falls_back_without_spawning_when_auto_start_is_disabled() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let probe_agent = agent_with_timeout(Duration::from_millis(50));
        let pull_agent = agent_with_timeout(Duration::from_millis(50));
        let spawn_calls = std::sync::atomic::AtomicUsize::new(0);
        assert!(!prepare_ollama_with(
            &probe_agent,
            &pull_agent,
            &url,
            "deepseek-ocr",
            false,
            || {
                spawn_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            },
        ));
        assert_eq!(spawn_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
