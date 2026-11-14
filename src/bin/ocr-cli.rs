use anyhow::{Context, Result, bail};
use image::RgbImage;
use ocr::{
    CaptionExtractorConfig, OcrExtractorConfig, SubtitleRoi, SubtitleSegment, extract_captions,
    extract_hardcoded_subtitles, merge_vtt_files_with_line_overrides,
};
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::Read;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "ts", "flv", "webm"];

struct CliArgs {
    input: PathBuf,
    output: PathBuf,
    models: PathBuf,
    caption_evidence: bool,
    subtitle_line: Option<String>,
    caption_line: Option<String>,
    subtitle_roi: SubtitleRoi,
    caption_roi: (f32, f32, f32, f32),
}

#[derive(Default)]
struct ProgressState {
    subtitle_percent: f32,
    caption_percent: f32,
    elapsed: String,
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

            let subtitle_filled = ((state.subtitle_percent / 100.0 * bar_width as f32).round()
                as usize)
                .min(bar_width);
            let caption_filled = ((state.caption_percent / 100.0 * bar_width as f32).round()
                as usize)
                .min(bar_width);
            let subtitle_bar = filled_char.repeat(subtitle_filled)
                + &empty_char.repeat(bar_width - subtitle_filled);
            let caption_bar =
                filled_char.repeat(caption_filled) + &empty_char.repeat(bar_width - caption_filled);

            print!(
                "\r [SUB] [{}] {:5.1}% | [CAP] [{}] {:5.1}% | {}   ",
                subtitle_bar,
                state.subtitle_percent,
                caption_bar,
                state.caption_percent,
                state.elapsed
            );
            let _ = io::stdout().flush();
        }
    });
    state
}

fn reset_progress(state: &Mutex<ProgressState>) {
    if let Ok(mut state) = state.lock() {
        *state = ProgressState::default();
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
  -h, --help                     Show this help\n\
\n\
Each video gets a sibling .tmp_<stem> directory containing subtitles.zh.vtt and\n\
captions.zh.vtt. Input folders are scanned non-recursively. A positional input\n\
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

    println!("[OCR] {} -> {:?}", video.display(), merged_vtt);
    reset_progress(progress_state);
    let subtitle_result =
        extract_hardcoded_subtitles(video, &subtitle_vtt, &args.models, &subtitle_config);
    println!();
    let subtitle_output =
        subtitle_result.with_context(|| format!("Trích xuất subtitle thất bại cho {:?}", video))?;
    let caption_result = extract_captions(video, &caption_vtt, &args.models, &caption_config);
    println!();
    let caption_output =
        caption_result.with_context(|| format!("Trích xuất caption thất bại cho {:?}", video))?;

    if args.caption_evidence {
        export_subtitle_evidence(
            video,
            &subtitle_evidence,
            &subtitle_config,
            &subtitle_output.segments,
        )?;
        println!(
            "  Evidence subtitle: {:?}; caption: {:?}",
            subtitle_evidence, caption_output.evidence_dir
        );
    }

    let summary = merge_vtt_files_with_line_overrides(
        &subtitle_vtt,
        &caption_vtt,
        &merged_vtt,
        args.subtitle_line.as_deref(),
        args.caption_line.as_deref(),
    )?;
    println!(
        "  OK: {} cues ({} subtitle + {} caption); raw VTT: {:?}",
        summary.total_merged_cues, summary.subtitle_cues, summary.caption_cues, temp_dir
    );
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
            eprintln!("[ERROR] {}: {error:#}", video.display());
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
        ]
        .map(str::to_string);
        let parsed = parse_args_from(args.into_iter()).unwrap().unwrap();
        assert_eq!(parsed.input, PathBuf::from("videos"));
        assert_eq!(parsed.output, PathBuf::from("merged"));
        assert_eq!(parsed.subtitle_line.as_deref(), Some("85%"));

        let positional = ["videos", "--output", "merged"].map(str::to_string);
        assert!(parse_args_from(positional.into_iter()).unwrap().is_some());

        let conflicting =
            ["--input", "videos", "other.mp4", "--output", "merged"].map(str::to_string);
        assert!(parse_args_from(conflicting.into_iter()).is_err());
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
