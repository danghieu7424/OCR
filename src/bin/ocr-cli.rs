use anyhow::{Context, Result, anyhow, bail};
use image::RgbImage;
use ocr::{
    CaptionExtractorConfig, CaptionExtractorOutput, CaptionWarning, OcrExtractorConfig,
    SubtitleRoi, SubtitleSegment, extract_captions, extract_hardcoded_subtitles,
    merge_vtt_files_with_line_overrides,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::UNIX_EPOCH;

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "ts", "flv", "webm"];
const STATE_VERSION: u32 = 1;
const ANSI_RESET: &str = "\x1b[0m";
const ANSI_BLUE: &str = "\x1b[94m";
const ANSI_GREEN: &str = "\x1b[92m";
const ANSI_ORANGE: &str = "\x1b[38;5;208m";
const ANSI_RED: &str = "\x1b[91m";
const ANSI_CYAN: &str = "\x1b[96m";
const ANSI_DIM: &str = "\x1b[90m";

struct CliArgs {
    input: PathBuf,
    output: PathBuf,
    models: PathBuf,
    caption_evidence: bool,
    subtitle_line: Option<String>,
    caption_line: Option<String>,
    subtitle_roi: SubtitleRoi,
    caption_roi: (f32, f32, f32, f32),
    force: bool,
    attribute_speakers: bool,
    gallery: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum PhaseStatus {
    #[default]
    Pending,
    Running,
    Complete,
    Warning,
    Error,
}

#[derive(Default)]
struct ProgressState {
    subtitle_percent: f32,
    caption_percent: f32,
    elapsed: String,
    subtitle_status: PhaseStatus,
    caption_status: PhaseStatus,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StageCheckpoint {
    signature: String,
    completed: bool,
    warnings: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunCheckpoint {
    version: u32,
    subtitle: StageCheckpoint,
    caption: StageCheckpoint,
    merge: StageCheckpoint,
}

impl Default for RunCheckpoint {
    fn default() -> Self {
        Self {
            version: STATE_VERSION,
            subtitle: StageCheckpoint::default(),
            caption: StageCheckpoint::default(),
            merge: StageCheckpoint::default(),
        }
    }
}

#[derive(Clone, Serialize)]
struct FileFingerprint {
    path: String,
    size: u64,
    modified_unix_nanos: u128,
}

#[derive(Serialize)]
struct SubtitleSignature {
    video: FileFingerprint,
    detector_model: FileFingerprint,
    recognition_model: FileFingerprint,
    dictionary: FileFingerprint,
    roi: [f32; 4],
    fps: u32,
    accuracy_mode: bool,
    evidence: bool,
}

#[derive(Serialize)]
struct CaptionSignature {
    video: FileFingerprint,
    detector_model: FileFingerprint,
    recognition_model: FileFingerprint,
    dictionary: FileFingerprint,
    roi: [f32; 4],
    fps: u32,
    vertical_aspect_threshold: f32,
    column_x_tolerance: f32,
    min_confidence: f32,
    min_duration_sec: f64,
    watermark_freq_threshold: f32,
    evidence: bool,
    evidence_warning_confidence: f32,
    evidence_duration_margin_sec: f64,
}

#[derive(Serialize)]
struct MergeSignature<'a> {
    subtitle: &'a str,
    caption: &'a str,
    subtitle_line: Option<&'a str>,
    caption_line: Option<&'a str>,
}

fn register_progress_handler() -> Arc<Mutex<ProgressState>> {
    let state = Arc::new(Mutex::new(ProgressState::default()));
    let callback_state = Arc::clone(&state);
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_width = 12usize;

    ocr::on_progress(move |progress| {
        let total = progress.total.max(1);
        let current = progress.current.min(total);
        let percent = (current as f32 / total as f32 * 100.0).clamp(0.0, 99.9);

        if let Ok(mut state) = callback_state.lock() {
            if progress.title.contains("Caption") {
                state.caption_percent = percent;
            } else {
                state.subtitle_percent = percent;
            }
            state.elapsed.clone_from(&progress.elapsed);
            render_progress(&state, filled_char, empty_char, bar_width);
        }
    });
    state
}

fn reset_progress(state: &Mutex<ProgressState>) {
    if let Ok(mut state) = state.lock() {
        *state = ProgressState::default();
    }
}

fn status_style(status: PhaseStatus) -> &'static str {
    match status {
        PhaseStatus::Pending => ANSI_DIM,
        PhaseStatus::Running => ANSI_CYAN,
        PhaseStatus::Complete => ANSI_GREEN,
        PhaseStatus::Warning => ANSI_ORANGE,
        PhaseStatus::Error => ANSI_RED,
    }
}

fn render_progress(state: &ProgressState, filled_char: &str, empty_char: &str, bar_width: usize) {
    let subtitle_bar = progress_bar(state.subtitle_percent, filled_char, empty_char, bar_width);
    let caption_bar = progress_bar(state.caption_percent, filled_char, empty_char, bar_width);

    print!(
        "{}",
        format_progress_line(state, &subtitle_bar, &caption_bar,)
    );
    if let Err(error) = io::stdout().flush() {
        eprintln!("\n[WARN] Không thể cập nhật thanh tiến trình: {error}");
    }
}

fn format_progress_line(state: &ProgressState, subtitle_bar: &str, caption_bar: &str) -> String {
    format!(
        "\r {}[SUB]: {}{}{} {:5.1}% | {}[CAP]: {}{}{} {:5.1}% | {}   ",
        status_style(state.subtitle_status),
        ANSI_BLUE,
        subtitle_bar,
        ANSI_RESET,
        state.subtitle_percent,
        status_style(state.caption_status),
        ANSI_BLUE,
        caption_bar,
        ANSI_RESET,
        state.caption_percent,
        state.elapsed
    )
}

fn progress_bar(percent: f32, filled_char: &str, empty_char: &str, bar_width: usize) -> String {
    let clamped_percent = percent.clamp(0.0, 100.0);
    let filled = (clamped_percent / 100.0 * bar_width as f32).floor() as usize;
    filled_char.repeat(filled) + &empty_char.repeat(bar_width - filled)
}

fn set_phase_progress(
    state: &Mutex<ProgressState>,
    subtitle: bool,
    percent: f32,
    status: PhaseStatus,
) {
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    if let Ok(mut state) = state.lock() {
        if subtitle {
            state.subtitle_percent = percent;
            state.subtitle_status = status;
        } else {
            state.caption_percent = percent;
            state.caption_status = status;
        }
        render_progress(&state, filled_char, empty_char, 12);
    }
}

fn finalize_progress(
    state: &Mutex<ProgressState>,
    subtitle_complete: bool,
    subtitle_warning: bool,
    caption_complete: bool,
    caption_warning: bool,
) {
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    if let Ok(mut state) = state.lock() {
        if subtitle_complete {
            state.subtitle_percent = 100.0;
            state.subtitle_status = if subtitle_warning {
                PhaseStatus::Warning
            } else {
                PhaseStatus::Complete
            };
        } else {
            state.subtitle_status = PhaseStatus::Error;
        }
        if caption_complete {
            state.caption_percent = 100.0;
            state.caption_status = if caption_warning {
                PhaseStatus::Warning
            } else {
                PhaseStatus::Complete
            };
        } else {
            state.caption_status = PhaseStatus::Error;
        }
        render_progress(&state, filled_char, empty_char, 12);
    }
}

fn usage() -> &'static str {
    "Video OCR extractor, WebVTT merger, and optional Light-ASD runner\n\
\n\
Usage:\n\
  ocr-cli --input <file-or-folder> --output <folder> [options]\n\
  ocr-cli -i <file-or-folder> -o <folder> [options]\n\
\n\
Options:\n\
  -i, --input <file-or-folder>   Input video file or non-recursive folder\n\
  -o, --output <folder>          Destination for merged VTT files (required)\n\
      --models <folder>          Model directory containing ocr/{det.onnx,rec.onnx,keys.txt} (default: models)\n\
      --caption-evidence         Export first/best/last evidence images for subtitles and captions\n\
      --subtitle-line <value>    Override subtitle WebVTT line setting (for example: 85% or -1)\n\
      --caption-line <value>     Override caption WebVTT line setting (for example: 20%)\n\
      --subtitle-roi <coords>    Subtitle ROI as Y_MIN,Y_MAX,X_MIN,X_MAX fractions (0..1)\n\
      --caption-roi <coords>     Caption ROI as Y_MIN,Y_MAX,X_MIN,X_MAX fractions (0..1)\n\
      --attribute-speakers       Attribute character speakers, wrap subtitles in <v Character> tags\n\
                                 and normalize names against the gallery\n\
      --gallery <path>           Voiceprint gallery JSON path (default: <input>/.character_gallery.json)\n\
      --download-gliner          Auto-download GLiNER multi v2.1 INT8 ONNX model (~350MB) to models/gliner/\n\
      --force                    Reprocess completed subtitles, captions, and merge\n\
  -h, --help                     Show this help\n\
\n\
Each video gets a sibling .tmp_<stem> directory containing subtitles.zh.vtt and\n\
captions.zh.vtt and a state.json checkpoint. Input folders are scanned non-recursively. A positional input\n\
is also accepted for backwards compatibility. Cue cache is stored in\n\
<input>/.studio_cache.ferrite (one FerriteDB per movie folder)."
}

fn parse_roi(value: &str, label: &str) -> Result<(f32, f32, f32, f32)> {
    let values: Vec<f32> = value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<f32>()
                .with_context(|| format!("Giá trị ROI không hợp lệ cho {label}: {part}"))
        })
        .collect::<Result<_>>()?;
    if values.len() != 4
        || values
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        || values[0] >= values[1]
        || values[2] >= values[3]
    {
        bail!("{label} cần dạng Y_MIN,Y_MAX,X_MIN,X_MAX; mỗi giá trị thuộc [0,1] và min < max");
    }
    Ok((values[0], values[1], values[2], values[3]))
}

fn parse_args_from(mut args: impl Iterator<Item = String>) -> Result<Option<CliArgs>> {
    let mut input = None;
    let mut output = None;
    let mut models = PathBuf::from("models");
    let mut caption_evidence = false;
    let mut subtitle_line = None;
    let mut caption_line = None;
    let mut subtitle_roi = SubtitleRoi::default();
    let mut caption_roi = (0.10, 0.80, 0.0, 1.0);
    let mut force = false;
    let mut attribute_speakers = false;
    let mut gallery = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Ok(None);
            }
            "-o" | "--output" => {
                output = Some(PathBuf::from(
                    args.next().context("--output cần đường dẫn thư mục")?,
                ));
            }
            "-i" | "--input" => {
                if input.is_some() {
                    bail!("Chỉ nhận một input; không kết hợp --input với positional input.");
                }
                input = Some(PathBuf::from(
                    args.next()
                        .context("--input cần đường dẫn file hoặc folder")?,
                ));
            }
            "--models" => {
                models = PathBuf::from(args.next().context("--models cần đường dẫn thư mục")?);
            }
            "--caption-evidence" => caption_evidence = true,
            "--force" => force = true,
            "--subtitle-line" => {
                subtitle_line = Some(args.next().context("--subtitle-line cần giá trị")?);
            }
            "--caption-line" => {
                caption_line = Some(args.next().context("--caption-line cần giá trị")?);
            }
            "--subtitle-roi" => {
                let values = parse_roi(
                    &args.next().context("--subtitle-roi cần 4 giá trị")?,
                    "--subtitle-roi",
                )?;
                subtitle_roi = SubtitleRoi {
                    y_min: values.0,
                    y_max: values.1,
                    x_min: values.2,
                    x_max: values.3,
                };
            }
            "--caption-roi" => {
                caption_roi = parse_roi(
                    &args.next().context("--caption-roi cần 4 giá trị")?,
                    "--caption-roi",
                )?;
            }
            "--attribute-speakers" => {
                attribute_speakers = true;
            }
            "--gallery" => {
                gallery = Some(PathBuf::from(
                    args.next().context("--gallery cần đường dẫn json")?,
                ));
            }
            "--download-gliner" => {
                println!("=== Bắt đầu tải mô hình GLiNER Multi v2.1 INT8 ONNX (~350MB) ===");
                caption_ner::ensure_gliner_model_downloaded(&models)?;
                println!("=== Tải mô hình GLiNER thành công! ===");
                return Ok(None);
            }
            _ if arg.starts_with('-') => bail!("Tùy chọn không được hỗ trợ: {arg}\n\n{}", usage()),
            _ if input.is_none() => input = Some(PathBuf::from(arg)),
            _ => bail!("Chỉ nhận một input file hoặc folder.\n\n{}", usage()),
        }
    }

    let Some(input) = input else {
        bail!("{}", usage());
    };
    let Some(output) = output else {
        bail!("Thiếu --output <folder>.\n\n{}", usage());
    };
    Ok(Some(CliArgs {
        input,
        output,
        models,
        caption_evidence,
        subtitle_line,
        caption_line,
        subtitle_roi,
        caption_roi,
        force,
        attribute_speakers,
        gallery,
    }))
}

fn parse_args() -> Result<Option<CliArgs>> {
    parse_args_from(env::args().skip(1))
}

fn video_files(input: &Path) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        let extension = input.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        if !VIDEO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str()) {
            bail!("Định dạng video không được hỗ trợ: {:?}", input);
        }
        return Ok(vec![input.to_path_buf()]);
    }
    if !input.is_dir() {
        bail!(
            "Input không tồn tại hoặc không phải file/folder: {:?}",
            input
        );
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(input).with_context(|| format!("Không thể đọc folder {:?}", input))?
    {
        let path = entry?.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| VIDEO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        bail!("Không tìm thấy video trực tiếp trong folder {:?}", input);
    }
    let mut stems = HashSet::new();
    for file in &files {
        let stem = file
            .file_stem()
            .and_then(|value| value.to_str())
            .context("Tên file video không hợp lệ")?
            .to_ascii_lowercase();
        if !stems.insert(stem.clone()) {
            bail!("Có nhiều video cùng tên gốc `{stem}`; output và .tmp_<tên> sẽ bị trùng");
        }
    }
    Ok(files)
}

fn sibling_temp_dir(video: &Path) -> Result<PathBuf> {
    let stem = video
        .file_stem()
        .and_then(|value| value.to_str())
        .context("Tên file video không hợp lệ")?;
    Ok(video
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".tmp_{stem}")))
}

fn file_fingerprint(path: &Path) -> Result<FileFingerprint> {
    let canonical = fs::canonicalize(path)
        .with_context(|| format!("Không thể chuẩn hóa đường dẫn {:?}", path))?;
    let metadata =
        fs::metadata(path).with_context(|| format!("Không thể đọc thông tin {:?}", path))?;
    let modified_unix_nanos = metadata
        .modified()
        .with_context(|| format!("Không thể đọc thời gian sửa đổi của {:?}", path))?
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    Ok(FileFingerprint {
        path: canonical.to_string_lossy().into_owned(),
        size: metadata.len(),
        modified_unix_nanos,
    })
}

fn signatures(video: &Path, args: &CliArgs) -> Result<(String, String, String)> {
    let video_fingerprint = file_fingerprint(video)?;
    let detector_model = file_fingerprint(&args.models.join("ocr").join("det.onnx"))?;
    let recognition_model = file_fingerprint(&args.models.join("ocr").join("rec.onnx"))?;
    let dictionary = file_fingerprint(&args.models.join("ocr").join("keys.txt"))?;
    let subtitle_config = OcrExtractorConfig {
        roi: args.subtitle_roi.clone(),
        ..OcrExtractorConfig::default()
    };
    let mut caption_config = CaptionExtractorConfig::default();
    caption_config.roi.y_min = args.caption_roi.0;
    caption_config.roi.y_max = args.caption_roi.1;
    caption_config.roi.x_min = args.caption_roi.2;
    caption_config.roi.x_max = args.caption_roi.3;

    let subtitle_signature = serde_json::to_string(&SubtitleSignature {
        video: video_fingerprint.clone(),
        detector_model: detector_model.clone(),
        recognition_model: recognition_model.clone(),
        dictionary: dictionary.clone(),
        roi: [
            subtitle_config.roi.y_min,
            subtitle_config.roi.y_max,
            subtitle_config.roi.x_min,
            subtitle_config.roi.x_max,
        ],
        fps: subtitle_config.fps,
        accuracy_mode: subtitle_config.accuracy_mode,
        evidence: args.caption_evidence,
    })
    .context("Không thể tạo chữ ký cấu hình subtitle")?;
    let caption_signature = serde_json::to_string(&CaptionSignature {
        video: video_fingerprint,
        detector_model,
        recognition_model,
        dictionary,
        roi: [
            caption_config.roi.y_min,
            caption_config.roi.y_max,
            caption_config.roi.x_min,
            caption_config.roi.x_max,
        ],
        fps: caption_config.fps,
        vertical_aspect_threshold: caption_config.vertical_aspect_threshold,
        column_x_tolerance: caption_config.column_x_tolerance,
        min_confidence: caption_config.min_confidence,
        min_duration_sec: caption_config.min_duration_sec,
        watermark_freq_threshold: caption_config.watermark_freq_threshold,
        evidence: args.caption_evidence,
        evidence_warning_confidence: caption_config.evidence_warning_confidence,
        evidence_duration_margin_sec: caption_config.evidence_duration_margin_sec,
    })
    .context("Không thể tạo chữ ký cấu hình caption")?;
    let merge_signature = serde_json::to_string(&MergeSignature {
        subtitle: &subtitle_signature,
        caption: &caption_signature,
        subtitle_line: args.subtitle_line.as_deref(),
        caption_line: args.caption_line.as_deref(),
    })
    .context("Không thể tạo chữ ký cấu hình merge")?;

    Ok((subtitle_signature, caption_signature, merge_signature))
}

fn load_checkpoint(path: &Path) -> Result<RunCheckpoint> {
    if !path.exists() {
        return Ok(RunCheckpoint::default());
    }
    let data = fs::read(path).with_context(|| format!("Không thể đọc checkpoint {:?}", path))?;
    let checkpoint: RunCheckpoint = serde_json::from_slice(&data)
        .with_context(|| format!("Checkpoint lỗi; dùng --force để tạo lại: {:?}", path))?;
    if checkpoint.version != STATE_VERSION {
        bail!(
            "Phiên bản checkpoint không được hỗ trợ; dùng --force để tạo lại: {:?}",
            path
        );
    }
    Ok(checkpoint)
}

fn save_checkpoint(path: &Path, checkpoint: &RunCheckpoint) -> Result<()> {
    let temp_path = path.with_file_name(format!("state.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(checkpoint).context("Không thể mã hóa checkpoint")?;
    let mut file = File::create(&temp_path)
        .with_context(|| format!("Không thể ghi checkpoint tạm {:?}", temp_path))?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("Không thể cập nhật checkpoint {:?}", path))?;
    Ok(())
}

fn update_stage_signature(stage: &mut StageCheckpoint, signature: &str, artifact_valid: bool) {
    if stage.signature != signature {
        stage.signature = signature.to_string();
        stage.completed = false;
        stage.warnings = 0;
    }
    if !artifact_valid {
        stage.completed = false;
        stage.warnings = 0;
    }
}

fn stage_is_complete(stage: &StageCheckpoint, signature: &str, artifact_valid: bool) -> bool {
    stage.completed && stage.signature == signature && artifact_valid
}

fn remove_directory_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).with_context(|| format!("Không thể dọn thư mục {:?}", path))?;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_file(path).with_context(|| format!("Không thể dọn file {:?}", path))?;
    }
    Ok(())
}

fn is_valid_vtt(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|contents| contents.lines().next() == Some("WEBVTT"))
        .unwrap_or(false)
}

fn video_dimensions(video: &Path) -> Result<(usize, usize)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            &video.to_string_lossy(),
        ])
        .output()
        .context("Không thể chạy ffprobe để lấy kích thước video")?;
    if !output.status.success() {
        bail!(
            "ffprobe không đọc được video {:?}: {}",
            video,
            output.status
        );
    }
    let dimensions: Vec<usize> = String::from_utf8_lossy(&output.stdout)
        .trim()
        .split(',')
        .map(str::parse)
        .collect::<std::result::Result<_, _>>()
        .context("ffprobe trả về kích thước video không hợp lệ")?;
    if dimensions.len() != 2 || dimensions.contains(&0) {
        bail!(
            "ffprobe trả về kích thước video không hợp lệ: {:?}",
            dimensions
        );
    }
    Ok((dimensions[0], dimensions[1]))
}

fn even_crop(value: f32, size: usize) -> usize {
    (((size as f32 * value).round() as usize) / 2) * 2
}

fn export_subtitle_evidence(
    video: &Path,
    evidence_dir: &Path,
    config: &OcrExtractorConfig,
    segments: &[SubtitleSegment],
) -> Result<()> {
    fs::create_dir_all(evidence_dir)?;
    let fps = config.fps;
    if fps == 0 {
        bail!("FPS phải lớn hơn 0 khi xuất bằng chứng subtitle");
    }
    let (source_width, source_height) = video_dimensions(video)?;
    let crop_width = even_crop(config.roi.x_max - config.roi.x_min, source_width);
    let crop_height = even_crop(config.roi.y_max - config.roi.y_min, source_height);
    let crop_x = even_crop(config.roi.x_min, source_width);
    let crop_y = even_crop(config.roi.y_min, source_height);
    if crop_width == 0 || crop_height == 0 {
        bail!("ROI subtitle quá nhỏ để xuất ảnh evidence");
    }

    let mut targets: BTreeMap<usize, BTreeMap<usize, Vec<&str>>> = BTreeMap::new();
    for (cue_index, cue) in segments.iter().enumerate() {
        let first = (cue.start_sec.max(0.0) * fps as f64).ceil() as usize;
        let after_last = (cue.end_sec.max(0.0) * fps as f64).ceil() as usize;
        let last = after_last.saturating_sub(1).max(first);
        let best = ((cue.start_sec + cue.end_sec) * 0.5 * fps as f64)
            .floor()
            .max(first as f64) as usize;
        for (stage, frame) in [("first", first), ("best", best.min(last)), ("last", last)] {
            let stages = targets
                .entry(frame)
                .or_default()
                .entry(cue_index)
                .or_default();
            if !stages.contains(&stage) {
                stages.push(stage);
            }
        }
    }

    let manifest_path = evidence_dir.join("manifest.tsv");
    if targets.is_empty() {
        fs::write(&manifest_path, "cue\ttext\timages\n")?;
        return Ok(());
    }

    let filter = format!("fps={fps},crop={crop_width}:{crop_height}:{crop_x}:{crop_y}");
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-i",
            &video.to_string_lossy(),
            "-vf",
            &filter,
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-v",
            "error",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Không thể khởi động FFmpeg để xuất subtitle evidence")?;
    let mut stdout = ffmpeg.stdout.take().context("Không thể mở stdout FFmpeg")?;
    let frame_bytes = crop_width
        .checked_mul(crop_height)
        .and_then(|size| size.checked_mul(3))
        .context("Kích thước frame ROI vượt giới hạn")?;
    let mut buffer = vec![0u8; frame_bytes];
    let mut evidence_files = vec![Vec::<String>::new(); segments.len()];
    let mut frame_index = 0usize;

    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
        if let Some(frame_targets) = targets.get(&frame_index) {
            let image = RgbImage::from_raw(crop_width as u32, crop_height as u32, buffer.clone())
                .context("Không thể tạo ảnh subtitle evidence")?;
            let timestamp_ms = (frame_index as f64 * 1000.0 / fps as f64).round() as u64;
            for (&cue_index, stages) in frame_targets {
                let file_name = format!(
                    "cue_{:03}_{}_frame_{:06}_t_{:09}ms.png",
                    cue_index + 1,
                    stages.join("_"),
                    frame_index,
                    timestamp_ms
                );
                image.save(evidence_dir.join(&file_name))?;
                evidence_files[cue_index].push(file_name);
            }
        }
        frame_index += 1;
    }

    let status = ffmpeg.wait().context("Không thể chờ FFmpeg evidence")?;
    if !status.success() {
        bail!("FFmpeg lỗi khi xuất subtitle evidence: {status}");
    }
    if targets.keys().any(|frame| *frame >= frame_index) {
        bail!(
            "Không tìm thấy toàn bộ frame subtitle evidence; video có {} frame mẫu",
            frame_index
        );
    }

    let mut manifest = String::from("cue\ttext\timages\n");
    for (index, cue) in segments.iter().enumerate() {
        manifest.push_str(&format!(
            "{}\t{}\t{}\n",
            index + 1,
            cue.text.replace(['\t', '\n'], " "),
            evidence_files[index].join(";")
        ));
    }
    fs::write(manifest_path, manifest)?;
    Ok(())
}

const STUDIO_CACHE_FILE: &str = ".studio_cache.ferrite";

#[derive(Deserialize)]
struct GalleryFile {
    #[serde(default)]
    characters: BTreeMap<String, GalleryCharacter>,
}

#[derive(Deserialize)]
struct GalleryCharacter {
    #[serde(default)]
    name: String,
}

/// Mỗi thư mục `--input` là một bộ phim nên cache FerriteDB nằm ngay trong đó.
fn movie_cache_path(input: &Path, video: &Path) -> PathBuf {
    if input.is_dir() {
        input.join(STUDIO_CACHE_FILE)
    } else {
        video
            .parent()
            .map(|parent| parent.join(STUDIO_CACHE_FILE))
            .unwrap_or_else(|| PathBuf::from(STUDIO_CACHE_FILE))
    }
}

fn gallery_path(args: &CliArgs, video: &Path) -> PathBuf {
    if let Some(ref gallery) = args.gallery {
        gallery.clone()
    } else if args.input.is_dir() {
        args.input.join(".character_gallery.json")
    } else {
        video
            .parent()
            .map(|parent| parent.join(".character_gallery.json"))
            .unwrap_or_else(|| PathBuf::from(".character_gallery.json"))
    }
}

fn gallery_names(path: &Path) -> Vec<String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(gallery) = serde_json::from_str::<GalleryFile>(&contents) else {
        return Vec::new();
    };
    let mut names: Vec<String> = gallery
        .characters
        .into_iter()
        .flat_map(|(key, character)| {
            let name = character.name.trim().to_string();
            if name.is_empty() || name == key {
                vec![key]
            } else {
                vec![key, name]
            }
        })
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Đưa nhãn nhân vật từ caption về tên chuẩn trong gallery.
/// Nhãn bị OCR cắt cụt ("三位神明境強者" -> "三位神") hoặc lệch khoảng trắng
/// (" 陆清安") vẫn khớp được với tên đã đăng ký.
fn canonical_speaker(name: &str, gallery: &[String]) -> String {
    let cleaned: String = name.chars().filter(|ch| !ch.is_whitespace()).collect();
    if cleaned.is_empty() {
        return String::new();
    }
    if let Some(exact) = gallery.iter().find(|known| **known == cleaned) {
        return exact.clone();
    }
    gallery
        .iter()
        .filter(|known| known.chars().count() >= 2)
        .filter(|known| cleaned.starts_with(known.as_str()) || known.starts_with(&cleaned))
        .max_by_key(|known| known.chars().count())
        .cloned()
        .unwrap_or(cleaned)
}

fn split_speaker(text: &str) -> (String, String) {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("<v ")
        && let Some((name, body)) = rest.split_once('>')
    {
        let body = body.strip_suffix("</v>").unwrap_or(body);
        return (name.trim().to_string(), body.trim().to_string());
    }
    (String::new(), trimmed.to_string())
}

fn normalize_speaker_tags(vtt_path: &Path, gallery: &[String]) -> Result<usize> {
    if gallery.is_empty() {
        return Ok(0);
    }
    let contents = fs::read_to_string(vtt_path)
        .with_context(|| format!("Không thể đọc VTT {:?}", vtt_path))?;
    let mut changed = 0usize;
    let mut output = String::with_capacity(contents.len());
    for line in contents.lines() {
        let Some((name, body)) = line
            .strip_prefix("<v ")
            .and_then(|rest| rest.split_once('>'))
        else {
            output.push_str(line);
            output.push('\n');
            continue;
        };
        let canonical = canonical_speaker(name, gallery);
        if canonical != name {
            changed += 1;
        }
        output.push_str(&format!("<v {canonical}>{body}\n"));
    }
    if changed > 0 {
        fs::write(vtt_path, output).with_context(|| format!("Không thể ghi VTT {:?}", vtt_path))?;
    }
    Ok(changed)
}

fn parse_vtt_timestamp_ms(value: &str) -> Option<i64> {
    let mut parts = value.trim().split(':');
    let hours: i64 = parts.next()?.trim().parse().ok()?;
    let minutes: i64 = parts.next()?.trim().parse().ok()?;
    let rest = parts.next()?.trim();
    let (seconds, millis) = rest.split_once('.').unwrap_or((rest, "0"));
    let seconds: i64 = seconds.parse().ok()?;
    let mut millis_digits = millis.to_string();
    millis_digits.truncate(3);
    while millis_digits.len() < 3 {
        millis_digits.push('0');
    }
    let millis: i64 = millis_digits.parse().ok()?;
    Some(((hours * 60 + minutes) * 60 + seconds) * 1000 + millis)
}

#[derive(Debug, Clone, PartialEq)]
struct MergedCue {
    start_ms: i64,
    end_ms: i64,
    speaker: String,
    text: String,
}

fn parse_merged_cues(contents: &str) -> Vec<MergedCue> {
    let mut cues = Vec::new();
    let mut pending: Option<(i64, i64)> = None;
    for line in contents.lines() {
        let line = line.trim();
        if let Some((left, right)) = line.split_once("-->") {
            let start = left.trim().split(' ').next().unwrap_or_default();
            let end = right.trim().split(' ').next().unwrap_or_default();
            pending = match (parse_vtt_timestamp_ms(start), parse_vtt_timestamp_ms(end)) {
                (Some(start_ms), Some(end_ms)) => Some((start_ms, end_ms)),
                _ => None,
            };
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if let Some((start_ms, end_ms)) = pending.take() {
            let (speaker, text) = split_speaker(line);
            if !text.is_empty() {
                cues.push(MergedCue {
                    start_ms,
                    end_ms,
                    speaker,
                    text,
                });
            }
        }
    }
    cues
}

/// Mã tập lấy từ số trong tên file (S02E001 -> 2001) để cache/resume theo từng tập.
fn cue_chunk_id(video: &Path) -> i64 {
    video
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            stem.chars()
                .filter(char::is_ascii_digit)
                .collect::<String>()
        })
        .and_then(|digits| digits.parse::<i64>().ok())
        .unwrap_or(0)
}

/// Cắt crop PNG của từng caption cần Stage 2 (VLM) đọc lại vào thư mục tạm.
fn export_vlm_crops(
    video: &Path,
    crops_dir: &Path,
    config: &CaptionExtractorConfig,
    warnings: &[CaptionWarning],
) -> Result<Vec<ocr_shared::ferrite_cache::VlmJobRequest>> {
    if warnings.is_empty() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(crops_dir)?;
    let (source_width, source_height) = video_dimensions(video)?;
    let crop_width = even_crop(config.roi.x_max - config.roi.x_min, source_width);
    let crop_height = even_crop(config.roi.y_max - config.roi.y_min, source_height);
    let crop_x = even_crop(config.roi.x_min, source_width);
    let crop_y = even_crop(config.roi.y_min, source_height);
    if crop_width == 0 || crop_height == 0 {
        bail!("ROI caption quá nhỏ để xuất crop VLM");
    }
    let filter = format!("crop={crop_width}:{crop_height}:{crop_x}:{crop_y}");

    let mut jobs = Vec::with_capacity(warnings.len());
    for (index, warning) in warnings.iter().enumerate() {
        let timestamp_sec = ((warning.start_sec + warning.end_sec) * 0.5).max(0.0);
        let start_ms = (warning.start_sec * 1000.0).round() as i64;
        let file_name = format!("vlm_{:03}_t_{:09}ms.png", index + 1, start_ms);
        let path = crops_dir.join(&file_name);
        let status = Command::new("ffmpeg")
            .args([
                "-ss",
                &format!("{timestamp_sec:.3}"),
                "-i",
                &video.to_string_lossy(),
                "-vf",
                &filter,
                "-frames:v",
                "1",
                "-y",
                &path.to_string_lossy(),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("Không thể khởi động FFmpeg để xuất crop VLM")?;
        if !status.success() || !path.is_file() {
            bail!(
                "FFmpeg không xuất được crop VLM cho caption tại {:.2}s",
                warning.start_sec
            );
        }
        jobs.push(ocr_shared::ferrite_cache::VlmJobRequest {
            start_ms,
            crop_png_path: path.to_string_lossy().to_string(),
            reason: format!(
                "conf={:.3}; {}",
                warning.confidence,
                warning.reasons.join(", ")
            ),
        });
    }
    Ok(jobs)
}

/// Đẩy các caption cần Stage 2 vào bảng `vlm_jobs` của cache FerriteDB.
fn enqueue_vlm_jobs(
    args: &CliArgs,
    video: &Path,
    crops_dir: &Path,
    config: &CaptionExtractorConfig,
    warnings: &[CaptionWarning],
) -> Result<usize> {
    let jobs = export_vlm_crops(video, crops_dir, config, warnings)?;
    if jobs.is_empty() {
        return Ok(0);
    }
    let mut cache = ocr_shared::ferrite_cache::StudioCacheDb::open_or_create(&movie_cache_path(
        &args.input,
        video,
    ))?;
    cache.save_vlm_jobs(cue_chunk_id(video), &jobs)
}

fn cache_merged_cues(args: &CliArgs, video: &Path, merged_vtt: &Path) -> Result<usize> {
    let contents = fs::read_to_string(merged_vtt)
        .with_context(|| format!("Không thể đọc VTT {:?}", merged_vtt))?;
    let cues = parse_merged_cues(&contents);
    if cues.is_empty() {
        return Ok(0);
    }
    let chunk_id = cue_chunk_id(video);
    let mut cache = ocr_shared::ferrite_cache::StudioCacheDb::open_or_create(&movie_cache_path(
        &args.input,
        video,
    ))?;
    if cache.has_chunk_raw_cues(chunk_id)? {
        return Ok(0);
    }
    let cached: Vec<ocr_shared::ferrite_cache::CachedRawCue> = cues
        .iter()
        .map(|cue| ocr_shared::ferrite_cache::CachedRawCue {
            chunk_id,
            start_ms: cue.start_ms,
            end_ms: cue.end_ms,
            cue_kind: if cue.speaker.is_empty() { "CAP" } else { "SUB" }.to_string(),
            speaker_hint: cue.speaker.clone(),
            raw_zh: cue.text.clone(),
            confidence: 1.0,
            vad_speech_ratio: 1.0,
        })
        .collect();
    cache.save_chunk_raw_cues(chunk_id, &cached, &[])?;
    Ok(cached.len())
}

fn process_video(
    video: &Path,
    args: &CliArgs,
    progress_state: &Mutex<ProgressState>,
) -> Result<()> {
    reset_progress(progress_state);
    let stem = video
        .file_stem()
        .and_then(|value| value.to_str())
        .context("Tên file video không hợp lệ")?;
    let temp_dir = sibling_temp_dir(video)?;
    fs::create_dir_all(&temp_dir)?;
    fs::create_dir_all(&args.output)?;

    let subtitle_vtt = temp_dir.join("subtitles.zh.vtt");
    let caption_vtt = temp_dir.join("captions.zh.vtt");
    let merged_vtt = args.output.join(format!("{stem}.merged.zh.vtt"));
    let subtitle_evidence = temp_dir.join("subtitle_evidence");

    let subtitle_config = OcrExtractorConfig {
        roi: args.subtitle_roi.clone(),
        ..OcrExtractorConfig::default()
    };
    let mut caption_config = CaptionExtractorConfig::default();
    caption_config.roi.y_min = args.caption_roi.0;
    caption_config.roi.y_max = args.caption_roi.1;
    caption_config.roi.x_min = args.caption_roi.2;
    caption_config.roi.x_max = args.caption_roi.3;
    if args.caption_evidence {
        caption_config.evidence_dir = Some(temp_dir.join("caption_evidence"));
    }

    let (subtitle_signature, caption_signature, merge_signature) = signatures(video, args)?;
    let state_path = temp_dir.join("state.json");
    let mut checkpoint = if args.force {
        RunCheckpoint::default()
    } else {
        load_checkpoint(&state_path)?
    };
    let subtitle_evidence_valid =
        !args.caption_evidence || subtitle_evidence.join("manifest.tsv").is_file();
    let caption_evidence_dir = temp_dir.join("caption_evidence");
    let caption_evidence_valid = !args.caption_evidence
        || (caption_evidence_dir.join("manifest.tsv").is_file()
            && caption_evidence_dir.join("warnings.tsv").is_file());
    update_stage_signature(
        &mut checkpoint.subtitle,
        &subtitle_signature,
        is_valid_vtt(&subtitle_vtt) && subtitle_evidence_valid,
    );
    update_stage_signature(
        &mut checkpoint.caption,
        &caption_signature,
        is_valid_vtt(&caption_vtt) && caption_evidence_valid,
    );
    let subtitle_done = !args.force
        && (is_valid_vtt(&subtitle_vtt)
            || stage_is_complete(
                &checkpoint.subtitle,
                &subtitle_signature,
                is_valid_vtt(&subtitle_vtt) && subtitle_evidence_valid,
            ));
    let caption_done = !args.force
        && (is_valid_vtt(&caption_vtt)
            || stage_is_complete(
                &checkpoint.caption,
                &caption_signature,
                is_valid_vtt(&caption_vtt) && caption_evidence_valid,
            ));
    update_stage_signature(
        &mut checkpoint.merge,
        &merge_signature,
        is_valid_vtt(&merged_vtt) && subtitle_done && caption_done,
    );
    let merge_done = !args.force
        && !args.attribute_speakers
        && stage_is_complete(
            &checkpoint.merge,
            &merge_signature,
            is_valid_vtt(&merged_vtt) && subtitle_done && caption_done,
        );
    save_checkpoint(&state_path, &checkpoint)?;

    println!(" [OCR] {} -> {:?}", video.display(), merged_vtt);
    if args.force {
        println!("  [FORCE] Bỏ checkpoint, chạy lại toàn bộ các bước.");
    }
    if subtitle_done {
        println!("  [SUB: SKIP] Đã hoàn tất; dùng lại {:?}", subtitle_vtt);
    }
    if caption_done {
        println!("  [CAP: SKIP] Đã hoàn tất; dùng lại {:?}", caption_vtt);
        if checkpoint.caption.warnings > 0 {
            println!(
                "{ANSI_ORANGE}  [CAP: WARN] Có {} cue cần duyệt.{ANSI_RESET}",
                checkpoint.caption.warnings
            );
        }
    }
    if merge_done {
        println!("  [MERGE: SKIP] Đã hoàn tất; dùng lại {:?}", merged_vtt);
    }

    let ocr_needed = !subtitle_done || !caption_done;
    if ocr_needed {
        set_phase_progress(
            progress_state,
            true,
            if subtitle_done { 100.0 } else { 0.0 },
            if subtitle_done {
                if checkpoint.subtitle.warnings > 0 {
                    PhaseStatus::Warning
                } else {
                    PhaseStatus::Complete
                }
            } else {
                PhaseStatus::Running
            },
        );
        set_phase_progress(
            progress_state,
            false,
            if caption_done { 100.0 } else { 0.0 },
            if caption_done {
                if checkpoint.caption.warnings > 0 {
                    PhaseStatus::Warning
                } else {
                    PhaseStatus::Complete
                }
            } else {
                PhaseStatus::Running
            },
        );
    }

    if !subtitle_done {
        remove_file_if_exists(&subtitle_vtt)?;
        if args.caption_evidence {
            remove_directory_if_exists(&subtitle_evidence)?;
        }
    }
    if !caption_done {
        remove_file_if_exists(&caption_vtt)?;
        if args.caption_evidence {
            remove_directory_if_exists(&caption_evidence_dir)?;
        }
    }

    let (subtitle_result, caption_result) = thread::scope(|scope| {
        let subtitle_handle = (!subtitle_done).then(|| {
            scope.spawn(|| {
                extract_hardcoded_subtitles(video, &subtitle_vtt, &args.models, &subtitle_config)
                    .with_context(|| format!("Trích xuất subtitle thất bại cho {:?}", video))
            })
        });
        let caption_handle = (!caption_done).then(|| {
            scope.spawn(|| {
                extract_captions(video, &caption_vtt, &args.models, &caption_config)
                    .with_context(|| format!("Trích xuất caption thất bại cho {:?}", video))
            })
        });
        let subtitle_result = match subtitle_handle {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| Err(anyhow!("Thread trích xuất subtitle bị panic")))
                .map(Some),
            None => Ok(None),
        };
        let caption_result = match caption_handle {
            Some(handle) => handle
                .join()
                .unwrap_or_else(|_| Err(anyhow!("Thread trích xuất caption bị panic")))
                .map(Some),
            None => Ok(None),
        };
        (subtitle_result, caption_result)
    });

    let mut extraction_errors = Vec::new();
    let mut caption_output_for_display: Option<CaptionExtractorOutput> = None;
    match subtitle_result {
        Ok(Some(output)) => {
            let evidence_result = if args.caption_evidence {
                export_subtitle_evidence(
                    video,
                    &subtitle_evidence,
                    &subtitle_config,
                    &output.segments,
                )
            } else {
                Ok(())
            };
            if let Err(error) = evidence_result {
                extraction_errors.push(format!("subtitle evidence: {error:#}"));
            } else {
                checkpoint.subtitle.completed = true;
                checkpoint.subtitle.warnings = 0;
                save_checkpoint(&state_path, &checkpoint)?;
            }
        }
        Ok(None) => {}
        Err(error) => {
            extraction_errors.push(format!("subtitle: {error:#}"));
        }
    }
    match caption_result {
        Ok(Some(output)) => {
            checkpoint.caption.completed = true;
            checkpoint.caption.warnings = output.warnings.len();
            save_checkpoint(&state_path, &checkpoint)?;
            caption_output_for_display = Some(output);
        }
        Ok(None) => {}
        Err(error) => {
            extraction_errors.push(format!("caption: {error:#}"));
        }
    }
    if !extraction_errors.is_empty() {
        finalize_progress(
            progress_state,
            checkpoint.subtitle.completed,
            checkpoint.subtitle.warnings > 0,
            checkpoint.caption.completed,
            checkpoint.caption.warnings > 0,
        );
        println!();
        bail!(
            "Trích xuất thất bại cho {:?}: {}",
            video,
            extraction_errors.join("; ")
        );
    }

    if ocr_needed {
        finalize_progress(
            progress_state,
            checkpoint.subtitle.completed,
            checkpoint.subtitle.warnings > 0,
            checkpoint.caption.completed,
            checkpoint.caption.warnings > 0,
        );
        println!();
    }

    if let Some(output) = &caption_output_for_display {
        for warning in &output.warnings {
            println!(
                "{ANSI_ORANGE} >>> [CAP: WARN]{ANSI_RESET} {} | {:.2}s | confidence={:.3} | {}",
                warning.text,
                warning.duration_sec,
                warning.confidence,
                warning.reasons.join(", ")
            );
        }
    }
    if args.caption_evidence {
        println!(" >>> Evidence caption: {:?}", caption_evidence_dir);
        println!(" >>> Evidence subtitle: {:?}", subtitle_evidence);
    }

    if !merge_done {
        checkpoint.merge.completed = false;
        save_checkpoint(&state_path, &checkpoint)?;
    }
    if merge_done {
        println!(
            " {ANSI_GREEN}[MERGE: SKIP]{ANSI_RESET} Giữ nguyên file đã merge {:?}",
            merged_vtt
        );
    } else {
        match merge_vtt_files_with_line_overrides(
            &subtitle_vtt,
            &caption_vtt,
            &merged_vtt,
            args.subtitle_line.as_deref(),
            args.caption_line.as_deref(),
        ) {
            Ok(summary) => {
                checkpoint.merge.completed = true;
                save_checkpoint(&state_path, &checkpoint)?;
                println!(
                    " {ANSI_GREEN}[MERGE: DONE]{ANSI_RESET} {} cues ({} subtitle + {} caption); raw VTT: {:?}",
                    summary.total_merged_cues,
                    summary.subtitle_cues,
                    summary.caption_cues,
                    temp_dir
                );
            }
            Err(error) => {
                eprintln!("\n{ANSI_RED}[MERGE: ERROR]{ANSI_RESET} {error:#}");
                return Err(error);
            }
        }
    }
    if args.attribute_speakers {
        let gallery_file = gallery_path(args, video);
        println!(
            " {ANSI_CYAN}[ASD]{ANSI_RESET} Attributing speakers for {:?}",
            merged_vtt
        );
        let start_time = std::time::Instant::now();
        let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
        let bar_width = 12usize;

        let total_attributed = light_asd::attribute_vtt_file_native_with_gallery_and_progress(
            video,
            &merged_vtt,
            &merged_vtt,
            Some(&gallery_file),
            |current, total| {
                let percent = (current as f32 / total.max(1) as f32 * 100.0).clamp(0.0, 99.9);
                let bar = progress_bar(percent, filled_char, empty_char, bar_width);
                let elapsed_secs = start_time.elapsed().as_secs_f32();
                print!(
                    "\r {status_running}[ASD]: {ANSI_BLUE}{bar}{ANSI_RESET} {percent:5.1}% | {current}/{total} cues | {elapsed_secs:.1}s   ",
                    status_running = status_style(PhaseStatus::Running),
                );
                let _ = io::stdout().flush();
            },
        )
        .with_context(|| format!("Speaker attribution failed for {:?}", video))?;

        let elapsed_secs = start_time.elapsed().as_secs_f32();
        let bar = progress_bar(100.0, filled_char, empty_char, bar_width);
        println!(
            "\r {status_complete}[ASD]: {ANSI_BLUE}{bar}{ANSI_RESET} 100.0% | {total_attributed}/{total_attributed} cues | {elapsed_secs:.1}s   ",
            status_complete = status_style(PhaseStatus::Complete),
        );
        println!(
            " {ANSI_GREEN}[ASD: DONE]{ANSI_RESET} Attributed {} subtitle cues with <v Character> tags",
            total_attributed
        );
    }

    if is_valid_vtt(&merged_vtt) {
        if args.attribute_speakers {
            let names = gallery_names(&gallery_path(args, video));
            let normalized = normalize_speaker_tags(&merged_vtt, &names)?;
            if normalized > 0 {
                println!(
                    " {ANSI_CYAN}[NAME]{ANSI_RESET} Chuẩn hóa {normalized} nhãn nhân vật theo gallery"
                );
            }
        }
        let cached = cache_merged_cues(args, video, &merged_vtt)?;
        let cache_path = movie_cache_path(&args.input, video);
        if cached > 0 {
            println!(
                " {ANSI_GREEN}[CACHE]{ANSI_RESET} Lưu {cached} cue vào {:?}",
                cache_path
            );
        } else {
            println!(
                " {ANSI_DIM}[CACHE: SKIP]{ANSI_RESET} Đã có dữ liệu trong {:?}",
                cache_path
            );
        }
        if let Some(output) = &caption_output_for_display
            && !output.warnings.is_empty()
        {
            let crops_dir = temp_dir.join("vlm_crops");
            match enqueue_vlm_jobs(args, video, &crops_dir, &caption_config, &output.warnings) {
                Ok(queued) if queued > 0 => println!(
                    " {ANSI_CYAN}[VLM]{ANSI_RESET} Đưa {queued} crop caption vào hàng đợi Stage 2 ({:?})",
                    crops_dir
                ),
                Ok(_) => {}
                Err(error) => {
                    eprintln!("{ANSI_ORANGE}[VLM: WARN]{ANSI_RESET} {error:#}");
                }
            }
        }
    }
    Ok(())
}

fn run() -> Result<()> {
    let Some(args) = parse_args()? else {
        return Ok(());
    };
    for model_file in ["det.onnx", "rec.onnx", "keys.txt"] {
        let path = args.models.join("ocr").join(model_file);
        if !path.is_file() {
            bail!(
                "Thiếu model {:?}; đặt det.onnx, rec.onnx, keys.txt trong {:?}",
                path,
                args.models.join("ocr")
            );
        }
    }
    let files = video_files(&args.input)?;
    let progress_state = register_progress_handler();
    let mut failures = Vec::new();
    for video in &files {
        println!("========================================");
        if let Err(error) = process_video(video, &args, &progress_state) {
            eprintln!(
                "{ANSI_RED}[ERROR]{ANSI_RESET} {}: {error:#}",
                video.display()
            );
            failures.push(video.display().to_string());
        }
        println!("========================================");
    }
    if !failures.is_empty() {
        bail!(
            "{} / {} video xử lý thất bại: {}",
            failures.len(),
            files.len(),
            failures.join(", ")
        );
    }
    println!(">>> DONE ALL <<<");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roi_parser_accepts_fractions_and_rejects_invalid_bounds() {
        assert_eq!(
            parse_roi("0.85, 1, 0.05, 0.95", "test").unwrap(),
            (0.85, 1.0, 0.05, 0.95)
        );
        assert!(parse_roi("0,1,0,1", "test").is_ok());
        assert!(parse_roi("0.9,0.8,0,1", "test").is_err());
        assert!(parse_roi("0,1,0,1,0", "test").is_err());
        assert!(parse_roi("NaN,1,0,1", "test").is_err());
    }

    #[test]
    fn input_and_output_accept_long_and_short_flags() {
        let args = [
            "--input",
            "videos",
            "-o",
            "merged",
            "--subtitle-line",
            "85%",
            "--force",
        ]
        .map(str::to_string);
        let parsed = parse_args_from(args.into_iter()).unwrap().unwrap();
        assert_eq!(parsed.input, PathBuf::from("videos"));
        assert_eq!(parsed.output, PathBuf::from("merged"));
        assert_eq!(parsed.subtitle_line.as_deref(), Some("85%"));
        assert!(parsed.force);

        let positional = ["videos", "--output", "merged"].map(str::to_string);
        assert!(parse_args_from(positional.into_iter()).unwrap().is_some());

        let conflicting =
            ["--input", "videos", "other.mp4", "--output", "merged"].map(str::to_string);
        assert!(parse_args_from(conflicting.into_iter()).is_err());
    }

    #[test]
    fn attribute_speakers_flag_is_parsed_correctly() {
        let args = [
            "--input",
            "video.mp4",
            "--output",
            "out",
            "--attribute-speakers",
            "--gallery",
            "custom_gallery.json",
        ]
        .map(str::to_string);
        let parsed = parse_args_from(args.into_iter()).unwrap().unwrap();
        assert!(parsed.attribute_speakers);
        assert_eq!(parsed.gallery, Some(PathBuf::from("custom_gallery.json")));
    }

    #[test]
    fn checkpoint_persists_completed_stages_and_invalidates_changed_signatures() {
        let unique = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "ocr-cli-checkpoint-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("state.json");

        let mut checkpoint = RunCheckpoint::default();
        checkpoint.subtitle.signature = "subtitle-v1".to_string();
        checkpoint.subtitle.completed = true;
        save_checkpoint(&path, &checkpoint).unwrap();

        checkpoint.caption.signature = "caption-v1".to_string();
        checkpoint.caption.completed = true;
        save_checkpoint(&path, &checkpoint).unwrap();

        let restored = load_checkpoint(&path).unwrap();
        assert!(stage_is_complete(&restored.subtitle, "subtitle-v1", true));
        assert!(stage_is_complete(&restored.caption, "caption-v1", true));
        assert!(!stage_is_complete(
            &restored.subtitle,
            "different-roi",
            true
        ));
        assert!(!stage_is_complete(&restored.caption, "caption-v1", false));

        let mut changed = restored.subtitle;
        update_stage_signature(&mut changed, "subtitle-v2", true);
        assert!(!changed.completed);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn signatures_scope_line_changes_to_merge_only() {
        let unique = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "ocr-cli-signatures-{}-{unique}",
            std::process::id()
        ));
        let model_dir = directory.join("models");
        let ocr_dir = model_dir.join("ocr");
        fs::create_dir_all(&ocr_dir).unwrap();
        fs::write(directory.join("video.mp4"), b"video").unwrap();
        for name in ["det.onnx", "rec.onnx", "keys.txt"] {
            fs::write(ocr_dir.join(name), name.as_bytes()).unwrap();
        }
        let mut args = CliArgs {
            input: directory.join("video.mp4"),
            output: directory.join("output"),
            models: model_dir,
            caption_evidence: false,
            subtitle_line: None,
            caption_line: None,
            subtitle_roi: SubtitleRoi::default(),
            caption_roi: (0.10, 0.80, 0.0, 1.0),
            force: false,
            attribute_speakers: false,
            gallery: None,
        };
        let original = signatures(&args.input, &args).unwrap();

        args.subtitle_line = Some("90%".to_string());
        let line_changed = signatures(&args.input, &args).unwrap();
        assert_eq!(original.0, line_changed.0);
        assert_eq!(original.1, line_changed.1);
        assert_ne!(original.2, line_changed.2);

        args.caption_roi.0 = 0.15;
        let roi_changed = signatures(&args.input, &args).unwrap();
        assert_eq!(line_changed.0, roi_changed.0);
        assert_ne!(line_changed.1, roi_changed.1);
        assert_ne!(line_changed.2, roi_changed.2);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn empty_webvtt_is_valid_for_checkpointing_zero_cue_extractions() {
        let unique = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "ocr-cli-empty-vtt-{}-{unique}.vtt",
            std::process::id()
        ));
        fs::write(&path, "WEBVTT\n\n").unwrap();
        assert!(is_valid_vtt(&path));
        fs::write(&path, "not a WebVTT file\n").unwrap();
        assert!(!is_valid_vtt(&path));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn successful_progress_finishes_at_one_hundred_percent_and_uses_status_colors() {
        assert_eq!(status_style(PhaseStatus::Error), ANSI_RED);
        assert_eq!(status_style(PhaseStatus::Warning), ANSI_ORANGE);
        assert_eq!(status_style(PhaseStatus::Complete), ANSI_GREEN);

        let state = Mutex::new(ProgressState::default());
        finalize_progress(&state, true, false, true, true);
        let state = state.lock().unwrap();
        assert_eq!(state.subtitle_percent, 100.0);
        assert_eq!(state.caption_percent, 100.0);
        assert_eq!(state.subtitle_status, PhaseStatus::Complete);
        assert_eq!(state.caption_status, PhaseStatus::Warning);
    }

    #[test]
    fn progress_bar_only_fills_completely_at_one_hundred_percent() {
        let almost_full = progress_bar(98.2, "▰", "▱", 12);
        let full = progress_bar(100.0, "▰", "▱", 12);
        assert_eq!(almost_full.chars().filter(|ch| *ch == '▰').count(), 11);
        assert_eq!(full.chars().filter(|ch| *ch == '▰').count(), 12);

        let line = format_progress_line(&ProgressState::default(), "▰▰", "▱▱");
        assert!(line.contains("[SUB]:"));
        assert!(line.contains("[CAP]:"));
        assert!(!line.contains("[▰"));
        assert!(!line.contains("▰]"));
    }

    #[test]
    fn progress_callback_tracks_subtitle_and_caption_separately() {
        let state = register_progress_handler();
        ocr::emit_task_progress("OCR Video Subtitles", 25, 100, "frames", "2s", "25%");
        ocr::emit_task_progress("Caption Phase 1", 50, 100, "frames", "5s", "50%");
        ocr::emit_task_progress("OCR Video Subtitles", 100, 100, "frames", "6s", "100%");

        let state = state.lock().unwrap();
        assert_eq!(state.subtitle_percent, 99.9);
        assert_eq!(state.caption_percent, 50.0);
        assert_eq!(state.elapsed, "6s");
        println!();
    }

    #[test]
    fn merged_cue_parsing_reads_speakers_timestamps_and_settings() {
        let vtt = "WEBVTT\n\n00:00:00.450 --> 00:00:01.150 line:90%\n<v 陆清安>来</v>\n\n00:00:07.600 --> 00:00:08.700 line:20%\n《三位神明境強者》\n\n";
        let cues = parse_merged_cues(vtt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_ms, 450);
        assert_eq!(cues[0].end_ms, 1150);
        assert_eq!(cues[0].speaker, "陆清安");
        assert_eq!(cues[0].text, "来");
        assert_eq!(cues[1].speaker, "");
        assert_eq!(cues[1].text, "《三位神明境強者》");
        assert_eq!(cues[1].end_ms, 8700);
    }

    #[test]
    fn speaker_names_are_normalized_against_the_gallery() {
        let gallery = vec!["三位神".to_string(), "陆清安".to_string()];
        assert_eq!(canonical_speaker(" 陆清安", &gallery), "陆清安");
        assert_eq!(canonical_speaker("三位神明境強者", &gallery), "三位神");
        assert_eq!(canonical_speaker("陆清", &gallery), "陆清安");
        assert_eq!(canonical_speaker("路人甲", &gallery), "路人甲");
    }

    #[test]
    fn cue_chunk_id_uses_episode_digits() {
        assert_eq!(cue_chunk_id(Path::new("in/S02E001.mp4")), 2001);
        assert_eq!(cue_chunk_id(Path::new("episode.mp4")), 0);
    }
}
