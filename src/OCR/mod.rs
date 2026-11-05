/****
 * [MODULE]: OCR (Video Subtitle Optical Character Recognition)
 * Chức năng: Trích xuất phụ đề cứng (Hardcoded Subtitles) từ video sang chuẩn WebVTT (.vtt)
 *
 * Quy trình 4 bước:
 * 1. ROI Cropping & Adaptive Sampling (Tần số mẫu 8 fps trên dải dọc 80%-100%, ngang 5%-95%).
 * 2. Subtitle Scene Change & Keyframe Detection (So sánh biến thiên đồ họa pixel MSE/Luminance).
 * 3. Text Detection & Recognition (OCR Engine: RapidOCR / PaddleOCR ONNX).
 * 4. Temporal Voting & VTT Formatting (Khử flicker, đồng bộ timestamp mili-giây, xuất .vtt).
 *
 * Ranh giới bảo vệ:
 * - Module độc lập (Isolation): Không gây ảnh hưởng hay làm thay đổi logic các bước ASR, Translation, TTS.
 * - Hiệu năng cao: Stream frame trực tiếp qua FFmpeg pipe, không lưu frame rác ra đĩa.
 ****/

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ort::{
    session::builder::GraphOptimizationLevel,
    session::Session,
    value::Tensor,
};

use crate::shared::terminal_color::NameUtilsExt;
use crate::shared::icons::Icons;

pub mod opencv_filter;
/// Cấu hình tọa độ vùng ROI (Region of Interest)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubtitleRoi {
    /// Tỷ lệ biên trên (mặc định: 0.85 = 85%)
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
    /****
     * parse
     * Phân tích chuỗi "ymin,ymax,xmin,xmax" thành cấu trúc SubtitleRoi.
     * Fallback sang Default nếu định dạng không khớp.
     ****/
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
pub struct OcrExtractorConfig {
    /// Tần số lấy mẫu khung hình (mặc định: 8 fps)
    pub fps: u32,
    /// Tọa độ vùng cắt chữ
    pub roi: SubtitleRoi,
    /// Ngôn ngữ nhận diện ưu tiên ("zh", "en", "auto")
    pub lang: String,
    /// Ngưỡng phát hiện biến thiên khung hình để xác định ranh giới phụ đề (0.01 - 0.20)
    pub diff_threshold: f32,
    /// Chế độ chính xác: padding tight + filter chặt hơn + fps cao
    pub accuracy_mode: bool,
}

impl Default for OcrExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 12,
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

/// Cấu trúc lưu kết quả nhận diện kèm frame ảnh mẫu để người dùng kiểm tra trực quan
#[derive(Default, Clone)]
pub struct OcrFrameRecognition {
    pub text: String,
    pub raw_patch: Vec<u8>,
    pub clahe_patch: Vec<u8>,
    pub width: usize,
    pub height: usize,
}



/****
 * save_rgb_as_bmp
 * Lưu mảng RGB24 thành file BMP chuẩn Windows (24-bit uncompressed).
 * Zero dependency - mở xem trực tiếp trên Windows và VS Code.
 ****/
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
    header[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes()); // Top-down
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
            row_buf[x * 3] = b;     // B
            row_buf[x * 3 + 1] = g; // G
            row_buf[x * 3 + 2] = r; // R
        }
        file.write_all(&row_buf)?;
    }
    Ok(())
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
            bail!("Thiếu tài nguyên mô hình OCR tại {:?}. Vui lòng kiểm tra rec.onnx và keys.txt", ocr_dir);
        }

        let keys_content = fs::read_to_string(&keys_path)
            .with_context(|| format!("Không thể nạp từ điển ký tự OCR: {:?}", keys_path))?;
        let mut keys: Vec<String> = keys_content.lines().map(|l| l.to_string()).collect();
        // PP-OCR mapping: 0 là blank (CTC blank token), sau đó là keys[1..], cuối cùng là space
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

        Ok(Self {
            rec_session,
            keys,
        })
    }

        /// Nhận diện văn bản từ ảnh RGB24 dạng phẳng [H, W, 3] 
    fn recognize_text(&mut self, rgb_bytes: &[u8], width: usize, height: usize, tight: bool) -> Result<OcrFrameRecognition> {
        if width == 0 || height == 0 || rgb_bytes.is_empty() {
            return Ok(OcrFrameRecognition::default());
        }

        // 1. Gọi module OpenCV để lọc nhiễu, tìm bounding box, cắt và resize patch
        let (raw_patch, clahe_patch, target_w, target_h) = 
            match self::opencv_filter::extract_and_filter_patch(rgb_bytes, width, height, tight)? {
                Some(patch) => patch,
                None => return Ok(OcrFrameRecognition::default()), // Không tìm thấy chữ
            };

        // 2. Chuẩn bị Tensor NCHW [1, 3, 48, target_w] từ patch đã qua xử lý OpenCV
        let mut input_tensor = vec![0.0f32; 1 * 3 * target_h * target_w];
        for y in 0..target_h {
            for x in 0..target_w {
                let idx = (y * target_w + x) * 3;
                // Chuẩn hóa ImageNet cho PP-OCRv5
                let r_norm = (clahe_patch[idx] as f32 / 255.0 - 0.485) / 0.229;
                let g_norm = (clahe_patch[idx + 1] as f32 / 255.0 - 0.456) / 0.224;
                let b_norm = (clahe_patch[idx + 2] as f32 / 255.0 - 0.406) / 0.225;

                input_tensor[0 * target_h * target_w + y * target_w + x] = r_norm;
                input_tensor[1 * target_h * target_w + y * target_w + x] = g_norm;
                input_tensor[2 * target_h * target_w + y * target_w + x] = b_norm;
            }
        }

        let tensor_val = Tensor::from_array(([1usize, 3usize, target_h, target_w], input_tensor))?;
        let outputs = self.rec_session.run(ort::inputs![tensor_val])
            .map_err(|e| anyhow::anyhow!("Lỗi suy luận ONNX OCR: {:?}", e))?;
        let (shape, logits) = outputs[0].try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("Lỗi trích xuất tensor output OCR: {:?}", e))?;

        // Shape output là [1, time_steps, num_classes]
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

            // max_idx = 0 là CTC blank token
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

        // Làm sạch ký tự rác (stroke, dash, dấu ngoặc nhọn, icon) và bảo vệ chữ số 一
        let cleaned = sanitize_ocr_text(&decoded_text);

        Ok(OcrFrameRecognition {
            text: cleaned,
            raw_patch,
            clahe_patch,
            width: target_w,
            height: target_h,
        })
    }
}

/****
 * sanitize_ocr_text
 * Loại bỏ toàn diện ký tự viền, gạch ngang, dấu ngoặc và bảo vệ chữ số 一
 * - Nếu ký tự đầu là '-' hoặc '—' nhưng theo sau là lượng từ/từ tố kết hợp với '一'
 *   (ví dụ: 个, 起, 定, 直, 样, 体, 切, 见, 步, 些, 生, 世, 心, 意, 点, 回, 次, 阵, 声...)
 *   -> Chuyển thành '一' (sửa lỗi OCR nhận diện nhầm nét chữ Nhất).
 * - Nếu theo sau là đại từ/danh từ/ký hiệu khác -> Cắt bỏ vì đó là gạch thoại hoặc vạch biên rác.
 ****/
fn sanitize_ocr_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut chars: Vec<char> = trimmed.chars().collect();

    // 1. Kiểm tra đầu chuỗi: Nếu bắt đầu bằng '-' hoặc '—'
    if !chars.is_empty() && (chars[0] == '-' || chars[0] == '—' || chars[0] == '–') {
        if chars.len() > 1 {
            let next_c = chars[1];
            // Danh sách các từ tố/lượng từ thường đi liền sau chữ số 一
            let is_yi_compound = matches!(
                next_c,
                '个' | '起' | '定' | '直' | '样' | '体' | '切' | '见' | '步' | '些' | '生'
                    | '世' | '心' | '意' | '点' | '回' | '次' | '阵' | '声' | '眼' | '口'
                    | '同' | '如' | '致' | '贯' | '番' | '代' | '命' | '面' | '落' | '线'
            );
            if is_yi_compound {
                // Sửa lỗi OCR: biến nét gạch thành chữ số Một (一)
                chars[0] = '一';
            } else {
                // Là gạch thoại hoặc vạch viền rác -> bỏ đi
                chars.remove(0);
            }
        } else {
            return String::new();
        }
    }

    // 2. Loại bỏ các ký tự rác đầu/cuối: ngoặc, gạch, ký hiệu
    let s: String = chars.into_iter().collect();
    let mut cleaned = s
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
                || c == '“'
                || c == '”'
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

    // 3. Loại bỏ ký tự Latin rác đứng lẻ ở đầu câu tiếng Hán (ví dụ: "e夜雅" -> "夜雅")
    if cleaned.len() >= 4 {
        let first_char = cleaned.chars().next().unwrap_or_default();
        if first_char.is_ascii_alphabetic() {
            let second_char = cleaned.chars().nth(1).unwrap_or_default();
            if ('\u{4e00}'..='\u{9fff}').contains(&second_char) {
                cleaned = cleaned.chars().skip(1).collect();
            }
        }
    }

    // 4. Lọc bỏ chuỗi ký tự Latinh vô nghĩa sinh ra do texture nền (ví dụ: "EN", "MN", "NG", "SM", "aN", "AKIVEANON")
    // Phụ đề phim tiếng Trung bắt buộc phải chứa ít nhất 1 chữ Hán (0x4E00..=0x9FFF).
    // Nếu chuỗi hoàn toàn không có chữ Hán nào và chỉ gồm các chữ cái Latinh ngẫu nhiên thì hủy bỏ.
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
 * text_similarity
 * Tính độ tương đồng giữa 2 chuỗi theo Multiset Character Matching (Sørensen–Dice)
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

    // Nếu độ dài lệch quá 45%, không thể là cùng 1 câu phụ đề
    if (max_len - min_len) as f32 / max_len as f32 > 0.45 {
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

/****
 * dynamic_similarity_threshold
 * Ngưỡng tương đồng động theo độ dài văn bản:
 * - Câu ngắn (<5 ký tự): 0.50 (dễ gộp)
 * - Câu trung bình (5-15): 0.70
 * - Câu dài (>15): 0.80 + tỉ lệ độ dài
 ****/
fn dynamic_similarity_threshold(text1: &str, text2: &str) -> f32 {
    let len1 = text1.chars().count();
    let len2 = text2.chars().count();
    let min_len = len1.min(len2);
    let max_len = len1.max(len2);

    if min_len < 5 {
        0.50
    } else if min_len < 15 {
        0.70
    } else {
        0.80 + (min_len as f32 / max_len as f32) * 0.10
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

    crate::shared::tui_events::emit_step(">>> [OCR] Trích xuất Hardcoded Subtitles từ Video (4-Step Engine)...");
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

    // Bước 0: Nạp mô hình OCR PP-OCRv4
    crate::shared::tui_events::emit_log(format!("   {} Khởi tạo mô hình ONNX PP-OCRv4 Text Recognition...", "[OCR]".name_utils().violed()));
    let mut ocr_engine = PpOcrEngine::new(model_dir)?;

    // Lấy kích thước video gốc bằng ffprobe
    let probe_out = Command::new("ffprobe")
        .args(&[
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
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
        (1280usize, 720usize)
    };

    // Lấy tổng thời lượng video (giây) để hiển thị Progress
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
    let total_frames = (total_duration_sec * config.fps as f64).round() as usize;

    // Tính kích thước frame crop ROI
    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (src_w as f32 * config.roi.x_min).round() as usize;
    let crop_y = (src_h as f32 * config.roi.y_min).round() as usize;

    let vf_filter = format!("fps={},crop={}:{}:{}:{}", config.fps, crop_w, crop_h, crop_x, crop_y);
    crate::shared::tui_events::emit_log(format!("   {} Kích thước ROI: {}x{} | Filter: {}", "[OCR]".name_utils().violed(), crop_w, crop_h, vf_filter));

    // Bước 1: Mở FFmpeg pipe stream rawvideo rgb24
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

    let mut stdout = ffmpeg_child
        .stdout
        .take()
        .context("Không thể mở stdout của FFmpeg pipe")?;

    let frame_bytes = crop_w * crop_h * 3;
    let mut current_frame = vec![0u8; frame_bytes];
    let mut prev_frame = vec![0u8; frame_bytes];
    let mut has_prev = false;

    let dt = 1.0 / (config.fps as f64);
    let mut frame_idx = 0usize;
    let start_instant = std::time::Instant::now();

    let mut segments: Vec<SubtitleSegment> = Vec::new();

    // Thư mục lưu các frame phụ đề đã crop và xử lý (Raw + CLAHE) để người dùng kiểm tra trực quan
    let frames_debug_dir = output_vtt_path
        .parent()
        .map(|p| p.join("ocr_frames"))
        .unwrap_or_else(|| PathBuf::from("ocr_frames"));
    let _ = fs::create_dir_all(&frames_debug_dir);

    // Cấu trúc Session gom cụm frame theo thời gian để loại bỏ biến thể nhòe nét (fade-in / fade-out)
    struct CueSession {
        start_sec: f64,
        last_seen_sec: f64,
        candidates: std::collections::HashMap<String, usize>,
        sample_raw: Vec<u8>,
        sample_clahe: Vec<u8>,
        sample_w: usize,
        sample_h: usize,
    }

    let mut current_session: Option<CueSession> = None;
    let mut empty_streak = 0usize;
    // ✅ Biến mới: track timestamp frame đầu tiên thấy candidate (kể cả khi OCR fail)
    let mut candidate_first_sec: Option<f64> = None;
    let mut prev_had_candidates = false;
    // ✅ Biến mới: đếm số frame liên tiếp OCR fail dù có candidate (safety valve)
    let mut ocr_fail_streak = 0usize;

    crate::shared::tui_events::emit_log(format!("   {} Đang quét và nhận diện chữ từ khung hình video...", "[OCR]".name_utils().violed()));

    while stdout.read_exact(&mut current_frame).is_ok() {
        let timestamp = frame_idx as f64 * dt;
        frame_idx += 1;

        // Cập nhật Progress Bar lên giao diện TUI mỗi 16 frames (khoảng mỗi giây)
        if frame_idx % 16 == 0 || frame_idx == 1 {
            let elapsed_sec = start_instant.elapsed().as_secs();
            let dur_str = format!("{:02}:{:02}", elapsed_sec / 60, elapsed_sec % 60);
            let status_msg = format!("Đã bắt {} câu phụ đề", segments.len());
            crate::shared::tui_events::emit_task_progress(
                "OCR Hardsub Video",
                frame_idx.min(total_frames),
                total_frames.max(frame_idx),
                "frames",
                dur_str,
                status_msg,
            );
        }

        // Kiểm tra nhanh sự hiện diện của điểm ảnh chữ phụ đề ở vùng trung tâm (thăm dò siêu nhanh <0.02ms)
        let has_subtitle_candidates = {
            let mut count = 0usize;
            let y_start = crop_h / 4;
            let y_end = crop_h.saturating_sub(4);
            let x_start = crop_w / 6;
            let x_end = (crop_w * 5) / 6;
            for y in (y_start..y_end).step_by(4) {
                let y_stride = y * crop_w;
                for x in (x_start..x_end).step_by(6) {
                    let idx = (y_stride + x) * 3;
                    let r = current_frame[idx];
                    let g = current_frame[idx + 1];
                    let b = current_frame[idx + 2];
                    if r > 145 && g > 145 && b > 145 {
                        count += 1;
                        if count >= 4 {
                            break;
                        }
                    }
                }
                if count >= 4 {
                    break;
                }
            }
            count >= 4
        };

        // ✅ Track frame đầu tiên có candidate (dùng để lùi start_sec khi OCR fail frame đầu)
        if has_subtitle_candidates && !prev_had_candidates {
            candidate_first_sec = Some(timestamp);
        } else if !has_subtitle_candidates {
            candidate_first_sec = None;
        }
        prev_had_candidates = has_subtitle_candidates;


        // Tính toán mức độ biến thiên pixel (MSE) giữa 2 frame liên tiếp
        let is_scene_change = if !has_prev {
            true
        } else {
            let mut diff_sum: u64 = 0;
            let step = 12usize;
            let mut sample_count = 0usize;
            for i in (0..frame_bytes).step_by(step) {
                let d = (current_frame[i] as i32 - prev_frame[i] as i32).abs() as u64;
                diff_sum += d;
                sample_count += 1;
            }
            let avg_diff = diff_sum as f32 / (sample_count as f32 * 255.0);
            avg_diff > config.diff_threshold
        };

        let should_ocr = has_subtitle_candidates;

        if should_ocr {
            // Nhận diện OCR trực tiếp trên khung hình với Pure-Rust CLAHE + Soft Contrast Tuning
            let rec = ocr_engine.recognize_text(&current_frame, crop_w, crop_h, config.accuracy_mode).unwrap_or_default();
            let recognized = rec.text;

            if recognized.is_empty() {
                // ✅ Candidate hiện diện nhưng OCR không đọc được → KHÔNG đóng session,
                //    chỉ cần gia hạn last_seen_sec (sub có thể đang fade-in hoặc mờ nhẹ).
                if let Some(session) = current_session.as_mut() {
                    session.last_seen_sec = timestamp;
                }
                ocr_fail_streak += 1;
                // Safety valve: OCR fail 8 frame liên tiếp (~1s ở 8fps) → đóng session
                if ocr_fail_streak >= 8 {
                    if let Some(session) = current_session.take() {
                        let end_sec = session.last_seen_sec;
                        if end_sec > session.start_sec + 0.5 {
                            if let Some((best_text, _)) = session.candidates.into_iter().max_by_key(|(_, count)| *count) {
                                if !best_text.is_empty() {
                                    let cue_idx = segments.len() + 1;
                                    let cue_bmp = frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx));
                                    let _ = save_rgb_as_bmp(&cue_bmp, &session.sample_clahe, session.sample_w, session.sample_h);
                                    segments.push(SubtitleSegment {
                                        start_sec: session.start_sec,
                                        end_sec,
                                        text: best_text,
                                    });
                                }
                            }
                        }
                    }
                    candidate_first_sec = None;
                    ocr_fail_streak = 0;
                }
            } else {
                empty_streak = 0;
                ocr_fail_streak = 0;
                match current_session.as_mut() {
                    Some(session) => {
                        // Lấy biến thể có tần suất cao nhất hiện tại để đối chiếu độ tương đồng
                        let dominant_text = session
                            .candidates
                            .iter()
                            .max_by_key(|(_, count)| *count)
                            .map(|(k, _)| k.as_str())
                            .unwrap_or("");
                        let sim = text_similarity(dominant_text, &recognized);
                        let threshold = dynamic_similarity_threshold(dominant_text, &recognized);

                        if sim >= threshold {
                            // Cùng một câu phụ đề đang hiển thị: tích lũy phiếu bầu và kéo dài thời gian
                            session.last_seen_sec = timestamp;
                            let count = session.candidates.entry(recognized.clone()).or_insert(0);
                            *count += 1;
                            if *count >= 2 && !rec.raw_patch.is_empty() {
                                session.sample_raw = rec.raw_patch;
                                session.sample_clahe = rec.clahe_patch;
                                session.sample_w = rec.width;
                                session.sample_h = rec.height;
                            }
                        } else {
                            // Câu mới xuất hiện: đóng session cũ và mở session mới
                            let end_sec = session.last_seen_sec;
                            let last_seg_end = segments.last().map(|s| s.end_sec).unwrap_or(0.0);
                            let old_end = end_sec.max(last_seg_end);
                            let new_start = timestamp.max(old_end + 0.01);
                            let old_session = current_session.replace(CueSession {
                                start_sec: new_start,
                                last_seen_sec: timestamp,
                                candidates: {
                                    let mut map = std::collections::HashMap::new();
                                    map.insert(recognized, 1);
                                    map
                                },
                                sample_raw: rec.raw_patch,
                                sample_clahe: rec.clahe_patch,
                                sample_w: rec.width,
                                sample_h: rec.height,
                            }).unwrap();

                            if end_sec > old_session.start_sec + 0.35 {
                                if let Some((best_text, _)) = old_session.candidates.into_iter().max_by_key(|(_, count)| *count) {
                                    if !best_text.is_empty() {
                                        let cue_idx = segments.len() + 1;
                                        let cue_bmp = frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx));
                                        let _ = save_rgb_as_bmp(&cue_bmp, &old_session.sample_clahe, old_session.sample_w, old_session.sample_h);

                                        segments.push(SubtitleSegment {
                                            start_sec: old_session.start_sec,
                                            end_sec,
                                            text: best_text,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    None => {
                        let min_start = segments.last().map(|s| s.end_sec).unwrap_or(0.0);
                        let base_start = candidate_first_sec.unwrap_or(timestamp).max(min_start);
                        current_session = Some(CueSession {
                            start_sec: base_start,
                            last_seen_sec: timestamp,
                            candidates: {
                                let mut map = std::collections::HashMap::new();
                                map.insert(recognized, 1);
                                map
                            },
                            sample_raw: rec.raw_patch,
                            sample_clahe: rec.clahe_patch,
                            sample_w: rec.width,
                            sample_h: rec.height,
                        });
                    }
                }
            }
        } else if let Some(session) = current_session.as_mut() {
            empty_streak += 1;
            if empty_streak >= 2 {
                if let Some(session) = current_session.take() {
                    let end_sec = session.last_seen_sec;
                    if end_sec > session.start_sec + 0.5 {
                        if let Some((best_text, _)) = session.candidates.into_iter().max_by_key(|(_, count)| *count) {
                            if !best_text.is_empty() {
                                let cue_idx = segments.len() + 1;
                                let cue_bmp = frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx));
                                let _ = save_rgb_as_bmp(&cue_bmp, &session.sample_clahe, session.sample_w, session.sample_h);
                                segments.push(SubtitleSegment {
                                    start_sec: session.start_sec,
                                    end_sec,
                                    text: best_text,
                                });
                            }
                        }
                    }
                }
                candidate_first_sec = None;
            }
            
        }

        prev_frame.copy_from_slice(&current_frame);
        has_prev = true;
    }

    let _ = ffmpeg_child.wait();

    // Chốt session cuối cùng khi hết video
    if let Some(session) = current_session.take() {
        let end_sec = session.last_seen_sec;
        if end_sec > session.start_sec + 0.5 {
            if let Some((best_text, _)) = session.candidates.into_iter().max_by_key(|(_, count)| *count) {
                if !best_text.is_empty() {
                    let cue_idx = segments.len() + 1;
                    let cue_bmp = frames_debug_dir.join(format!("cue_{:03}.bmp", cue_idx));
                    let _ = save_rgb_as_bmp(&cue_bmp, &session.sample_clahe, session.sample_w, session.sample_h);

                    segments.push(SubtitleSegment {
                        start_sec: session.start_sec,
                        end_sec,
                        text: best_text,
                    });
                }
            }
        }
    }
    candidate_first_sec = None;

    // --- BƯỚC 4: Filter + Temporal Voting + Fix overlap ---
    let mut cleaned_segments: Vec<SubtitleSegment> = Vec::new();
    for seg in segments {
        let char_count = seg.text.chars().count();
        if char_count < 2 {
            continue;
        }

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

        let duration = seg.end_sec - seg.start_sec;

        // Filter watermark: sub lâu quá 15s chắc chắn là watermark
        if duration > 15.0 {
            continue;
        }

        // Min duration hạ thấp hơn cho câu ngắn (vẫn lọc được watermark)
        let min_duration = match char_count {
            0..=2 => 0.4,
            3..=4 => 0.5,
            5..=6 => 0.6,
            _ => 0.35,
        };
        if duration < min_duration {
            continue;
        }

        // Đảm bảo không overlap với segment trước
        let mut seg = seg;
        if let Some(last) = cleaned_segments.last() {
            if seg.start_sec < last.end_sec {
                seg.start_sec = last.end_sec + 0.01;
            }
        }
        if seg.end_sec <= seg.start_sec + 0.05 {
            continue;
        }

        if let Some(last) = cleaned_segments.last_mut() {
            let gap = seg.start_sec - last.end_sec;
            let sim = text_similarity(&last.text, &seg.text);
            let last_len = last.text.chars().count();
            let min_len = last_len.min(char_count);
            let merge_threshold = if min_len <= 5 { 0.70 } else { 0.85 };

            if sim >= merge_threshold && gap < 1.0 {
                // Detect ký tự thừa: nếu 1 chuỗi là substring của chuỗi kia (lệch 1-4 ký tự)
                let prefer_shorter = if last_len > char_count && last_len - char_count <= 4 {
                    last.text.contains(&seg.text) && char_count >= 4
                } else if char_count > last_len && char_count - last_len <= 4 {
                    seg.text.contains(&last.text) && last_len >= 4
                } else {
                    false
                };

                last.end_sec = last.end_sec.max(seg.end_sec);
                if prefer_shorter {
                    // Ưu tiên bản sạch hơn (không ký tự thừa)
                    if last_len > char_count {
                        last.text = seg.text;
                    }
                } else if char_count > last_len {
                    last.text = seg.text;
                }
                continue;
            }

            if last.text == seg.text && gap < 2.0 {
                last.end_sec = last.end_sec.max(seg.end_sec);
                continue;
            }
        }
        cleaned_segments.push(seg);
    }

    // Fix overlap lần cuối
    for i in 1..cleaned_segments.len() {
        if cleaned_segments[i].start_sec < cleaned_segments[i - 1].end_sec {
            cleaned_segments[i].start_sec = cleaned_segments[i - 1].end_sec + 0.01;
        }
    }
    cleaned_segments.retain(|s| s.end_sec > s.start_sec + 0.05);

    // Đảm bảo thư mục cha của file vtt tồn tại
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Lưu định dạng chuẩn WebVTT
    let mut vtt_content = String::from("WEBVTT\n\n");
    for seg in &cleaned_segments {
        let start_str = SubtitleSegment::format_vtt_timestamp(seg.start_sec);
        let end_str = SubtitleSegment::format_vtt_timestamp(seg.end_sec);
        vtt_content.push_str(&format!("{} --> {}\n{}\n\n", start_str, end_str, seg.text));
    }

    fs::write(output_vtt_path, vtt_content)
        .with_context(|| format!("Không thể ghi file WebVTT: {:?}", output_vtt_path))?;

    crate::shared::tui_events::emit_log(format!(
        "   {} {} Đã trích xuất hoàn tất phụ đề OCR từ Video: {:?} (Tổng số câu: {})",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        output_vtt_path,
        cleaned_segments.len()
    ));
    crate::shared::tui_events::emit_log(format!(
        "   {} {} Đã lưu mẫu frame phụ đề (Raw + CLAHE) vào: {:?}",
        "[OCR]".name_utils().violed(),
        Icons::SUCCESS,
        frames_debug_dir
    ));

    Ok(OcrExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments: cleaned_segments,
    })
}
