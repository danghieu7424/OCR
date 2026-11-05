/****
 * [MODULE]: OCR (Video Subtitle Optical Character Recognition) — INDUSTRIAL GRADE v2
 *
 * Architecture: 4-step pipeline with robust timing recovery
 *
 * Step 1: Precise Frame Extraction via FFmpeg
 *   - Use pts_time from FFmpeg showinfo filter for exact timestamps
 *   - Sample at configurable FPS with proper ROI cropping
 *
 * Step 2: Text Detection (DBNet) + Recognition (PP-OCRv4/v5 CRNN)
 *   - det_filter.rs handles bounding box detection and patch extraction
 *   - Recognition with proper NCHW tensor + CTC greedy decode
 *
 * Step 3: Scene-Change Detection via Text Transition
 *   - Track text changes, not pixel differences
 *   - Use voting within stable text windows
 *
 * Step 4: Temporal Consolidation & VTT Output
 *   - Merge near-duplicate cues
 *   - Fix overlap, filter noise
 *   - Output standard WebVTT
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
    pub y_min: f32,
    pub y_max: f32,
    pub x_min: f32,
    pub x_max: f32,
}

impl Default for SubtitleRoi {
    fn default() -> Self {
        Self {
            y_min: 0.75,
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
        Some(Self { y_min, y_max, x_min, x_max })
    }
}

/// Cấu hình trích xuất phụ đề video
#[derive(Debug, Clone)]
pub struct OcrExtractorConfig {
    /// Tần số lấy mẫu khung hình (mặc định: 8 fps)
    pub fps: u32,
    /// Tọa độ vùng cắt chữ
    pub roi: SubtitleRoi,
    /// Ngôn ngữ nhận diện ưu tiên ("zh", "en", "auto")
    pub lang: String,
    /// Ngưỡng phát hiện biến thiên khung hình (hiện không dùng MSE nữa, giữ cho compat)
    pub diff_threshold: f32,
    /// Chế độ chính xác
    pub accuracy_mode: bool,
}

impl Default for OcrExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 8,
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
    pub vtt_path: PathBuf,
    pub segments: Vec<SubtitleSegment>,
}

/// Cấu trúc lưu kết quả nhận diện kèm frame ảnh mẫu
#[derive(Default, Clone)]
pub struct OcrFrameRecognition {
    pub text: String,
    pub confidence: f32,
    pub raw_patch: Vec<u8>,
    pub clahe_patch: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

/// Lưu mảng RGB24 thành file BMP (debug)
fn save_rgb_as_bmp(path: &Path, rgb: &[u8], width: usize, height: usize) -> std::io::Result<()> {
    use std::io::Write;
    if width == 0 || height == 0 || rgb.is_empty() {
        return Ok(());
    }
    let row_stride = (width * 3 + 3) & !3;
    let image_size = row_stride * height;
    let file_size = 54 + image_size;

    let mut header = [0u8; 54];
    header[0] = b'B';
    header[1] = b'M';
    header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    header[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&24u16.to_le_bytes());
    header[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());

    let mut file = std::fs::File::create(path)?;
    file.write_all(&header)?;

    let mut row_buf = vec![0u8; row_stride];
    for y in 0..height {
        let src_row = &rgb[y * width * 3..(y + 1) * width * 3];
        for x in 0..width {
            let r = src_row[x * 3];
            let g = src_row[x * 3 + 1];
            let b = src_row[x * 3 + 2];
            row_buf[x * 3] = b;
            row_buf[x * 3 + 1] = g;
            row_buf[x * 3 + 2] = r;
        }
        file.write_all(&row_buf)?;
    }
    Ok(())
}

/// PP-OCR Recognition Engine (ONNX)
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
            bail!("Thiếu tài nguyên mô hình OCR tại {:?}", ocr_dir);
        }

        let keys_content = fs::read_to_string(&keys_path)
            .with_context(|| format!("Không thể nạp từ điển ký tự OCR: {:?}", keys_path))?;
        let mut keys: Vec<String> = keys_content.lines().map(|l| l.to_string()).collect();
        keys.insert(0, "<blank>".to_string());
        keys.push(" ".to_string());

        let threads = crate::shared::CpuOptimizer::single_task_threads();
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

    /// Nhận diện văn bản từ ảnh RGB24 [H, W, 3]
    fn recognize_text(
        &mut self,
        rgb_bytes: &[u8],
        width: usize,
        height: usize,
        _tight: bool,
        det_engine: &pure_onnx_ocr::OcrEngine,
    ) -> Result<OcrFrameRecognition> {
        if width == 0 || height == 0 || rgb_bytes.is_empty() {
            return Ok(OcrFrameRecognition::default());
        }

        // 1. Gọi DBNet để tìm bounding box và cắt ảnh patch
        let (raw_patch, target_w, target_h) =
            match self::det_filter::extract_patch_with_det(det_engine, rgb_bytes, width, height)? {
                Some(patch) => patch,
                None => return Ok(OcrFrameRecognition::default()),
            };

        let clahe_patch = raw_patch.clone();
        let raw_patch_debug = raw_patch.clone();

        // 2. Chuẩn bị Tensor NCHW [1, 3, target_h, target_w]
        let mut input_tensor = vec![0.0f32; 1 * 3 * target_h * target_w];
        for y in 0..target_h {
            for x in 0..target_w {
                let idx = (y * target_w + x) * 3;
                let r_norm = (clahe_patch[idx] as f32 / 255.0 - 0.485) / 0.229;
                let g_norm = (clahe_patch[idx + 1] as f32 / 255.0 - 0.456) / 0.224;
                let b_norm = (clahe_patch[idx + 2] as f32 / 255.0 - 0.406) / 0.225;

                // PP-OCR expects BGR channel order
                input_tensor[0 * target_h * target_w + y * target_w + x] = b_norm;
                input_tensor[1 * target_h * target_w + y * target_w + x] = g_norm;
                input_tensor[2 * target_h * target_w + y * target_w + x] = r_norm;
            }
        }

        let tensor_val = Tensor::from_array(([1usize, 3usize, target_h, target_w], input_tensor))?;
        let outputs = self.rec_session.run(ort::inputs![tensor_val])
            .map_err(|e| anyhow::anyhow!("Lỗi suy luận ONNX OCR: {:?}", e))?;
        let (shape, logits) = outputs[0].try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("Lỗi trích xuất tensor output OCR: {:?}", e))?;

        let time_steps = shape[1] as usize;
        let num_classes = shape[2] as usize;

        // CTC Greedy Decoder with confidence
        let mut decoded_text = String::new();
        let mut last_idx = 0usize;
        let mut total_conf = 0.0f32;
        let mut conf_count = 0usize;

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
                        // Softmax approximation for confidence
                        total_conf += max_val;
                        conf_count += 1;
                    }
                }
            }
            last_idx = max_idx;
        }

        let avg_conf = if conf_count > 0 { total_conf / conf_count as f32 } else { 0.0 };
        let cleaned = sanitize_ocr_text(&decoded_text);

        Ok(OcrFrameRecognition {
            text: cleaned,
            confidence: avg_conf,
            raw_patch: raw_patch_debug,
            clahe_patch,
            width: target_w,
            height: target_h,
        })
    }
}

/****
 * sanitize_ocr_text
 * Loại bỏ ký tự viền, gạch ngang, dấu ngoặc và bảo vệ chữ số 一
 ****/
fn sanitize_ocr_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut chars: Vec<char> = trimmed.chars().collect();

    // 1. Kiểm tra đầu chuỗi: '-' hoặc '—' có thể là chữ 一
    if !chars.is_empty() && (chars[0] == '-' || chars[0] == '—' || chars[0] == '–') {
        if chars.len() > 1 {
            let next_c = chars[1];
            let is_yi_compound = matches!(
                next_c,
                '个' | '起' | '定' | '直' | '样' | '体' | '切' | '见' | '步' | '些' | '生'
                    | '世' | '心' | '意' | '点' | '回' | '次' | '阵' | '声' | '眼' | '口'
                    | '同' | '如' | '致' | '贯' | '番' | '代' | '命' | '面' | '落' | '线'
                    | '半' | '刻' | '层'
            );
            if is_yi_compound {
                chars[0] = '一';
            } else {
                chars.remove(0);
            }
        } else {
            return String::new();
        }
    }

    // 2. Loại bỏ các ký tự rác đầu/cuối
    let s: String = chars.into_iter().collect();
    let mut cleaned = s
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

    // 3. Loại bỏ ký tự Latin rác đứng lẻ ở đầu câu tiếng Hán
    if cleaned.len() >= 4 {
        let first_char = cleaned.chars().next().unwrap_or_default();
        if first_char.is_ascii_alphabetic() {
            let second_char = cleaned.chars().nth(1).unwrap_or_default();
            if ('\u{4e00}'..='\u{9fff}').contains(&second_char) {
                cleaned = cleaned.chars().skip(1).collect();
            }
        }
    }

    // 4. Lọc bỏ chuỗi Latin thuần vô nghĩa (phụ đề Hán phải chứa CJK)
    let has_cjk = cleaned.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk {
        let is_pure_latin_noise = cleaned.chars().all(|c| c.is_ascii_alphabetic() || c.is_whitespace() || c.is_ascii_digit());
        if is_pure_latin_noise {
            return String::new();
        }
    }

    cleaned
}

/****
 * text_similarity (Sørensen–Dice on character multisets)
 ****/
fn text_similarity(s1: &str, s2: &str) -> f32 {
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();

    if chars1.is_empty() || chars2.is_empty() {
        return 0.0;
    }
    if s1 == s2 {
        return 1.0;
    }

    let min_len = chars1.len().min(chars2.len());
    let max_len = chars1.len().max(chars2.len());

    // Nếu lệch quá 50% thì khác câu
    if (max_len - min_len) as f32 / max_len as f32 > 0.50 {
        return 0.0;
    }

    let mut matches = 0usize;
    let mut c2_used = vec![false; chars2.len()];
    for c1 in &chars1 {
        for (j, c2) in chars2.iter().enumerate() {
            if !c2_used[j] && c1 == c2 {
                c2_used[j] = true;
                matches += 1;
                break;
            }
        }
    }

    (2.0 * matches as f32) / (chars1.len() + chars2.len()) as f32
}

/// Một frame đã được OCR kèm timestamp chính xác
struct TimedOcrFrame {
    /// Timestamp chính xác từ FFmpeg pts (giây)
    timestamp: f64,
    /// Văn bản nhận diện được (đã sanitize)
    text: String,
    /// Confidence score
    confidence: f32,
    /// Debug data
    raw_patch: Vec<u8>,
    clahe_patch: Vec<u8>,
    patch_w: usize,
    patch_h: usize,
}

/// Phiên chữ (cue session) — một khoảng thời gian liên tục có cùng nội dung chữ
struct CueSession {
    /// Timestamp chính xác khi chữ BẮT ĐẦU xuất hiện (từ FFmpeg pts)
    first_seen: f64,
    /// Timestamp chính xác khi chữ CUỐI CÙNG được nhìn thấy
    last_seen: f64,
    /// Bảng vote: text → count
    candidates: HashMap<String, usize>,
    /// Debug sample
    sample_raw: Vec<u8>,
    sample_clahe: Vec<u8>,
    sample_w: usize,
    sample_h: usize,
}

impl CueSession {
    fn new(frame: &TimedOcrFrame) -> Self {
        let mut candidates = HashMap::new();
        candidates.insert(frame.text.clone(), 1);
        Self {
            first_seen: frame.timestamp,
            last_seen: frame.timestamp,
            candidates,
            sample_raw: frame.raw_patch.clone(),
            sample_clahe: frame.clahe_patch.clone(),
            sample_w: frame.patch_w,
            sample_h: frame.patch_h,
        }
    }

    fn vote(&mut self, frame: &TimedOcrFrame) {
        self.last_seen = frame.timestamp;
        let count = self.candidates.entry(frame.text.clone()).or_insert(0);
        *count += 1;
        // Update sample from frame with higher vote count
        if *count >= 2 && !frame.raw_patch.is_empty() {
            self.sample_raw = frame.raw_patch.clone();
            self.sample_clahe = frame.clahe_patch.clone();
            self.sample_w = frame.patch_w;
            self.sample_h = frame.patch_h;
        }
    }

    fn best_text(&self) -> Option<String> {
        self.candidates
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(text, _)| text.clone())
            .filter(|t| !t.is_empty())
    }

    fn dominant_text(&self) -> &str {
        self.candidates
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(text, _)| text.as_str())
            .unwrap_or("")
    }

    /// Kiểm tra text mới có phải là cùng câu với session hiện tại không
    fn is_same_text(&self, new_text: &str) -> bool {
        let dominant = self.dominant_text();
        if dominant == new_text {
            return true;
        }
        let sim = text_similarity(dominant, new_text);
        let len1 = dominant.chars().count();
        let len2 = new_text.chars().count();
        let min_len = len1.min(len2);

        // Dynamic threshold: câu ngắn dễ dao động hơn
        let threshold = if min_len < 4 {
            0.50
        } else if min_len < 8 {
            0.60
        } else {
            0.70
        };

        sim >= threshold
    }

    /// Tạo segment từ session, dùng khoảng thời gian first_seen → last_seen + half_dt
    fn to_segment(&self, half_dt: f64) -> Option<SubtitleSegment> {
        let text = self.best_text()?;
        let start = self.first_seen;
        let end = self.last_seen + half_dt;
        if end <= start + 0.08 {
            return None;
        }
        Some(SubtitleSegment {
            start_sec: start,
            end_sec: end,
            text,
        })
    }
}

/****
 * MAIN ENTRY POINT: extract_hardcoded_subtitles
 *
 * Industrial-grade approach:
 * 1. Use FFmpeg with showinfo filter to get EXACT pts timestamps for each frame
 * 2. OCR every sampled frame 
 * 3. Group consecutive frames with same text into cue sessions
 * 4. Post-process: merge duplicates, filter noise, fix overlaps
 ****/
pub fn extract_hardcoded_subtitles(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
) -> Result<OcrExtractorOutput> {
    if !video_path.exists() {
        bail!("Video không tồn tại: {:?}", video_path);
    }

    crate::shared::tui_events::emit_step(">>> [OCR] Trích xuất Hardcoded Subtitles từ Video (Industrial Grade v2)...");
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

    // === Bước 0: Nạp mô hình OCR ===
    crate::shared::tui_events::emit_log(format!(
        "   {} Khởi tạo mô hình ONNX PP-OCR Text Recognition...",
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

    // === Lấy kích thước video gốc ===
    let probe_out = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
            &video_path.to_string_lossy(),
        ])
        .output()
        .context("Không thể thực thi ffprobe")?;

    let dim_str = String::from_utf8_lossy(&probe_out.stdout);
    let dims: Vec<usize> = dim_str
        .trim()
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect();

    let (src_w, src_h) = if dims.len() == 2 {
        (dims[0], dims[1])
    } else {
        (1280usize, 720usize)
    };

    // === Lấy tổng thời lượng video ===
    let duration_out = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .output();
    let total_duration_sec: f64 = duration_out
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .unwrap_or(600.0);

    // === Tính kích thước ROI crop ===
    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (src_w as f32 * config.roi.x_min).round() as usize;
    let crop_y = (src_h as f32 * config.roi.y_min).round() as usize;

    let vf_filter = format!(
        "fps={},crop={}:{}:{}:{}",
        config.fps, crop_w, crop_h, crop_x, crop_y
    );
    crate::shared::tui_events::emit_log(format!(
        "   {} ROI crop: {}x{} @ ({},{}) | Filter: {}",
        "[OCR]".name_utils().violed(),
        crop_w, crop_h, crop_x, crop_y, vf_filter
    ));

    // === Bước 1: FFmpeg pipe stream rawvideo rgb24 ===
    // fps=N filter outputs frames at exactly n/N seconds, so timestamp = frame_idx / fps
    // is mathematically precise. No showinfo needed.
    let mut ffmpeg_child = Command::new("ffmpeg")
        .args(&[
            "-i", &video_path.to_string_lossy(),
            "-vf", &vf_filter,
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-v", "error",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Không thể khởi động FFmpeg streaming pipe")?;

    let mut stdout = ffmpeg_child.stdout.take().context("Không thể mở stdout FFmpeg")?;

    let frame_bytes = crop_w * crop_h * 3;
    let mut current_frame = vec![0u8; frame_bytes];
    let dt = 1.0 / (config.fps as f64);
    let half_dt = dt * 0.5;
    let mut frame_idx = 0usize;

    let frames_debug_dir = output_vtt_path
        .parent()
        .map(|p| p.join("ocr_frames"))
        .unwrap_or_else(|| PathBuf::from("ocr_frames"));
    let _ = fs::create_dir_all(&frames_debug_dir);

    // === Bước 2+3: OCR mỗi frame và track text transitions ===
    let mut raw_segments: Vec<SubtitleSegment> = Vec::new();
    let mut current_session: Option<CueSession> = None;
    let mut consecutive_empty = 0usize;

    // Số frame trống tối đa trước khi đóng session
    // Với fps=8: 3 frames = 375ms — đủ cho hiệu ứng fade ngắn
    // Với fps=12: 4 frames = 333ms
    let max_empty_gap = (config.fps as usize / 3).max(2);

    println!("   [OCR] Bắt đầu quét video... (max_empty_gap={})", max_empty_gap);

    while stdout.read_exact(&mut current_frame).is_ok() {
        // Timestamp chính xác: fps=N filter tạo frame tại n/N giây
        let timestamp = frame_idx as f64 * dt;
        frame_idx += 1;

        // OCR frame
        let rec = ocr_engine
            .recognize_text(&current_frame, crop_w, crop_h, true, &det_engine)
            .unwrap_or_default();

        let timed_frame = TimedOcrFrame {
            timestamp,
            text: rec.text.clone(),
            confidence: rec.confidence,
            raw_patch: rec.raw_patch,
            clahe_patch: rec.clahe_patch,
            patch_w: rec.width,
            patch_h: rec.height,
        };

        if timed_frame.text.is_empty() {
            // Frame trống (không có chữ)
            consecutive_empty += 1;

            if consecutive_empty >= max_empty_gap {
                // Đóng session hiện tại
                if let Some(session) = current_session.take() {
                    if let Some(seg) = session.to_segment(half_dt) {
                        let cue_idx = raw_segments.len() + 1;
                        let _ = save_rgb_as_bmp(
                            &frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx)),
                            &session.sample_clahe,
                            session.sample_w,
                            session.sample_h,
                        );
                        raw_segments.push(seg);
                    }
                }
            }
        } else {
            // Frame có chữ
            consecutive_empty = 0;

            match current_session.as_mut() {
                Some(session) => {
                    if session.is_same_text(&timed_frame.text) {
                        // Cùng câu → vote
                        session.vote(&timed_frame);
                    } else {
                        // Text khác → đóng session cũ, mở session mới
                        let old_session = current_session.take().unwrap();
                        if let Some(seg) = old_session.to_segment(half_dt) {
                            let cue_idx = raw_segments.len() + 1;
                            let _ = save_rgb_as_bmp(
                                &frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx)),
                                &old_session.sample_clahe,
                                old_session.sample_w,
                                old_session.sample_h,
                            );
                            raw_segments.push(seg);
                        }
                        current_session = Some(CueSession::new(&timed_frame));
                    }
                }
                None => {
                    // Bắt đầu session mới
                    current_session = Some(CueSession::new(&timed_frame));
                }
            }
        }

        // Progress logging (mỗi 2 giây)
        if frame_idx % (config.fps as usize * 2) == 0 {
            let progress_pct = (timestamp / total_duration_sec * 100.0).min(100.0);
            crate::shared::tui_events::emit_task_progress(
                "OCR Scanning",
                frame_idx,
                (total_duration_sec * config.fps as f64) as usize,
                "frames",
                format!("{:.1}s", timestamp),
                format!("{:.1}% | Cues: {}", progress_pct, raw_segments.len()),
            );
        }
    }

    // Đóng session cuối cùng
    if let Some(session) = current_session.take() {
        if let Some(seg) = session.to_segment(half_dt) {
            let cue_idx = raw_segments.len() + 1;
            let _ = save_rgb_as_bmp(
                &frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx)),
                &session.sample_clahe,
                session.sample_w,
                session.sample_h,
            );
            raw_segments.push(seg);
        }
    }

    let _ = ffmpeg_child.wait();

    println!(
        "   [OCR] Quét xong. Raw segments: {} | Frames processed: {}",
        raw_segments.len(),
        frame_idx
    );

    // === Bước 4: Post-processing ===
    let cleaned_segments = post_process_segments(raw_segments, config, dt);

    // === Ghi file VTT ===
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut vtt_content = String::from("WEBVTT\n\n");
    for seg in &cleaned_segments {
        let start_str = SubtitleSegment::format_vtt_timestamp(seg.start_sec);
        let end_str = SubtitleSegment::format_vtt_timestamp(seg.end_sec);
        vtt_content.push_str(&format!("{} --> {}\n{}\n\n", start_str, end_str, seg.text));
    }

    fs::write(output_vtt_path, vtt_content)
        .with_context(|| format!("Không thể ghi file WebVTT: {:?}", output_vtt_path))?;

    crate::shared::tui_events::emit_log(format!(
        "   {} {} Đã trích xuất hoàn tất phụ đề OCR: {:?} (Tổng: {} cues)",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        output_vtt_path,
        cleaned_segments.len()
    ));
    crate::shared::tui_events::emit_log(format!(
        "   {} {} Debug frames: {:?}",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        frames_debug_dir
    ));

    Ok(OcrExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments: cleaned_segments,
    })
}

/****
 * post_process_segments
 *
 * 1. Filter noise (too short, no CJK, watermark)
 * 2. Merge near-duplicate consecutive cues
 * 3. Fix overlaps
 * 4. Final sanity checks
 ****/
fn post_process_segments(
    raw_segments: Vec<SubtitleSegment>,
    config: &OcrExtractorConfig,
    dt: f64,
) -> Vec<SubtitleSegment> {
    if raw_segments.is_empty() {
        return Vec::new();
    }

    println!("   [OCR] Post-processing {} raw segments...", raw_segments.len());

    // --- Pass 1: Basic filtering ---
    let mut filtered: Vec<SubtitleSegment> = Vec::new();
    for seg in raw_segments {
        let char_count = seg.text.chars().count();
        let duration = seg.end_sec - seg.start_sec;

        // Bỏ cue quá ngắn (< 2 ký tự)
        if char_count < 2 {
            continue;
        }

        // Lọc CJK requirement cho tiếng Trung
        if config.lang.starts_with("zh") {
            let has_cjk = seg.text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
            if !has_cjk {
                continue;
            }
            let latin_count = seg.text.chars().filter(|c| c.is_ascii_alphabetic()).count();
            let cjk_count = seg.text.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).count();
            if latin_count > cjk_count && cjk_count < 3 {
                continue;
            }
        }

        // Lọc watermark: quá dài (>15s) rất có thể là watermark
        if duration > 15.0 {
            continue;
        }

        // Minimum duration: rất ngắn = noise
        // Nhưng threshold cực thấp để không drop cue hợp lệ
        if duration < 0.15 {
            continue;
        }

        filtered.push(seg);
    }

    println!("   [OCR] After basic filter: {} segments", filtered.len());

    // --- Pass 2: Merge near-duplicate consecutive cues ---
    let mut merged: Vec<SubtitleSegment> = Vec::new();
    for seg in filtered {
        if let Some(last) = merged.last_mut() {
            let gap = seg.start_sec - last.end_sec;
            let sim = text_similarity(&last.text, &seg.text);
            let last_len = last.text.chars().count();
            let seg_len = seg.text.chars().count();
            let min_len = last_len.min(seg_len);

            // Merge threshold dựa trên độ dài
            let merge_threshold = if min_len <= 4 { 0.60 } else if min_len <= 8 { 0.70 } else { 0.80 };

            if sim >= merge_threshold && gap < 1.5 {
                // Cùng câu → merge, giữ text dài hơn (thường chính xác hơn)
                last.end_sec = last.end_sec.max(seg.end_sec);

                // Prefer bản có CJK count cao hơn, hoặc text dài hơn
                let last_cjk = last.text.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).count();
                let seg_cjk = seg.text.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).count();
                if seg_cjk > last_cjk || (seg_cjk == last_cjk && seg_len > last_len) {
                    last.text = seg.text;
                }
                continue;
            }

            // Exact match merge với gap lớn hơn
            if last.text == seg.text && gap < 3.0 {
                last.end_sec = last.end_sec.max(seg.end_sec);
                continue;
            }
        }
        merged.push(seg);
    }

    println!("   [OCR] After merge: {} segments", merged.len());

    // --- Pass 3: Fix overlaps ---
    for i in 1..merged.len() {
        if merged[i].start_sec < merged[i - 1].end_sec {
            // Chia thời gian tại điểm giữa
            let mid = (merged[i - 1].end_sec + merged[i].start_sec) / 2.0;
            merged[i - 1].end_sec = mid - 0.001;
            merged[i].start_sec = mid;
        }
    }

    // Remove degenerate segments after overlap fix
    merged.retain(|s| s.end_sec > s.start_sec + 0.05);

    println!("   [OCR] Final output: {} segments", merged.len());

    merged
}
