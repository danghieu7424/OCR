#![allow(non_snake_case)]
/****
 * [MODULE]: OCR (Video Subtitle Optical Character Recognition) - Industrial Grade
 * Chức năng: Trích xuất phụ đề cứng (Hardcoded Subtitles) từ video sang chuẩn WebVTT (.vtt)
 *
 * Quy trình 4 bước chuẩn công nghiệp:
 * 1. ROI Adaptive Cropping & High-Precision FFmpeg Streaming (fps = 10-12, rawvideo rgb24).
 * 2. Deep Learning Text Detection (DBNet) & Text Line Clustering.
 * 3. PP-OCRv6 CTC Recognition với chuẩn hóa PaddleOCR ([-1.0, 1.0] BGR).
 * 4. Temporal State Machine: Frame Denoising, Text Transition Boundary Detection & Majority Voting.
 ****/

use anyhow::{Context, Result, bail};
use image::RgbImage;
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ort::{session::Session, session::builder::GraphOptimizationLevel, value::Tensor};

use ocr_shared::icons::Icons;
use ocr_shared::terminal_color::NameUtilsExt;

pub mod det_filter;
pub mod temporal_voting;
pub use temporal_voting::*;

/// Cấu hình tọa độ vùng ROI (Region of Interest)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubtitleRoi {
    /// Tỷ lệ biên trên (mặc định: 0.75 = 75%)
    pub y_min: f32,
    /// Tỷ lệ biên dưới (mặc định: 1.00 = 100%)
    pub y_max: f32,
    /// Tỷ lệ biên trái (mặc định: 0.05 = 5%)
    pub x_min: f32,
    /// Tỷ lệ biên phải (mặc định: 0.95 = 95%)
    pub x_max: f32,
}

impl Default for SubtitleRoi {
    fn default() -> Self {
        Self {
            y_min: 0.85,
            y_max: 1.00,
            x_min: 0.05,
            x_max: 0.95,
        }
    }
}

impl SubtitleRoi {
    #[allow(dead_code)]
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        if parts.len() != 4 {
            return None;
        }
        let y_min = parts[0].parse::<f32>().ok()?;
        let y_max = parts[1].parse::<f32>().ok()?;
        let x_min = parts[2].parse::<f32>().ok()?;
        let x_max = parts[3].parse::<f32>().ok()?;
        Some(Self {
            y_min,
            y_max,
            x_min,
            x_max,
        })
    }
}

/// Cấu hình trích xuất phụ đề video
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OcrExtractorConfig {
    /// Tần số lấy mẫu khung hình (mặc định: 10 fps cho độ chính xác cao < 50ms)
    pub fps: u32,
    /// Tọa độ vùng cắt chữ
    pub roi: SubtitleRoi,
    /// Ngôn ngữ nhận diện ưu tiên ("zh", "en", "auto")
    pub lang: String,
    /// Ngưỡng phát hiện biến thiên (giữ lại tương thích)
    pub diff_threshold: f32,
    /// Chế độ chính xác
    pub accuracy_mode: bool,
}

impl Default for OcrExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 10,
            roi: SubtitleRoi::default(),
            lang: "zh".to_string(),
            diff_threshold: 0.04,
            accuracy_mode: true,
        }
    }
}

/// Thông tin một phân đoạn phụ đề được trích xuất
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleSegment {
    /// Thời điểm bắt đầu (giây)
    pub start_sec: f64,
    /// Thời điểm kết thúc (giây)
    pub end_sec: f64,
    /// Nội dung văn bản phụ đề
    pub text: String,
}

#[derive(Debug, Clone)]
struct TrackedSubtitleSegment {
    segment: SubtitleSegment,
    confidence: f32,
    best_frame_idx: usize,
}

impl SubtitleSegment {
    /// Định dạng thời gian sang chuỗi WebVTT (hh:mm:ss.mmm)
    pub fn format_vtt_timestamp(seconds: f64) -> String {
        let total_ms = (seconds * 1000.0).round() as u64;
        let ms = total_ms % 1000;
        let total_sec = total_ms / 1000;
        let sec = total_sec % 60;
        let total_min = total_sec / 60;
        let min = total_min % 60;
        let hours = total_min / 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, min, sec, ms)
    }
}

/// Kết quả đầu ra của module OCR
#[derive(Debug, Clone)]
pub struct OcrExtractorOutput {
    /// Đường dẫn file phụ đề WebVTT được tạo ra
    pub vtt_path: PathBuf,
    /// Danh sách các phân đoạn phụ đề
    pub segments: Vec<SubtitleSegment>,
}

fn export_fallback_crops(
    video_path: &Path,
    output_dir: &Path,
    fps: u32,
    threshold: f32,
    crop: (usize, usize, usize, usize),
    segments: &[TrackedSubtitleSegment],
) -> Result<()> {
    let (crop_width, crop_height, crop_x, crop_y) = crop;
    fs::create_dir_all(output_dir)?;
    let mut targets: HashMap<usize, Vec<usize>> = HashMap::new();
    for (cue_index, cue) in segments.iter().enumerate() {
        if cue.confidence < threshold {
            targets
                .entry(cue.best_frame_idx)
                .or_default()
                .push(cue_index);
        }
    }

    let manifest_path = output_dir.join("subtitles.tsv");
    let mut manifest = String::from("cue_index\tstart\tend\tconfidence\tbest_frame_idx\tpath\n");
    if targets.is_empty() {
        for (index, cue) in segments.iter().enumerate() {
            manifest.push_str(&format!(
                "{}\t{}\t{}\t{:.4}\t{}\t\n",
                index + 1,
                SubtitleSegment::format_vtt_timestamp(cue.segment.start_sec),
                SubtitleSegment::format_vtt_timestamp(cue.segment.end_sec),
                cue.confidence,
                cue.best_frame_idx
            ));
        }
        fs::write(manifest_path, manifest)?;
        return Ok(());
    }

    let mut target_frames = targets.keys().copied().collect::<Vec<_>>();
    target_frames.sort_unstable();
    let select = target_frames
        .iter()
        .map(|frame| format!("eq(n\\,{frame})"))
        .collect::<Vec<_>>()
        .join("+");
    let filter =
        format!("fps={fps},crop={crop_width}:{crop_height}:{crop_x}:{crop_y},select={select}");
    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-i",
            &video_path.to_string_lossy(),
            "-vf",
            &filter,
            "-vsync",
            "0",
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
        .context("Không thể khởi động FFmpeg để xuất fallback subtitle crops")?;
    let mut stdout = ffmpeg
        .stdout
        .take()
        .context("Không thể mở stdout FFmpeg fallback")?;
    let frame_bytes = crop_width
        .checked_mul(crop_height)
        .and_then(|size| size.checked_mul(3))
        .context("Kích thước subtitle crop vượt giới hạn")?;
    let mut buffer = vec![0u8; frame_bytes];
    let mut frame_index = 0usize;
    loop {
        match stdout.read_exact(&mut buffer) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(error.into()),
        }
        let target_frame = target_frames.get(frame_index).copied();
        if let Some(target_frame) = target_frame {
            let image = RgbImage::from_raw(crop_width as u32, crop_height as u32, buffer.clone())
                .context("Không thể tạo fallback subtitle crop")?;
            for &cue_index in &targets[&target_frame] {
                let file_name = format!("sub_{:03}.png", cue_index + 1);
                image.save(output_dir.join(&file_name))?;
            }
        }
        frame_index += 1;
    }
    let status = ffmpeg.wait().context("Không thể chờ FFmpeg fallback")?;
    if !status.success() {
        bail!("FFmpeg lỗi khi xuất fallback subtitle crops: {status}");
    }
    if frame_index != targets.len() {
        bail!(
            "Thiếu fallback subtitle crop: xuất {frame_index}/{} frame mục tiêu",
            targets.len()
        );
    }
    for (index, cue) in segments.iter().enumerate() {
        let file_name = if cue.confidence < threshold {
            format!("sub_{:03}.png", index + 1)
        } else {
            String::new()
        };
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{:.4}\t{}\t{}\n",
            index + 1,
            SubtitleSegment::format_vtt_timestamp(cue.segment.start_sec),
            SubtitleSegment::format_vtt_timestamp(cue.segment.end_sec),
            cue.confidence,
            cue.best_frame_idx,
            file_name
        ));
    }
    fs::write(manifest_path, manifest)?;
    Ok(())
}

/// Bộ máy nhận dạng chữ PP-OCRv6 bằng ONNX Runtime
struct PpOcrEngine {
    rec_session: Session,
    keys: Vec<String>,
}

impl PpOcrEngine {
    fn new(model_dir: &Path) -> Result<Self> {
        let ocr_dir = model_dir.join("ocr");
        let rec_model_path = ocr_dir.join("rec.onnx");
        let keys_path = ocr_dir.join("keys.txt");

        if !rec_model_path.exists() || !keys_path.exists() {
            bail!(
                "Thiếu tài nguyên mô hình OCR tại {:?}. Vui lòng kiểm tra rec.onnx và keys.txt",
                ocr_dir
            );
        }

        let keys_content = fs::read_to_string(&keys_path)
            .with_context(|| format!("Không thể nạp từ điển ký tự OCR: {:?}", keys_path))?;
        let mut keys: Vec<String> = keys_content.lines().map(|l| l.to_string()).collect();
        // PP-OCR CTC mapping: index 0 là blank, cuối cùng là space
        keys.insert(0, "<blank>".to_string());
        keys.push(" ".to_string());

        let threads = ocr_shared::CpuOptimizer::single_task_threads().max(2);
        let rec_session = Session::builder()
            .map_err(|e| anyhow::anyhow!("Builder error: {:?}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| anyhow::anyhow!("Opt error: {:?}", e))?
            .with_intra_threads(threads)
            .map_err(|e| anyhow::anyhow!("Thread error: {:?}", e))?
            .commit_from_file(&rec_model_path)
            .with_context(|| format!("Không thể nạp mô hình rec.onnx: {:?}", rec_model_path))?;

        Ok(Self { rec_session, keys })
    }

    /// Nhận diện chuỗi text từ patch ảnh raw RGB24
    fn recognize_patch(
        &mut self,
        raw_patch: &[u8],
        target_w: usize,
        target_h: usize,
    ) -> Result<String> {
        if target_w == 0 || target_h == 0 || raw_patch.is_empty() {
            return Ok(String::new());
        }

        // Chuẩn hóa PaddleOCR chuẩn công nghiệp: (pixel / 127.5) - 1.0 trên dải kênh BGR
        let mut input_tensor = vec![0.0f32; 3 * target_h * target_w];
        for y in 0..target_h {
            for x in 0..target_w {
                let idx = (y * target_w + x) * 3;
                let r = raw_patch[idx] as f32;
                let g = raw_patch[idx + 1] as f32;
                let b = raw_patch[idx + 2] as f32;

                // Kênh 0: B, Kênh 1: G, Kênh 2: R (Chuẩn BGR)
                input_tensor[y * target_w + x] = b / 127.5 - 1.0;
                input_tensor[target_h * target_w + y * target_w + x] = g / 127.5 - 1.0;
                input_tensor[2 * target_h * target_w + y * target_w + x] = r / 127.5 - 1.0;
            }
        }

        let tensor_val = Tensor::from_array(([1usize, 3usize, target_h, target_w], input_tensor))?;
        let outputs = self
            .rec_session
            .run(ort::inputs![tensor_val])
            .map_err(|e| anyhow::anyhow!("Lỗi suy luận ONNX OCR: {:?}", e))?;
        let (shape, logits) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("Lỗi trích xuất tensor output OCR: {:?}", e))?;

        let time_steps = shape[1] as usize;
        let num_classes = shape[2] as usize;

        // CTC Greedy Decoder
        let mut decoded_text = String::new();
        let mut last_idx = 0usize;

        for t in 0..time_steps {
            let step_offset = t * num_classes;
            let mut max_idx = 0usize;
            let mut max_val = f32::NEG_INFINITY;

            for c in 0..num_classes {
                let val = logits[step_offset + c];
                if val > max_val {
                    max_val = val;
                    max_idx = c;
                }
            }

            if max_idx != 0 && max_idx != last_idx && max_idx < self.keys.len() {
                let ch = &self.keys[max_idx];
                if ch != "<blank>" {
                    decoded_text.push_str(ch);
                }
            }
            last_idx = max_idx;
        }

        Ok(sanitize_ocr_text(&decoded_text))
    }
}

/****
 * sanitize_ocr_text
 * Làm sạch chuỗi ký tự nhận diện, sửa lỗi nhận nhầm nét gạch ngang thành chữ '一'
 * và chuẩn hóa các ký tự dị thể/phồn thể thông dụng sang giản thể chuẩn.
 ****/
pub fn sanitize_ocr_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let cleaned = trimmed
        .trim_matches(|c: char| {
            c.is_whitespace()
                || c == '-'
                || c == '—'
                || c == '–'
                || c == '_'
                || c == '<'
                || c == '>'
                || c == '《'
                || c == '》'
                || c == '【'
                || c == '】'
                || c == '\u{201c}'
                || c == '\u{201d}'
                || c == '"'
                || c == '\''
                || c == '#'
                || c == '*'
                || c == '+'
                || c == '='
                || c == '|'
                || c == '~'
                || c == ','
                || c == '，'
                || c == '.'
                || c == '。'
                || c == '!'
                || c == '！'
                || c == '?'
                || c == '？'
                || c == ':'
                || c == '：'
                || c == ';'
                || c == '；'
        })
        .trim()
        .to_string();

    // Bỏ kana: phụ đề tiếng Hán không dùng kana, ký tự này chỉ là rác nhận diện
    // (vd "魔域之人で" — CTC đọc nhầm nét cuối thành で).
    let mut final_chars: Vec<char> = cleaned
        .chars()
        .filter(|&c| {
            !(('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{31f0}'..='\u{31ff}').contains(&c))
        })
        .collect();
    if final_chars.len() >= 2 {
        if final_chars[0].is_ascii_alphabetic()
            && ('\u{4e00}'..='\u{9fff}').contains(&final_chars[1])
        {
            final_chars.remove(0);
        }
        // Ký tự Latin lẻ ở cuối cũng là rác (vd "魔域V")
        let last = final_chars.len() - 1;
        if final_chars[last].is_ascii_alphabetic()
            && ('\u{4e00}'..='\u{9fff}').contains(&final_chars[last - 1])
        {
            final_chars.pop();
        }
    }

    // Phụ đề tiếng Hán yêu cầu chứa ít nhất 1 ký tự CJK
    let has_cjk = final_chars
        .iter()
        .any(|&c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk {
        return String::new();
    }

    // Giữ nguyên 100% ký tự nhận diện chuẩn từ AI mô hình, tuyệt đối không hardcode thay thế
    final_chars.into_iter().collect()
}

#[cfg(test)]
mod sanitize_tests {
    use super::sanitize_ocr_text;

    #[test]
    fn leading_dash_is_removed_without_inserting_a_han_character() {
        assert_eq!(
            sanitize_ocr_text("-还只是比你强一点点"),
            "还只是比你强一点点"
        );
        assert_eq!(
            sanitize_ocr_text("一还只是比你强一点点"),
            "一还只是比你强一点点"
        );
    }
}

#[cfg(test)]
mod observation_tests {
    use super::{
        has_stable_short_subtitle_track, has_stable_single_character_track,
        is_same_subtitle_observation, select_best_subtitle_text,
    };

    #[test]
    fn temporal_voting_prefers_the_higher_confidence_character() {
        let texts = vec!["参".to_string(), "爹".to_string(), "参".to_string()];
        let confidence = [0.2, 0.95, 0.2];
        assert_eq!(select_best_subtitle_text(&texts, &confidence), "爹");
    }

    #[test]
    fn temporal_character_support_removes_unstable_edge_characters() {
        let texts = vec![
            "小啊".to_string(),
            "啊".to_string(),
            "啊".to_string(),
            "啊".to_string(),
        ];
        assert_eq!(
            select_best_subtitle_text(&texts, &[0.9, 0.9, 0.9, 0.9]),
            "啊"
        );

        let texts = vec![
            "可还有遗言要说光".to_string(),
            "可还有遗言要说光".to_string(),
            "可还有遗言要说".to_string(),
            "可还有遗言要说".to_string(),
            "可还有遗言要说".to_string(),
        ];
        assert_eq!(
            select_best_subtitle_text(&texts, &[0.9; 5]),
            "可还有遗言要说"
        );
    }

    #[test]
    fn temporal_character_support_preserves_a_repeated_thin_edge_character() {
        let texts = vec![
            "一路上所有虚空都得被你破坏".to_string(),
            "一路上所有虚空都得被你破坏".to_string(),
            "一路上所有虚空都得被你破坏".to_string(),
            "一路上所有虚空都得被你破坏".to_string(),
            "路上所有虚空都得被你破坏".to_string(),
            "路上所有虚空都得被你破坏".to_string(),
        ];
        assert_eq!(
            select_best_subtitle_text(&texts, &[0.9; 6]),
            "一路上所有虚空都得被你破坏"
        );
    }

    #[test]
    fn short_multicharacter_tracks_require_stable_confidence_and_geometry() {
        assert!(has_stable_short_subtitle_track(
            &[
                Some((100.0, 40.0, 200.0, 90.0)),
                Some((102.0, 41.0, 202.0, 91.0)),
            ],
            &[0.9, 0.85],
            Some(50.0),
            2,
        ));
        assert!(!has_stable_short_subtitle_track(
            &[
                Some((100.0, 40.0, 200.0, 90.0)),
                Some((102.0, 41.0, 202.0, 91.0)),
            ],
            &[0.65, 0.70],
            Some(50.0),
            2,
        ));
        assert!(!has_stable_short_subtitle_track(
            &[
                Some((100.0, 40.0, 200.0, 90.0)),
                Some((500.0, 40.0, 600.0, 90.0)),
            ],
            &[0.9, 0.9],
            Some(50.0),
            2,
        ));
    }

    #[test]
    fn edge_character_variants_join_only_on_the_same_box_edge() {
        assert!(is_same_subtitle_observation(
            "小啊",
            "啊",
            Some((100.0, 20.0, 200.0, 60.0)),
            Some((150.0, 20.0, 200.0, 60.0)),
        ));
        assert!(!is_same_subtitle_observation(
            "小啊",
            "啊",
            Some((100.0, 20.0, 200.0, 60.0)),
            Some((150.0, 90.0, 200.0, 130.0)),
        ));
    }

    #[test]
    fn single_character_requires_a_stable_detected_region() {
        assert!(has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 30.0, 30.0)),
                Some((11.0, 10.0, 31.0, 30.0)),
            ],
            &[0.95, 0.95],
            Some(20.0),
        ));
        assert!(!has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 30.0, 30.0)),
                Some((60.0, 60.0, 80.0, 80.0)),
            ],
            &[0.95, 0.95],
            Some(20.0),
        ));
    }

    #[test]
    fn single_character_rejects_broad_regions_and_low_confidence_tracks() {
        assert!(!has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 760.0, 70.0)),
                Some((15.0, 12.0, 765.0, 72.0)),
            ],
            &[0.95, 0.95],
            Some(60.0),
        ));
        assert!(!has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 30.0, 30.0)),
                Some((11.0, 10.0, 31.0, 30.0)),
                Some((12.0, 10.0, 32.0, 30.0)),
            ],
            &[0.40, 0.54, 1.00],
            Some(20.0),
        ));
        assert!(!has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 34.0, 34.0)),
                Some((11.0, 10.0, 35.0, 34.0)),
            ],
            &[0.95, 0.95],
            Some(80.0),
        ));
    }

    #[test]
    fn single_character_confidence_ignores_interpolated_frames_without_boxes() {
        assert!(has_stable_single_character_track(
            &[
                Some((10.0, 10.0, 30.0, 30.0)),
                Some((11.0, 10.0, 31.0, 30.0)),
                None,
            ],
            &[0.95, 0.95, 0.0],
            Some(20.0),
        ));
    }

    #[test]
    fn single_character_keeps_a_track_with_a_few_low_confidence_frames() {
        let bounds = [Some((10.0, 10.0, 30.0, 30.0)); 8];
        let confidences = [0.824, 0.576, 0.823, 0.642, 0.529, 0.776, 0.798, 0.741];
        assert!(has_stable_single_character_track(
            &bounds,
            &confidences,
            Some(20.0),
        ));
    }
}

/****
 * levenshtein_distance
 * Tính khoảng cách chỉnh sửa giữa 2 chuỗi ký tự (giữ nguyên trật tự tuần tự)
 ****/
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();
    let len1 = chars1.len();
    let len2 = chars2.len();
    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev: Vec<usize> = (0..=len2).collect();
    let mut curr: Vec<usize> = vec![0; len2 + 1];

    for i in 1..=len1 {
        curr[0] = i;
        for j in 1..=len2 {
            let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    curr[len2]
}

/****
 * common_prefix_len
 * Tính độ dài tiền tố chung giữa 2 chuỗi ký tự
 ****/
fn common_prefix_len(s1: &str, s2: &str) -> usize {
    s1.chars()
        .zip(s2.chars())
        .take_while(|(c1, c2)| c1 == c2)
        .count()
}

/****
 * common_suffix_len
 * Tính độ dài hậu tố chung giữa 2 chuỗi ký tự
 ****/
fn common_suffix_len(s1: &str, s2: &str) -> usize {
    let rev1: Vec<char> = s1.chars().rev().collect();
    let rev2: Vec<char> = s2.chars().rev().collect();
    rev1.iter()
        .zip(rev2.iter())
        .take_while(|(c1, c2)| c1 == c2)
        .count()
}

/****
 * is_same_subtitle
 * Kiểm tra xem 2 khung hình nhận diện có cùng một câu thoại hay không.
 * Kết hợp Levenshtein Sequence Matching và Common Suffix/Prefix Matching.
 * Đảm bảo:
 * 1. Tuyệt đối không gộp các câu 1 chữ khác nhau (bảo toàn 100% sub 1 chữ như 爹, 嗯).
 * 2. Tuyệt đối không gộp 2 câu thoại khác nhau liên tiếp (như cụm Tập 4).
 * 3. Tự động nhận diện và gộp các biến thể chữ bị nhòe nét đầu/cuối (như "爹爹我们走" vs "珍我们走" vs "多爹我们走").
 ****/
fn is_same_subtitle(s1: &str, s2: &str) -> bool {
    if s1 == s2 {
        return true;
    }
    if s1.is_empty() || s2.is_empty() {
        return false;
    }
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let max_len = len1.max(len2);
    let min_len = len1.min(len2);

    // BẢO VỆ TUYỆT ĐỐI SUB 1 CHỮ: nếu có câu 1 chữ thì bắt buộc phải trùng khớp hoàn toàn
    if min_len == 1 {
        return s1 == s2;
    }

    // Nếu một câu là chuỗi con của câu kia (ví dụ rụng chữ '一' ở đầu câu: "定是错觉" trong "一定是错觉")
    if (s1.contains(s2) || s2.contains(s1)) && max_len - min_len <= 2 {
        return true;
    }

    // Kiểm tra trùng khớp Tiền tố / Hậu tố (Prefix/Suffix) cho câu >= 4 chữ:
    // Khắc phục triệt để lỗi rung nét đầu/cuối của OCR (ví dụ: "爹爹我们走" vs "珍我们走" vs "多爹我们走")
    if min_len >= 4 {
        let pref = common_prefix_len(s1, s2);
        let suff = common_suffix_len(s1, s2);
        let common_fix = pref.max(suff);
        if common_fix >= 2 {
            let ratio = common_fix as f32 / min_len as f32;
            if ratio >= 0.55 && max_len - min_len <= 2 {
                return true;
            }
        }
    }

    let dist = levenshtein_distance(s1, s2);

    // Đối với câu ngắn (<= 3 ký tự), bắt buộc phải khớp tuyệt đối
    if min_len <= 3 {
        dist == 0
    } else if min_len <= 5 {
        dist <= 1
    } else {
        let sim = 1.0 - (dist as f32 / max_len as f32);
        sim >= 0.70 && dist <= 2
    }
}

fn is_same_subtitle_observation(
    s1: &str,
    s2: &str,
    bounds1: Option<(f64, f64, f64, f64)>,
    bounds2: Option<(f64, f64, f64, f64)>,
) -> bool {
    if is_same_subtitle(s1, s2) {
        return true;
    }
    let (Some(a), Some(b)) = (bounds1, bounds2) else {
        return false;
    };
    let (longer, shorter) = if s1.chars().count() > s2.chars().count() {
        (s1, s2)
    } else {
        (s2, s1)
    };
    if longer.chars().count() - shorter.chars().count() > 2 {
        return false;
    }

    let aligns_left =
        longer.starts_with(shorter) && (a.0 - b.0).abs() <= (a.2 - a.0).max(b.2 - b.0) * 0.25;
    let aligns_right =
        longer.ends_with(shorter) && (a.2 - b.2).abs() <= (a.2 - a.0).max(b.2 - b.0) * 0.25;
    let overlap_y = (a.3.min(b.3) - a.1.max(b.1)).max(0.0);
    let min_height = (a.3 - a.1).min(b.3 - b.1);
    let horizontal_overlap = (a.2.min(b.2) - a.0.max(b.0)).max(0.0);
    let min_width = (a.2 - a.0).min(b.2 - b.0);

    min_height > 0.0
        && min_width > 0.0
        && overlap_y / min_height >= 0.6
        && horizontal_overlap / min_width >= 0.6
        && (aligns_left || aligns_right)
}

/****
 * select_best_subtitle_text
 * Thuật toán Majority Voting thông minh cho OCR:
 * 1. Khắc phục triệt để lỗi rụng ký tự nét mỏng ở mép (như chữ '一' trong "一定是错觉").
 * 2. Ưu tiên câu hoàn chỉnh, đúng ngữ nghĩa có chung hậu tố/tiền tố (như "爹爹我们走" > "珍我们走" / "多爹我们走").
 ****/
fn select_best_subtitle_text(texts: &[String], weights: &[f32]) -> String {
    if texts.is_empty() {
        return String::new();
    }
    let default_weights;
    let weights = if texts.len() != weights.len()
        || !weights
            .iter()
            .any(|weight| weight.is_finite() && *weight > 0.0)
    {
        default_weights = vec![1.0; texts.len()];
        default_weights.as_slice()
    } else {
        weights
    };

    let mut counts: HashMap<String, (usize, f32)> = HashMap::new();
    for (text, weight) in texts.iter().zip(weights) {
        let entry = counts.entry(text.clone()).or_default();
        entry.0 += 1;
        entry.1 += if weight.is_finite() {
            weight.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    let total_weight = counts.values().map(|(_, weight)| weight).sum::<f32>();
    let mut candidates: Vec<(&String, usize, f32, f32)> = counts
        .iter()
        .map(|(candidate, (count, weight))| {
            let chars: Vec<char> = candidate.chars().collect();
            let mut support = vec![0.0f32; chars.len()];
            for (other, &(_, other_weight)) in &counts {
                for position in lcs_candidate_positions(candidate, other) {
                    support[position] += other_weight;
                }
            }

            let mut score = 0.0f32;
            for character_support in support {
                let ratio = if total_weight > 0.0 {
                    character_support / total_weight
                } else {
                    0.0
                };
                score += if ratio >= 0.60 { ratio } else { -0.25 };
            }
            if total_weight > 0.0 {
                score += 0.35 * weight / total_weight;
            }
            score += *count as f32 * 0.001;
            (candidate, *count, *weight, score)
        })
        .collect();
    candidates.sort_by(|a, b| {
        a.3.total_cmp(&b.3)
            .then_with(|| a.2.total_cmp(&b.2))
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.0.cmp(b.0))
    });
    candidates
        .last()
        .map(|(text, _, _, _)| (*text).clone())
        .unwrap_or_default()
}

fn lcs_candidate_positions(candidate: &str, other: &str) -> Vec<usize> {
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let other_chars: Vec<char> = other.chars().collect();
    let mut lengths = vec![vec![0usize; other_chars.len() + 1]; candidate_chars.len() + 1];
    for (i, candidate_char) in candidate_chars.iter().enumerate() {
        for (j, other_char) in other_chars.iter().enumerate() {
            lengths[i + 1][j + 1] = if candidate_char == other_char {
                lengths[i][j] + 1
            } else {
                lengths[i][j + 1].max(lengths[i + 1][j])
            };
        }
    }

    let mut positions = Vec::new();
    let (mut i, mut j) = (candidate_chars.len(), other_chars.len());
    while i > 0 && j > 0 {
        if candidate_chars[i - 1] == other_chars[j - 1] {
            positions.push(i - 1);
            i -= 1;
            j -= 1;
        } else if lengths[i - 1][j] >= lengths[i][j - 1] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    positions
}

fn bounds_iou(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let intersection_w = (a.2.min(b.2) - a.0.max(b.0)).max(0.0);
    let intersection_h = (a.3.min(b.3) - a.1.max(b.1)).max(0.0);
    let intersection = intersection_w * intersection_h;
    let area_a = (a.2 - a.0).max(0.0) * (a.3 - a.1).max(0.0);
    let area_b = (b.2 - b.0).max(0.0) * (b.3 - b.1).max(0.0);
    let union = area_a + area_b - intersection;
    if union > 0.0 {
        intersection / union
    } else {
        0.0
    }
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return None;
    }

    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

fn has_stable_single_character_track(
    bounds: &[Option<(f64, f64, f64, f64)>],
    confidences: &[f32],
    typical_glyph_height: Option<f64>,
) -> bool {
    const MIN_MEDIAN_CONFIDENCE: f32 = 0.65;
    const MAX_ASPECT_RATIO: f64 = 2.5;
    const MIN_HEIGHT_RATIO: f64 = 0.5;
    const MAX_HEIGHT_RATIO: f64 = 1.5;

    let mut observed_confidences: Vec<f32> = bounds
        .iter()
        .zip(confidences)
        .filter_map(|(bounds, &confidence)| bounds.map(|_| confidence))
        .collect();
    if observed_confidences.is_empty()
        || observed_confidences
            .iter()
            .any(|confidence| !confidence.is_finite())
    {
        return false;
    }

    observed_confidences.sort_by(f32::total_cmp);
    let middle = observed_confidences.len() / 2;
    let median_confidence = if observed_confidences.len().is_multiple_of(2) {
        (observed_confidences[middle - 1] + observed_confidences[middle]) / 2.0
    } else {
        observed_confidences[middle]
    };
    if median_confidence < MIN_MEDIAN_CONFIDENCE {
        return false;
    }

    bounds.windows(2).any(|pair| {
        pair[0].zip(pair[1]).is_some_and(|(a, b)| {
            let glyph_shaped = |rect: (f64, f64, f64, f64)| {
                let width = rect.2 - rect.0;
                let height = rect.3 - rect.1;
                let height_consistent = match typical_glyph_height {
                    Some(reference_height) => {
                        let relative_height = height / reference_height;
                        (MIN_HEIGHT_RATIO..=MAX_HEIGHT_RATIO).contains(&relative_height)
                    }
                    None => true,
                };
                width > 0.0
                    && height > 0.0
                    && (width / height).max(height / width) <= MAX_ASPECT_RATIO
                    && height_consistent
            };
            glyph_shaped(a) && glyph_shaped(b) && bounds_iou(a, b) >= 0.25
        })
    })
}

fn has_stable_short_subtitle_track(
    bounds: &[Option<(f64, f64, f64, f64)>],
    confidences: &[f32],
    typical_glyph_height: Option<f64>,
    char_count: usize,
) -> bool {
    const MIN_MEDIAN_CONFIDENCE: f32 = 0.75;
    const MIN_HEIGHT_RATIO: f64 = 0.5;
    const MAX_HEIGHT_RATIO: f64 = 1.5;

    let mut observed_confidences: Vec<f32> = bounds
        .iter()
        .zip(confidences)
        .filter_map(|(rect, &confidence)| rect.map(|_| confidence))
        .collect();
    if observed_confidences.len() < 2
        || observed_confidences
            .iter()
            .any(|confidence| !confidence.is_finite())
    {
        return false;
    }
    observed_confidences.sort_by(f32::total_cmp);
    let middle = observed_confidences.len() / 2;
    let median_confidence = if observed_confidences.len().is_multiple_of(2) {
        (observed_confidences[middle - 1] + observed_confidences[middle]) / 2.0
    } else {
        observed_confidences[middle]
    };
    if median_confidence < MIN_MEDIAN_CONFIDENCE {
        return false;
    }

    let max_aspect_ratio = (char_count as f64 * 2.5).clamp(2.5, 16.0);
    bounds.windows(2).any(|pair| {
        pair[0].zip(pair[1]).is_some_and(|(a, b)| {
            let glyph_shaped = |rect: (f64, f64, f64, f64)| {
                let width = rect.2 - rect.0;
                let height = rect.3 - rect.1;
                let height_consistent = match typical_glyph_height {
                    Some(reference_height) => {
                        (MIN_HEIGHT_RATIO..=MAX_HEIGHT_RATIO).contains(&(height / reference_height))
                    }
                    None => true,
                };
                width > 0.0
                    && height > 0.0
                    && width / height <= max_aspect_ratio
                    && height_consistent
            };
            glyph_shaped(a) && glyph_shaped(b) && bounds_iou(a, b) >= 0.25
        })
    })
}

pub fn extract_hardcoded_subtitles(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
) -> Result<OcrExtractorOutput> {
    extract_hardcoded_subtitles_inner(video_path, output_vtt_path, model_dir, config, None)
}

pub fn extract_hardcoded_subtitles_with_fallback_crops(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
    fallback_crops_dir: &Path,
    fallback_threshold: f32,
) -> Result<OcrExtractorOutput> {
    extract_hardcoded_subtitles_inner(
        video_path,
        output_vtt_path,
        model_dir,
        config,
        Some((fallback_crops_dir, fallback_threshold)),
    )
}

fn extract_hardcoded_subtitles_inner(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
    fallback: Option<(&Path, f32)>,
) -> Result<OcrExtractorOutput> {
    if !video_path.exists() {
        bail!("Video không tồn tại: {:?}", video_path);
    }

    ocr_shared::tui_events::emit_step(
        ">>> [OCR] Trích xuất Hardcoded Subtitles từ Video (Industrial Engine)...",
    );
    ocr_shared::tui_events::emit_log(format!(
        "   {} ROI: Y[{:.0}% - {:.0}%] | X[{:.0}% - {:.0}%] | Fps: {} | Lang: {}",
        "[OCR]".name_utils().violed(),
        config.roi.y_min * 100.0,
        config.roi.y_max * 100.0,
        config.roi.x_min * 100.0,
        config.roi.x_max * 100.0,
        config.fps,
        config.lang
    ));

    // Bước 0: Nạp mô hình OCR PP-OCRv4/v5
    ocr_shared::tui_events::emit_log(format!(
        "   {} Khởi tạo mô hình ONNX PP-OCRv6 Text Recognition...",
        "[OCR]".name_utils().violed()
    ));
    let mut ocr_engine = PpOcrEngine::new(model_dir)?;

    let det_path = model_dir.join("ocr/det.onnx");
    let rec_path = model_dir.join("ocr/rec.onnx");
    let keys_path = model_dir.join("ocr/keys.txt");

    ocr_shared::tui_events::emit_log(format!(
        "   {} Khởi tạo mô hình DBNet Text Detection...",
        "[OCR]".name_utils().violed()
    ));
    // Cạnh dài tối đa khi đưa ảnh vào DBNet: nhỏ hơn = nhanh hơn, quá nhỏ = mất
    // nét chữ nhỏ. 640 đo trên tập 002/005/006: nhanh hơn ~1.4x, chất lượng bằng
    // hoặc tốt hơn 960 (002: 98.50% → 98.64%, 005: giống hệt, 006: thêm 1 mảnh
    // "挡" của câu 挡住). 320 thì hỏng nhiều câu nên không dùng.
    let det_limit = std::env::var("OCR_SUB_DET_LIMIT")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(640);
    let det_engine = self::det_filter::create_det_engine_with_limit(
        &det_path.to_string_lossy(),
        &rec_path.to_string_lossy(),
        &keys_path.to_string_lossy(),
        det_limit,
    )?;

    // Lấy kích thước video gốc bằng ffprobe
    let probe_out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
            &video_path.to_string_lossy(),
        ])
        .output()
        .context("Không thể thực thi ffprobe để đọc kích thước video")?;

    let dim_str = String::from_utf8_lossy(&probe_out.stdout);
    let dims: Vec<usize> = dim_str
        .trim()
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect();

    let (src_w, src_h) = if dims.len() == 2 {
        (dims[0], dims[1])
    } else {
        (1920usize, 1080usize)
    };

    // Lấy tổng thời lượng video (giây)
    let duration_out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .output();
    let total_duration_sec: f64 = duration_out
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .parse::<f64>()
                .ok()
        })
        .unwrap_or(600.0);

    // Tính kích thước frame crop ROI (làm tròn số chẵn cho FFmpeg)
    let crop_w =
        (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h =
        (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (((src_w as f32 * config.roi.x_min).round() as usize) / 2) * 2;
    let crop_y = (((src_h as f32 * config.roi.y_min).round() as usize) / 2) * 2;

    let vf_filter = format!(
        "fps={},crop={}:{}:{}:{}",
        config.fps, crop_w, crop_h, crop_x, crop_y
    );
    ocr_shared::tui_events::emit_log(format!(
        "   {} Kích thước ROI: {}x{} @ ({},{}) | Filter: {}",
        "[OCR]".name_utils().violed(),
        crop_w,
        crop_h,
        crop_x,
        crop_y,
        vf_filter
    ));

    // Bước 1: Mở FFmpeg pipe stream rawvideo rgb24
    let mut ffmpeg_child = Command::new("ffmpeg")
        .args([
            "-i",
            &video_path.to_string_lossy(),
            "-vf",
            &vf_filter,
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
        .context("Không thể khởi động FFmpeg streaming pipe")?;

    let mut stdout = ffmpeg_child
        .stdout
        .take()
        .context("Không thể mở stdout của FFmpeg pipe")?;

    let frame_bytes = crop_w * crop_h * 3;
    let mut current_frame = vec![0u8; frame_bytes];
    let dt = 1.0 / (config.fps as f64);
    let half_dt = dt * 0.5;
    let mut frame_idx = 0usize;

    struct SubtitleFrameRecord {
        text: String,
        confidence: f32,
        bounds: Option<(f64, f64, f64, f64)>,
    }
    let mut frame_records: Vec<SubtitleFrameRecord> = Vec::new();
    let total_frames_estimated = (total_duration_sec * config.fps as f64) as usize;

    ocr_shared::tui_events::emit_log(format!(
        "   {} Bắt đầu phân tích khung hình video (ước tính {} frames)...",
        "[OCR]".name_utils().violed(),
        total_frames_estimated
    ));

    let mut det_time = std::time::Duration::ZERO;
    let mut rec_time = std::time::Duration::ZERO;

    while stdout.read_exact(&mut current_frame).is_ok() {
        let timestamp = frame_idx as f64 * dt;
        frame_idx += 1;

        // DBNet phát hiện vùng phụ đề
        let t_det = std::time::Instant::now();
        let patch =
            self::det_filter::extract_patch_with_det(&det_engine, &current_frame, crop_w, crop_h)?;
        det_time += t_det.elapsed();

        let t_rec = std::time::Instant::now();
        let (text, confidence, bounds) = if let Some(patch) = patch {
            (
                ocr_engine
                    .recognize_patch(&patch.bytes, patch.width, patch.height)
                    .unwrap_or_default(),
                patch.confidence,
                Some(patch.bounds),
            )
        } else {
            (String::new(), 0.0, None)
        };
        rec_time += t_rec.elapsed();

        if std::env::var_os("OCR_SUB_DEBUG").is_some() && !text.is_empty() {
            eprintln!(
                "[FRAME] t={timestamp:.2}s conf={confidence:.3} bounds={bounds:?} text={text:?}"
            );
        }
        frame_records.push(SubtitleFrameRecord {
            text,
            confidence,
            bounds,
        });

        #[expect(
            clippy::manual_is_multiple_of,
            reason = "Preserve the existing zero-FPS behavior for invalid profiles"
        )]
        if frame_idx % (config.fps as usize * 3) == 0 {
            let progress_pct = (timestamp / total_duration_sec * 100.0).min(100.0);
            ocr_shared::tui_events::emit_task_progress(
                "OCR Video Subtitles",
                frame_idx,
                total_frames_estimated,
                "frames",
                format!("{:.1}s", timestamp),
                format!("{:.1}%", progress_pct),
            );
        }
    }

    let _ = ffmpeg_child.wait();
    let total_frames = frame_records.len();
    if total_frames > 0 && std::env::var_os("OCR_SUB_DEBUG").is_some() {
        eprintln!(
            "[TIMING] {} frames | det {:.1} ms/frame | rec {:.1} ms/frame",
            total_frames,
            det_time.as_secs_f64() * 1000.0 / total_frames as f64,
            rec_time.as_secs_f64() * 1000.0 / total_frames as f64,
        );
    }
    ocr_shared::tui_events::emit_log(format!(
        "   {} Đã quét xong {} frames. Tiến hành khử nhiễu & phân đoạn...",
        "[OCR]".name_utils().violed(),
        total_frames
    ));

    // Bước 2: Lọc nhiễu cục bộ (Local Temporal Denoising)
    let mut smoothed_text: Vec<String> = frame_records
        .iter()
        .map(|record| record.text.clone())
        .collect();

    // 1-frame dropout fill: A, "", A -> A, A, A (do hiệu ứng chớp tắt hoặc nén frame)
    for i in 1..total_frames.saturating_sub(1) {
        if smoothed_text[i].is_empty()
            && !smoothed_text[i - 1].is_empty()
            && !smoothed_text[i + 1].is_empty()
            && is_same_subtitle_observation(
                &smoothed_text[i - 1],
                &smoothed_text[i + 1],
                frame_records[i - 1].bounds,
                frame_records[i + 1].bounds,
            )
        {
            smoothed_text[i] = smoothed_text[i - 1].clone();
        }
    }

    // 2-frame dropout fill: A, "", "", A -> A, A, A, A
    for i in 1..total_frames.saturating_sub(2) {
        if smoothed_text[i].is_empty()
            && smoothed_text[i + 1].is_empty()
            && !smoothed_text[i - 1].is_empty()
            && !smoothed_text[i + 2].is_empty()
            && is_same_subtitle_observation(
                &smoothed_text[i - 1],
                &smoothed_text[i + 2],
                frame_records[i - 1].bounds,
                frame_records[i + 2].bounds,
            )
        {
            smoothed_text[i] = smoothed_text[i - 1].clone();
            smoothed_text[i + 1] = smoothed_text[i - 1].clone();
        }
    }

    // 1-frame isolated spike removal: "", A, "" -> "", "", "" (nhiễu rác 1 khung hình)
    for i in 1..total_frames.saturating_sub(1) {
        if !smoothed_text[i].is_empty()
            && smoothed_text[i - 1].is_empty()
            && smoothed_text[i + 1].is_empty()
        {
            smoothed_text[i] = String::new();
        }
    }

    // 1-frame typo smoothing: A, B, A -> A, A, A (triệt tiêu hiện tượng nhấp nháy chữ đơn lẻ giữa các frame)
    for i in 1..total_frames.saturating_sub(1) {
        if !smoothed_text[i - 1].is_empty()
            && !smoothed_text[i + 1].is_empty()
            && !smoothed_text[i].is_empty()
            && smoothed_text[i] != smoothed_text[i - 1]
            && smoothed_text[i - 1] == smoothed_text[i + 1]
            && is_same_subtitle_observation(
                &smoothed_text[i - 1],
                &smoothed_text[i],
                frame_records[i - 1].bounds,
                frame_records[i].bounds,
            )
        {
            // Tuyệt đối không làm suy thoái bản đầy đủ ký tự thành bản bị rụng nét!
            if smoothed_text[i].chars().count() <= smoothed_text[i - 1].chars().count() {
                smoothed_text[i] = smoothed_text[i - 1].clone();
            } else if smoothed_text[i].contains(&smoothed_text[i - 1]) {
                smoothed_text[i - 1] = smoothed_text[i].clone();
                smoothed_text[i + 1] = smoothed_text[i].clone();
            }
        }
    }

    // 2-frame typo smoothing: A, B, B, A -> A, A, A, A
    for i in 1..total_frames.saturating_sub(2) {
        if !smoothed_text[i - 1].is_empty()
            && !smoothed_text[i + 2].is_empty()
            && !smoothed_text[i].is_empty()
            && smoothed_text[i - 1] == smoothed_text[i + 2]
            && smoothed_text[i] == smoothed_text[i + 1]
            && smoothed_text[i] != smoothed_text[i - 1]
            && is_same_subtitle_observation(
                &smoothed_text[i - 1],
                &smoothed_text[i],
                frame_records[i - 1].bounds,
                frame_records[i].bounds,
            )
        {
            if smoothed_text[i].chars().count() <= smoothed_text[i - 1].chars().count() {
                smoothed_text[i] = smoothed_text[i - 1].clone();
                smoothed_text[i + 1] = smoothed_text[i - 1].clone();
            } else if smoothed_text[i].contains(&smoothed_text[i - 1]) {
                smoothed_text[i - 1] = smoothed_text[i].clone();
                smoothed_text[i + 2] = smoothed_text[i].clone();
            }
        }
    }

    // Bước 3: Phân cụm các khung hình thành các phân đoạn (Temporal Segments)
    struct RawSegment {
        start_frame: usize,
        end_frame: usize,
        texts: Vec<String>,
        confidences: Vec<f32>,
        bounds: Vec<Option<(f64, f64, f64, f64)>>,
    }

    let mut raw_segments: Vec<RawSegment> = Vec::new();
    let mut current_seg: Option<RawSegment> = None;

    for (i, text) in smoothed_text.iter().enumerate() {
        if text.is_empty() {
            if let Some(seg) = current_seg.take() {
                raw_segments.push(seg);
            }
        } else {
            match current_seg.as_mut() {
                Some(seg) => {
                    let dominant = select_best_subtitle_text(&seg.texts, &seg.confidences);
                    if is_same_subtitle_observation(
                        &dominant,
                        text,
                        frame_records[seg.end_frame].bounds,
                        frame_records[i].bounds,
                    ) {
                        seg.end_frame = i;
                        seg.texts.push(text.clone());
                        seg.confidences.push(frame_records[i].confidence);
                        seg.bounds.push(frame_records[i].bounds);
                    } else {
                        // Chuyển sang câu phụ đề tiếp theo (back-to-back subtitle)
                        let old = current_seg
                            .replace(RawSegment {
                                start_frame: i,
                                end_frame: i,
                                texts: vec![text.clone()],
                                confidences: vec![frame_records[i].confidence],
                                bounds: vec![frame_records[i].bounds],
                            })
                            .unwrap();
                        raw_segments.push(old);
                    }
                }
                None => {
                    current_seg = Some(RawSegment {
                        start_frame: i,
                        end_frame: i,
                        texts: vec![text.clone()],
                        confidences: vec![frame_records[i].confidence],
                        bounds: vec![frame_records[i].bounds],
                    });
                }
            }
        }
    }
    if let Some(seg) = current_seg.take() {
        raw_segments.push(seg);
    }

    // Bước 4: Khử nhiễu, biểu quyết nội dung & chuẩn hóa mốc thời gian WebVTT
    let mut initial_segments: Vec<TrackedSubtitleSegment> = Vec::new();
    let mut reference_glyph_heights: Vec<f64> = raw_segments
        .iter()
        .filter(|seg| {
            select_best_subtitle_text(&seg.texts, &seg.confidences)
                .chars()
                .count()
                > 1
        })
        .flat_map(|seg| seg.bounds.iter().flatten())
        .map(|bounds| bounds.3 - bounds.1)
        .filter(|height| height.is_finite() && *height > 0.0)
        .collect();
    let typical_glyph_height = median_f64(&mut reference_glyph_heights);

    for seg in raw_segments {
        let dbg = std::env::var_os("OCR_SUB_DEBUG").is_some();
        if dbg {
            eprintln!(
                "[RAW] {:.2}-{:.2} frames={} texts={:?}",
                seg.start_frame as f64 * dt,
                seg.end_frame as f64 * dt,
                seg.end_frame - seg.start_frame + 1,
                seg.texts,
            );
        }

        // Smart Majority Voting: ưu tiên câu hoàn chỉnh ký tự hơn (cứu chữ '一' ở đầu/cuối câu)
        let best_text = select_best_subtitle_text(&seg.texts, &seg.confidences);

        let char_count = best_text.chars().count();
        if char_count == 0 {
            continue;
        }

        // Tối thiểu 2 frames liên tiếp (100ms @10fps) để loại hoàn toàn nhiễu nhấp nháy 1-frame.
        // Đã thử nới cho câu 1-2 chữ Hán (giữ 1 frame): không cứu được câu nào,
        // chỉ thêm nhiễu rời rạc (海, 夏, 艾, 華, 污…) nên giữ nguyên 2 frames.
        if seg.end_frame < seg.start_frame + 1 {
            if dbg {
                eprintln!("       -> DROP 1-frame");
            }
            continue;
        }

        let t_start = seg.start_frame as f64 * dt;
        let t_end = seg.end_frame as f64 * dt;

        // Dịch thời gian bắt đầu sang trái 50ms (-0.050s) theo yêu cầu
        let start_sec = (t_start - 0.050).max(0.0);
        let end_sec = t_end + half_dt;
        let duration = end_sec - start_sec;

        // Cho phép nhận diện cả các từ chỉ có 1 ký tự (爹, 嗯, 姐, ...) với duration >= 100ms
        if char_count == 1 {
            let is_cjk = best_text
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
            if !is_cjk
                || duration < 0.10
                || !has_stable_single_character_track(
                    &seg.bounds,
                    &seg.confidences,
                    typical_glyph_height,
                )
            {
                continue;
            }
        } else if duration <= 0.40
            && !has_stable_short_subtitle_track(
                &seg.bounds,
                &seg.confidences,
                typical_glyph_height,
                char_count,
            )
        {
            if dbg {
                eprintln!("       -> DROP unstable short track ({duration:.3}s)");
            }
            continue;
        } else if duration < 0.10 {
            if std::env::var_os("OCR_SUB_DEBUG").is_some() {
                eprintln!("       -> DROP duration {:.3}s", duration);
            }
            continue;
        }

        // Đảm bảo không overlap với phân đoạn trước
        let start_sec = if let Some(last) = initial_segments.last() {
            start_sec.max(last.segment.end_sec + 0.001)
        } else {
            start_sec
        };

        if end_sec > start_sec + 0.05 {
            let confidence = seg.confidences.iter().sum::<f32>() / seg.confidences.len() as f32;
            let best_offset = seg
                .confidences
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.total_cmp(b))
                .map(|(index, _)| index)
                .unwrap_or(0);
            initial_segments.push(TrackedSubtitleSegment {
                segment: SubtitleSegment {
                    start_sec,
                    end_sec,
                    text: best_text,
                },
                confidence,
                best_frame_idx: seg.start_frame + best_offset,
            });
        }
    }

    // Gộp các phân đoạn liền kề nếu là cùng một câu (gap rất ngắn < 0.35s)
    let mut final_segments: Vec<TrackedSubtitleSegment> = Vec::new();
    for seg in initial_segments {
        if let Some(last) = final_segments.last_mut() {
            let gap = seg.segment.start_sec - last.segment.end_sec;
            if is_same_subtitle(&last.segment.text, &seg.segment.text) && gap < 0.35 {
                let old_duration = last.segment.end_sec - last.segment.start_sec;
                let new_duration = seg.segment.end_sec - seg.segment.start_sec;
                let total_duration = old_duration + new_duration;
                if total_duration > 0.0 {
                    last.confidence = (last.confidence * old_duration as f32
                        + seg.confidence * new_duration as f32)
                        / total_duration as f32;
                }
                if seg.confidence > last.confidence {
                    last.best_frame_idx = seg.best_frame_idx;
                }
                last.segment.end_sec = seg.segment.end_sec;
                if seg.segment.text.chars().count() > last.segment.text.chars().count() {
                    last.segment.text = seg.segment.text;
                }
                continue;
            }
        }
        final_segments.push(seg);
    }

    // Bỏ mảnh vụn ở điểm chuyển sub: câu 1-2 chữ, rất ngắn, nằm sát câu khác và
    // là tập con của câu đó (vd "和" cạnh "和离", "爷" cạnh "爷爷").
    let mut deduped: Vec<TrackedSubtitleSegment> = Vec::with_capacity(final_segments.len());
    for i in 0..final_segments.len() {
        let seg = &final_segments[i];
        let dur = seg.segment.end_sec - seg.segment.start_sec;
        let chars = seg.segment.text.chars().count();
        if dur <= 0.30 && chars <= 2 {
            let neighbour_contains = |other: Option<&TrackedSubtitleSegment>| -> bool {
                if let Some(o) = other {
                    let near = (o.segment.start_sec - seg.segment.end_sec).abs() <= 0.35
                        || (seg.segment.start_sec - o.segment.end_sec).abs() <= 0.35;
                    near && o.segment.text != seg.segment.text
                        && o.segment.text.contains(&seg.segment.text)
                } else {
                    false
                }
            };
            let prev = if i > 0 {
                Some(&final_segments[i - 1])
            } else {
                None
            };
            let next = final_segments.get(i + 1);
            if neighbour_contains(prev) || neighbour_contains(next) {
                continue;
            }
        }
        deduped.push(seg.clone());
    }
    let final_segments = deduped;

    if let Some((fallback_dir, threshold)) = fallback {
        export_fallback_crops(
            video_path,
            fallback_dir,
            config.fps,
            threshold,
            (crop_w, crop_h, crop_x, crop_y),
            &final_segments,
        )?;
    }

    // Đảm bảo thư mục cha tồn tại
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Ghi file định dạng chuẩn WebVTT
    let mut vtt_content = String::from("WEBVTT\n\n");
    for seg in &final_segments {
        let start_str = SubtitleSegment::format_vtt_timestamp(seg.segment.start_sec);
        let end_str = SubtitleSegment::format_vtt_timestamp(seg.segment.end_sec);
        vtt_content.push_str(&format!(
            "{} --> {}\n{}\n\n",
            start_str, end_str, seg.segment.text
        ));
    }

    fs::write(output_vtt_path, vtt_content)
        .with_context(|| format!("Không thể ghi file WebVTT: {:?}", output_vtt_path))?;

    ocr_shared::tui_events::emit_log(format!(
        "   {} {} Hoàn tất trích xuất phụ đề công nghiệp: {:?} (Tổng số câu: {})",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        output_vtt_path,
        final_segments.len()
    ));

    Ok(OcrExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments: final_segments
            .into_iter()
            .map(|tracked| tracked.segment)
            .collect(),
    })
}
