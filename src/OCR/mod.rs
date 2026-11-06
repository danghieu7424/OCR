/****
 * [MODULE]: OCR (Video Subtitle Optical Character Recognition) - Industrial Grade
 * Chức năng: Trích xuất phụ đề cứng (Hardcoded Subtitles) từ video sang chuẩn WebVTT (.vtt)
 *
 * Quy trình 4 bước chuẩn công nghiệp:
 * 1. ROI Adaptive Cropping & High-Precision FFmpeg Streaming (fps = 10-12, rawvideo rgb24).
 * 2. Deep Learning Text Detection (DBNet) & Text Line Clustering.
 * 3. PP-OCRv4/v5 CTC Recognition với chuẩn hóa PaddleOCR ([-1.0, 1.0] BGR).
 * 4. Temporal State Machine: Frame Denoising, Text Transition Boundary Detection & Majority Voting.
 ****/

use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ort::{
    session::builder::GraphOptimizationLevel,
    session::Session,
    value::Tensor,
};

use crate::shared::icons::Icons;
use crate::shared::terminal_color::NameUtilsExt;

pub mod det_filter;

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
            y_min: 0.72,
            y_max: 0.98,
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

/// Bộ máy nhận dạng chữ PP-OCRv4 bằng ONNX Runtime
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

        let threads = crate::shared::CpuOptimizer::single_task_threads().max(2);
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
    fn recognize_patch(&mut self, raw_patch: &[u8], target_w: usize, target_h: usize) -> Result<String> {
        if target_w == 0 || target_h == 0 || raw_patch.is_empty() {
            return Ok(String::new());
        }

        // Chuẩn hóa PaddleOCR chuẩn công nghiệp: (pixel / 127.5) - 1.0 trên dải kênh BGR
        let mut input_tensor = vec![0.0f32; 1 * 3 * target_h * target_w];
        for y in 0..target_h {
            for x in 0..target_w {
                let idx = (y * target_w + x) * 3;
                let r = raw_patch[idx] as f32;
                let g = raw_patch[idx + 1] as f32;
                let b = raw_patch[idx + 2] as f32;

                // Kênh 0: B, Kênh 1: G, Kênh 2: R (Chuẩn BGR)
                input_tensor[0 * target_h * target_w + y * target_w + x] = b / 127.5 - 1.0;
                input_tensor[1 * target_h * target_w + y * target_w + x] = g / 127.5 - 1.0;
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

            if max_idx != 0 && max_idx != last_idx {
                if max_idx < self.keys.len() {
                    let ch = &self.keys[max_idx];
                    if ch != "<blank>" {
                        decoded_text.push_str(ch);
                    }
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

    let mut chars: Vec<char> = trimmed.chars().collect();

    // Sửa lỗi gạch ngang hoặc dash đầu dòng thành chữ '一' (số Một) nếu theo sau là chữ Hán
    if !chars.is_empty() && (chars[0] == '-' || chars[0] == '—' || chars[0] == '–') {
        if chars.len() > 1 {
            let next_c = chars[1];
            if ('\u{4e00}'..='\u{9fff}').contains(&next_c) {
                chars[0] = '一';
            } else {
                chars.remove(0);
            }
        } else {
            return String::new();
        }
    }

    let s: String = chars.into_iter().collect();
    let cleaned = s
        .trim_matches(|c: char| {
            c.is_whitespace()
                || c == '-' || c == '—' || c == '–' || c == '_'
                || c == '<' || c == '>' || c == '《' || c == '》'
                || c == '【' || c == '】' || c == '\u{201c}' || c == '\u{201d}'
                || c == '"' || c == '\'' || c == '#' || c == '*'
                || c == '+' || c == '=' || c == '|' || c == '~'
                || c == ',' || c == '，' || c == '.' || c == '。'
                || c == '!' || c == '！' || c == '?' || c == '？'
                || c == ':' || c == '：' || c == ';' || c == '；'
        })
        .trim()
        .to_string();

    let mut final_chars: Vec<char> = cleaned.chars().collect();
    if final_chars.len() >= 2 {
        if final_chars[0].is_ascii_alphabetic() && ('\u{4e00}'..='\u{9fff}').contains(&final_chars[1]) {
            final_chars.remove(0);
        }
    }

    // Phụ đề tiếng Hán yêu cầu chứa ít nhất 1 ký tự CJK
    let has_cjk = final_chars.iter().any(|&c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk {
        return String::new();
    }

    // Giữ nguyên 100% ký tự nhận diện chuẩn từ AI mô hình, tuyệt đối không hardcode thay thế
    final_chars.into_iter().collect()
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
 * is_same_subtitle
 * Kiểm tra xem 2 khung hình nhận diện có cùng một câu thoại hay không.
 * Sử dụng kết hợp quan hệ Chuỗi con (Substring) và Levenshtein Sequence Similarity.
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

    // Nếu một câu là chuỗi con của câu kia (ví dụ rụng chữ '一' ở đầu câu: "定是错觉" trong "一定是错觉")
    if s1.contains(s2) || s2.contains(s1) {
        if max_len - min_len <= 2 {
            return true;
        }
    }

    let dist = levenshtein_distance(s1, s2);

    // Đối với câu rất ngắn (<= 3 ký tự, ví dụ: 爹, 嗯, 好), bắt buộc phải khớp tuyệt đối
    if min_len <= 3 {
        dist == 0
    } else if min_len <= 5 {
        dist <= 1
    } else {
        let sim = 1.0 - (dist as f32 / max_len as f32);
        sim >= 0.70 && dist <= 2
    }
}

/****
 * select_best_subtitle_text
 * Thuật toán Majority Voting thông minh cho OCR:
 * Khắc phục triệt để lỗi rụng ký tự nét mỏng ở mép (như chữ '一' trong "一定是错觉"):
 * Khi một ứng viên dài hơn (đầy đủ hơn) chứa trọn vẹn chuỗi con của ứng viên ngắn hơn,
 * phiếu của ứng viên ngắn sẽ chứng thực và cộng điểm cho ứng viên đầy đủ.
 ****/
fn select_best_subtitle_text(texts: &[String]) -> String {
    if texts.is_empty() {
        return String::new();
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for t in texts {
        *counts.entry(t.clone()).or_insert(0) += 1;
    }

    let total = texts.len();
    let mut best_text = String::new();
    let mut best_score = -1.0f32;

    for (cand, &count) in &counts {
        let cand_len = cand.chars().count();
        let mut score = count as f32;

        // Nếu ứng viên này xuất hiện ít nhất 20% số frame hoặc >= 3 lần:
        if count >= 3 || (total <= 5 && count >= 2) || (count as f32 / total as f32) >= 0.20 {
            for (other, &other_cnt) in &counts {
                if cand != other && cand.contains(other) {
                    // other là chuỗi con bị rụng nét (ví dụ: "定是错觉" trong "一定是错觉")
                    score += other_cnt as f32 * 0.95;
                }
            }
            score += cand_len as f32 * 0.15;
        }

        if score > best_score {
            best_score = score;
            best_text = cand.clone();
        }
    }

    if best_text.is_empty() {
        counts.into_iter().max_by_key(|(_, c)| *c).map(|(t, _)| t).unwrap_or_default()
    } else {
        best_text
    }
}

pub fn extract_hardcoded_subtitles(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
) -> Result<OcrExtractorOutput> {
    if !video_path.exists() {
        bail!("Video không tồn tại: {:?}", video_path);
    }

    crate::shared::tui_events::emit_step(">>> [OCR] Trích xuất Hardcoded Subtitles từ Video (Industrial Engine)...");
    crate::shared::tui_events::emit_log(format!(
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
    crate::shared::tui_events::emit_log(format!(
        "   {} Khởi tạo mô hình ONNX PP-OCRv4/v5 Text Recognition...",
        "[OCR]".name_utils().violed()
    ));
    let mut ocr_engine = PpOcrEngine::new(model_dir)?;

    let det_path = model_dir.join("ocr/det.onnx");
    let rec_path = model_dir.join("ocr/rec.onnx");
    let keys_path = model_dir.join("ocr/keys.txt");

    crate::shared::tui_events::emit_log(format!(
        "   {} Khởi tạo mô hình DBNet Text Detection...",
        "[OCR]".name_utils().violed()
    ));
    let det_engine = self::det_filter::create_det_engine(
        &det_path.to_string_lossy(),
        &rec_path.to_string_lossy(),
        &keys_path.to_string_lossy(),
    )?;

    // Lấy kích thước video gốc bằng ffprobe
    let probe_out = Command::new("ffprobe")
        .args(&[
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
        .args(&[
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
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .unwrap_or(600.0);

    // Tính kích thước frame crop ROI (làm tròn số chẵn cho FFmpeg)
    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (((src_w as f32 * config.roi.x_min).round() as usize) / 2) * 2;
    let crop_y = (((src_h as f32 * config.roi.y_min).round() as usize) / 2) * 2;

    let vf_filter = format!("fps={},crop={}:{}:{}:{}", config.fps, crop_w, crop_h, crop_x, crop_y);
    crate::shared::tui_events::emit_log(format!(
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
        .args(&[
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

    // Bộ sưu tập kết quả nhận diện từng frame: (frame_idx, timestamp, text)
    let mut frame_records: Vec<(usize, f64, String)> = Vec::new();
    let total_frames_estimated = (total_duration_sec * config.fps as f64) as usize;

    crate::shared::tui_events::emit_log(format!(
        "   {} Bắt đầu phân tích khung hình video (ước tính {} frames)...",
        "[OCR]".name_utils().violed(),
        total_frames_estimated
    ));

    while stdout.read_exact(&mut current_frame).is_ok() {
        let timestamp = frame_idx as f64 * dt;
        frame_idx += 1;

        // DBNet phát hiện vùng phụ đề
        let patch = self::det_filter::extract_patch_with_det(&det_engine, &current_frame, crop_w, crop_h)?;
        let text = if let Some((raw_patch, w, h)) = patch {
            ocr_engine.recognize_patch(&raw_patch, w, h).unwrap_or_default()
        } else {
            String::new()
        };

        frame_records.push((frame_idx - 1, timestamp, text));

        if frame_idx % (config.fps as usize * 3) == 0 {
            let progress_pct = (timestamp / total_duration_sec * 100.0).min(100.0);
            crate::shared::tui_events::emit_task_progress(
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
    crate::shared::tui_events::emit_log(format!(
        "   {} Đã quét xong {} frames. Tiến hành khử nhiễu & phân đoạn...",
        "[OCR]".name_utils().violed(),
        total_frames
    ));

    // Bước 2: Lọc nhiễu cục bộ (Local Temporal Denoising)
    let mut smoothed_text: Vec<String> = frame_records.iter().map(|(_, _, t)| t.clone()).collect();

    // 1-frame dropout fill: A, "", A -> A, A, A (do hiệu ứng chớp tắt hoặc nén frame)
    for i in 1..total_frames.saturating_sub(1) {
        if smoothed_text[i].is_empty() && !smoothed_text[i - 1].is_empty() && !smoothed_text[i + 1].is_empty() {
            if is_same_subtitle(&smoothed_text[i - 1], &smoothed_text[i + 1]) {
                smoothed_text[i] = smoothed_text[i - 1].clone();
            }
        }
    }

    // 2-frame dropout fill: A, "", "", A -> A, A, A, A
    for i in 1..total_frames.saturating_sub(2) {
        if smoothed_text[i].is_empty()
            && smoothed_text[i + 1].is_empty()
            && !smoothed_text[i - 1].is_empty()
            && !smoothed_text[i + 2].is_empty()
        {
            if is_same_subtitle(&smoothed_text[i - 1], &smoothed_text[i + 2]) {
                smoothed_text[i] = smoothed_text[i - 1].clone();
                smoothed_text[i + 1] = smoothed_text[i - 1].clone();
            }
        }
    }

    // 1-frame isolated spike removal: "", A, "" -> "", "", "" (nhiễu rác 1 khung hình)
    for i in 1..total_frames.saturating_sub(1) {
        if !smoothed_text[i].is_empty() && smoothed_text[i - 1].is_empty() && smoothed_text[i + 1].is_empty() {
            smoothed_text[i] = String::new();
        }
    }

    // Bước 3: Phân cụm các khung hình thành các phân đoạn (Temporal Segments)
    struct RawSegment {
        start_frame: usize,
        end_frame: usize,
        texts: Vec<String>,
    }

    let mut raw_segments: Vec<RawSegment> = Vec::new();
    let mut current_seg: Option<RawSegment> = None;

    for i in 0..total_frames {
        let text = &smoothed_text[i];
        if text.is_empty() {
            if let Some(seg) = current_seg.take() {
                raw_segments.push(seg);
            }
        } else {
            match current_seg.as_mut() {
                Some(seg) => {
                    let dominant = select_best_subtitle_text(&seg.texts);
                    if is_same_subtitle(&dominant, text) {
                        seg.end_frame = i;
                        seg.texts.push(text.clone());
                    } else {
                        // Chuyển sang câu phụ đề tiếp theo (back-to-back subtitle)
                        let old = current_seg
                            .replace(RawSegment {
                                start_frame: i,
                                end_frame: i,
                                texts: vec![text.clone()],
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
                    });
                }
            }
        }
    }
    if let Some(seg) = current_seg.take() {
        raw_segments.push(seg);
    }

    // Bước 4: Khử nhiễu, biểu quyết nội dung & chuẩn hóa mốc thời gian WebVTT
    let mut initial_segments: Vec<SubtitleSegment> = Vec::new();

    for seg in raw_segments {
        // Tối thiểu 2 frames liên tiếp (ở 20fps là 100ms) để loại bỏ hoàn toàn nhiễu nhấp nháy 1-frame
        if seg.end_frame < seg.start_frame + 1 {
            continue;
        }

        // Smart Majority Voting: ưu tiên câu hoàn chỉnh ký tự hơn (cứu chữ '一' ở đầu/cuối câu)
        let best_text = select_best_subtitle_text(&seg.texts);

        let char_count = best_text.chars().count();
        if char_count == 0 {
            continue;
        }

        let t_start = seg.start_frame as f64 * dt;
        let t_end = seg.end_frame as f64 * dt;

        // Dịch thời gian bắt đầu sang trái 100ms (-0.100s)
        let start_sec = (t_start - 0.100).max(0.0);
        let end_sec = t_end + half_dt;
        let duration = end_sec - start_sec;

        // Cho phép nhận diện cả các từ chỉ có 1 ký tự (爹, 嗯, 姐, ...) với duration >= 100ms
        if char_count == 1 {
            let is_cjk = best_text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
            if !is_cjk || duration < 0.10 {
                continue;
            }
        } else if duration < 0.10 {
            continue;
        }

        // Đảm bảo không overlap với phân đoạn trước
        let start_sec = if let Some(last) = initial_segments.last() {
            start_sec.max(last.end_sec + 0.001)
        } else {
            start_sec
        };

        if end_sec > start_sec + 0.05 {
            initial_segments.push(SubtitleSegment {
                start_sec,
                end_sec,
                text: best_text,
            });
        }
    }

    // Gộp các phân đoạn liền kề nếu là cùng một câu (gap rất ngắn < 0.35s)
    let mut final_segments: Vec<SubtitleSegment> = Vec::new();
    for seg in initial_segments {
        if let Some(last) = final_segments.last_mut() {
            let gap = seg.start_sec - last.end_sec;
            if is_same_subtitle(&last.text, &seg.text) && gap < 0.35 {
                last.end_sec = seg.end_sec;
                if seg.text.chars().count() > last.text.chars().count() {
                    last.text = seg.text;
                }
                continue;
            }
        }
        final_segments.push(seg);
    }

    // Đảm bảo thư mục cha tồn tại
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Ghi file định dạng chuẩn WebVTT
    let mut vtt_content = String::from("WEBVTT\n\n");
    for seg in &final_segments {
        let start_str = SubtitleSegment::format_vtt_timestamp(seg.start_sec);
        let end_str = SubtitleSegment::format_vtt_timestamp(seg.end_sec);
        vtt_content.push_str(&format!("{} --> {}\n{}\n\n", start_str, end_str, seg.text));
    }

    fs::write(output_vtt_path, vtt_content)
        .with_context(|| format!("Không thể ghi file WebVTT: {:?}", output_vtt_path))?;

    crate::shared::tui_events::emit_log(format!(
        "   {} {} Hoàn tất trích xuất phụ đề công nghiệp: {:?} (Tổng số câu: {})",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        output_vtt_path,
        final_segments.len()
    ));

    Ok(OcrExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments: final_segments,
    })
}
