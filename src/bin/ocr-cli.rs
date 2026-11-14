use anyhow::{Context, Result, anyhow, bail};
use image::RgbImage;
use ocr::{
    CaptionExtractorConfig, OcrExtractorConfig, SubtitleRoi, SubtitleSegment, extract_captions,
    extract_hardcoded_subtitles, merge_vtt_files_with_line_overrides,
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
        let percent = (current as f32 / total as f32 * 100.0).clamp(0.0, 100.0);

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
    let subtitle_filled =
        ((state.subtitle_percent / 100.0 * bar_width as f32).round() as usize).min(bar_width);
    let caption_filled =
        ((state.caption_percent / 100.0 * bar_width as f32).round() as usize).min(bar_width);
    let subtitle_bar =
        filled_char.repeat(subtitle_filled) + &empty_char.repeat(bar_width - subtitle_filled);
    let caption_bar =
        filled_char.repeat(caption_filled) + &empty_char.repeat(bar_width - caption_filled);

    print!(
        "\r {}[SUB]{} [{}{}{}] {:5.1}% | {}[CAP]{} [{}{}{}] {:5.1}% | {}   ",
        status_style(state.subtitle_status),
        ANSI_RESET,
        ANSI_BLUE,
        subtitle_bar,
        ANSI_RESET,
        state.subtitle_percent,
        status_style(state.caption_status),
        ANSI_RESET,
        ANSI_BLUE,
        caption_bar,
        ANSI_RESET,
        state.caption_percent,
        state.elapsed
    );
    if let Err(error) = io::stdout().flush() {
        eprintln!("\n[WARN] Không thể cập nhật thanh tiến trình: {error}");
    }
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

fn set_phase_status(state: &Mutex<ProgressState>, subtitle: bool, status: PhaseStatus) {
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    if let Ok(mut state) = state.lock() {
        if subtitle {
            state.subtitle_status = status;
        } else {
            state.caption_status = status;
        }
        render_progress(&state, filled_char, empty_char, 12);
    }
}

fn usage() -> &'static str {
    "OCR-only video extractor and WebVTT merger\n\
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
      --force                    Reprocess completed subtitles, captions, and merge\n\
  -h, --help                     Show this help\n\
\n\
Each video gets a sibling .tmp_<stem> directory containing subtitles.zh.vtt and\n\
captions.zh.vtt and a state.json checkpoint. Input folders are scanned non-recursively. A positional input\n\
is also accepted for backwards compatibility."
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
    let subtitle_done = stage_is_complete(
        &checkpoint.subtitle,
        &subtitle_signature,
        is_valid_vtt(&subtitle_vtt) && subtitle_evidence_valid,
    );
    let caption_done = stage_is_complete(
        &checkpoint.caption,
        &caption_signature,
        is_valid_vtt(&caption_vtt) && caption_evidence_valid,
    );
    update_stage_signature(
        &mut checkpoint.merge,
        &merge_signature,
        is_valid_vtt(&merged_vtt) && subtitle_done && caption_done,
    );
    let merge_done = stage_is_complete(
        &checkpoint.merge,
        &merge_signature,
        is_valid_vtt(&merged_vtt) && subtitle_done && caption_done,
    );
    save_checkpoint(&state_path, &checkpoint)?;

    println!("[OCR] {} -> {:?}", video.display(), merged_vtt);
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

    println!();
    let mut extraction_errors = Vec::new();
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
                set_phase_status(progress_state, true, PhaseStatus::Error);
                extraction_errors.push(format!("subtitle evidence: {error:#}"));
            } else {
                checkpoint.subtitle.completed = true;
                checkpoint.subtitle.warnings = 0;
                save_checkpoint(&state_path, &checkpoint)?;
                set_phase_progress(progress_state, true, 100.0, PhaseStatus::Complete);
            }
        }
        Ok(None) => {}
        Err(error) => {
            set_phase_status(progress_state, true, PhaseStatus::Error);
            extraction_errors.push(format!("subtitle: {error:#}"));
        }
    }
    match caption_result {
        Ok(Some(output)) => {
            checkpoint.caption.completed = true;
            checkpoint.caption.warnings = output.warnings.len();
            save_checkpoint(&state_path, &checkpoint)?;
            let status = if output.warnings.is_empty() {
                PhaseStatus::Complete
            } else {
                PhaseStatus::Warning
            };
            set_phase_progress(progress_state, false, 100.0, status);
            for warning in &output.warnings {
                println!(
                    "\n{ANSI_ORANGE}[CAP: WARN]{ANSI_RESET} {} | {:.2}s | confidence={:.3} | {}",
                    warning.text,
                    warning.duration_sec,
                    warning.confidence,
                    warning.reasons.join(", ")
                );
            }
            if args.caption_evidence {
                println!(
                    "\n  Evidence caption: {:?}",
                    output
                        .evidence_dir
                        .as_deref()
                        .unwrap_or(&caption_evidence_dir)
                );
            }
        }
        Ok(None) => {}
        Err(error) => {
            set_phase_status(progress_state, false, PhaseStatus::Error);
            extraction_errors.push(format!("caption: {error:#}"));
        }
    }
    println!();
    if !extraction_errors.is_empty() {
        bail!(
            "Trích xuất thất bại cho {:?}: {}",
            video,
            extraction_errors.join("; ")
        );
    }

    if args.caption_evidence && !subtitle_done {
        println!("  Evidence subtitle: {:?}", subtitle_evidence);
    }

    if !merge_done {
        checkpoint.merge.completed = false;
        save_checkpoint(&state_path, &checkpoint)?;
    }
    if merge_done {
        println!("  [MERGE: SKIP] Giữ nguyên file đã merge {:?}", merged_vtt);
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
                    "\n{ANSI_GREEN}[MERGE: DONE]{ANSI_RESET} {} cues ({} subtitle + {} caption); raw VTT: {:?}",
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
        if let Err(error) = process_video(video, &args, &progress_state) {
            eprintln!(
                "{ANSI_RED}[ERROR]{ANSI_RESET} {}: {error:#}",
                video.display()
            );
            failures.push(video.display().to_string());
        }
    }
    if !failures.is_empty() {
        bail!(
            "{} / {} video xử lý thất bại: {}",
            failures.len(),
            files.len(),
            failures.join(", ")
        );
    }
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
        set_phase_progress(&state, true, 100.0, PhaseStatus::Complete);
        set_phase_progress(&state, false, 100.0, PhaseStatus::Warning);
        let state = state.lock().unwrap();
        assert_eq!(state.subtitle_percent, 100.0);
        assert_eq!(state.caption_percent, 100.0);
        assert_eq!(state.subtitle_status, PhaseStatus::Complete);
        assert_eq!(state.caption_status, PhaseStatus::Warning);
    }

    #[test]
    fn progress_callback_tracks_subtitle_and_caption_separately() {
        let state = register_progress_handler();
        ocr::emit_task_progress("OCR Video Subtitles", 25, 100, "frames", "2s", "25%");
        ocr::emit_task_progress("Caption Phase 1", 50, 100, "frames", "5s", "50%");

        let state = state.lock().unwrap();
        assert_eq!(state.subtitle_percent, 25.0);
        assert_eq!(state.caption_percent, 50.0);
        assert_eq!(state.elapsed, "5s");
        println!();
    }
}
