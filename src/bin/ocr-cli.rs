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
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

fn usage() -> &'static str {
    "OCR-only video extractor and WebVTT merger\n\
\n\
Usage:\n\
  ocr-cli <input-file-or-folder> --output <folder> [options]\n\
\n\
Options:\n\
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
captions.zh.vtt. Input folders are scanned non-recursively."
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

fn parse_args() -> Result<Option<CliArgs>> {
    let mut args = env::args().skip(1);
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

fn process_video(video: &Path, args: &CliArgs) -> Result<()> {
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
    let subtitle_output =
        extract_hardcoded_subtitles(video, &subtitle_vtt, &args.models, &subtitle_config)
            .with_context(|| format!("Trích xuất subtitle thất bại cho {:?}", video))?;
    let caption_output = extract_captions(video, &caption_vtt, &args.models, &caption_config)
        .with_context(|| format!("Trích xuất caption thất bại cho {:?}", video))?;

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
    let mut failures = Vec::new();
    for video in &files {
        if let Err(error) = process_video(video, &args) {
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
}
