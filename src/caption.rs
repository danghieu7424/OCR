/****
 * [MODULE]: Caption Extractor (Video Caption OCR) - Industrial Grade
 * Chức năng: Trích xuất chú thích (caption) từ video sang chuẩn WebVTT (.captions.zh.vtt)
 *
 * Caption khác Subtitle:
 * - Caption: tên nhân vật, chú thích ngữ cảnh, trên thanh đen dọc bên trái hoặc text ngang lớn
 * - Subtitle: lời thoại, nằm dưới cùng màn hình (85%-100%)
 *
 * Hỗ trợ 2 loại caption:
 * 1. Chữ DỌC (vertical): trên thanh đen, mỗi ký tự 1 box riêng → nhóm cột, đọc trên→dưới, trái→phải
 * 2. Chữ NGANG (horizontal): text ngang → OCR trực tiếp từ run_from_image()
 *
 * Thuật toán chống watermark:
 * - Phase 1: Scan tất cả frames, thu thập raw text boxes
 * - Phase 2: Tính fingerprint → text xuất hiện >40% frames ở vùng tương tự = watermark → loại bỏ
 * - Phase 3: Ghép boxes còn lại thành caption text + temporal denoising
 ****/

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use image::{DynamicImage, RgbImage};
use pure_onnx_ocr::OcrEngine;

use crate::shared::icons::Icons;
use crate::shared::terminal_color::NameUtilsExt;

/// Cấu hình vùng ROI cho caption
#[derive(Debug, Clone)]
pub struct CaptionRoi {
    pub y_min: f32,
    pub y_max: f32,
    pub x_min: f32,
    pub x_max: f32,
}

impl Default for CaptionRoi {
    fn default() -> Self {
        Self {
            y_min: 0.10,
            y_max: 0.80,
            // Caption có thể xuất hiện ở bất kỳ vị trí ngang nào (canh trái hoặc
            // canh giữa màn hình, vd: 《系统》), nên ROI phủ toàn bộ chiều rộng.
            x_min: 0.00,
            x_max: 1.00,
        }
    }
}

/// Cấu hình trích xuất caption
#[derive(Debug, Clone)]
pub struct CaptionExtractorConfig {
    pub fps: u32,
    pub roi: CaptionRoi,
    /// h > w * threshold → dọc
    pub vertical_aspect_threshold: f32,
    /// Gom cột: khoảng cách X tỷ lệ so với chiều rộng ROI
    pub column_x_tolerance: f32,
    /// Ngưỡng tin cậy tối thiểu. Caption là chữ được render sắc nét nên điểm
    /// tin cậy thường ~1.0; chữ trong cảnh (biển hiệu, chữ vàng trên sách)
    /// thường chỉ 0.7-0.8. Hạ xuống 0.30 sẽ bắt thêm được caption mờ/nhòe
    /// nhưng cũng kéo theo chữ nằm trong cảnh — hạ kèm `min_duration_sec`>1s
    /// nếu video có nhiều biển hiệu/sách chữ.
    pub min_confidence: f32,
    /// Thời lượng tối thiểu (giây) của một cụm caption. Caption thật hiển thị
    /// đủ lâu để đọc, còn chữ nằm trong cảnh chỉ lướt qua vài khung hình.
    /// Mặc định 1.0s; hạ xuống ~0.4s nếu video có caption rất ngắn (đổi lại
    /// dễ bắt nhầm chữ trên sách/biển hiệu).
    pub min_duration_sec: f64,
    /// Ngưỡng tỷ lệ frame xuất hiện để coi là watermark (0.0-1.0)
    pub watermark_freq_threshold: f32,
}

impl Default for CaptionExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 5,
            roi: CaptionRoi::default(),
            vertical_aspect_threshold: 1.2,
            column_x_tolerance: 0.06,
            min_confidence: 0.80,
            min_duration_sec: 1.0,
            watermark_freq_threshold: 0.40,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CaptionSegment {
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
}

impl CaptionSegment {
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

#[derive(Debug, Clone)]
pub struct CaptionExtractorOutput {
    pub vtt_path: PathBuf,
    pub segments: Vec<CaptionSegment>,
}

/// Text box detected bởi DBNet + CTC
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TextBox {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    width: f64,
    height: f64,
    center_x: f64,
    center_y: f64,
    is_vertical: bool,
    text: String,
    confidence: f32,
}

/// Fingerprint cho watermark detection: vị trí trung bình + text
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RegionKey {
    /// Vị trí lượng tử hóa (chia grid 10x10)
    qx: u32,
    qy: u32,
}

fn detect_and_recognize(
    engine: &OcrEngine,
    rgb_bytes: &[u8],
    width: usize,
    height: usize,
    vertical_threshold: f32,
    min_confidence: f32,
) -> Result<Vec<TextBox>> {
    let img_buf = RgbImage::from_raw(width as u32, height as u32, rgb_bytes.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Không thể tạo RgbImage"))?;
    let dynamic_img = DynamicImage::ImageRgb8(img_buf);

    let results = engine.run_from_image(&dynamic_img)?;
    let mut boxes = Vec::new();

    for result in &results {
        if result.confidence < min_confidence {
            continue;
        }

        let points: Vec<(f64, f64)> = result
            .bounding_box
            .exterior()
            .points()
            .map(|p| (p.x(), p.y()))
            .collect();

        if points.is_empty() { continue; }

        let min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);

        let w = max_x - min_x;
        let h = max_y - min_y;
        if w < 5.0 || h < 5.0 { continue; }

        let clean_text = sanitize_caption_text(&result.text);
        if clean_text.is_empty() { continue; }

        let stats = box_pixel_stats(rgb_bytes, width, height, min_x, min_y, max_x, max_y);
        let ink_ok = ink_looks_like_caption(rgb_bytes, width, height, min_x, min_y, max_x, max_y);
        let scene_ok = stats.sat_p90 <= CAPTION_MAX_SCENE_SAT_P90;

        let is_vertical = is_vertical_box(w, h, vertical_threshold);

        // Cột dọc đọc thiếu ký tự so với số ký tự hình học cho phép → thử lại
        // trên ảnh đảo màu, chỉ nhận bản mở rộng đúng chuỗi đã đọc.
        let mut caption_text = clean_text;
        if is_vertical && ink_ok {
            let expected = (h / w).round().max(1.0) as usize;
            if expected > caption_text.chars().count() {
                if let Some(better) = retry_vertical_missing_chars(
                    engine, &dynamic_img, width, height,
                    min_x, min_y, max_x, max_y, expected, &caption_text,
                ) {
                    caption_text = better;
                }
            }
        }

        let shape_ok = shape_matches_text(w, h, &caption_text);
        let accept = ink_ok && shape_ok && scene_ok;

        if std::env::var_os("OCR_CAP_DEBUG").is_some() {
            eprintln!(
                "[BOX] ({:.0},{:.0})-({:.0},{:.0}) w={:.0} h={:.0} '{}' conf={:.2} ink={} shape={} scene={} | lum_med={:.0} dark={:.2} sat_med={:.2} sat_p90={:.2} sat_hi={:.2}",
                min_x, min_y, max_x, max_y, w, h, caption_text, result.confidence,
                ink_ok as u8, shape_ok as u8, scene_ok as u8,
                stats.lum_med, stats.dark_frac, stats.sat_med, stats.sat_p90, stats.sat_hi,
            );
        }

        if !accept { continue; }

        boxes.push(TextBox {
            min_x, min_y, max_x, max_y,
            width: w, height: h,
            center_x: (min_x + max_x) / 2.0,
            center_y: (min_y + max_y) / 2.0,
            is_vertical,
            text: caption_text,
            confidence: result.confidence,
        });
    }
    Ok(boxes)
}

/// Ngưỡng bão hoà màu tối đa của nét chữ caption.
/// Caption là chữ trắng/nhạt nên các điểm sáng nhất trong box gần như vô màu;
/// chữ nằm trong cảnh (chữ đỏ 囍, chữ vàng trên sách/cuộn) có độ bão hoà cao.
const CAPTION_MAX_INK_SAT: f32 = 0.33;

/// Ngưỡng bão hoà phân vị 90% của **toàn box**. Caption là chữ trắng/nhạt đè
/// lên nền tối gần như vô màu, nên gần như mọi pixel trong box đều ít bão hoà.
/// Box bắt trên vật thể cảnh có màu mạnh — băng rôn/cờ đỏ (vd: 《社》 tập 9),
/// chữ vàng trên sách — có phần lớn pixel bão hoà cao dù nét chữ nhạt.
const CAPTION_MAX_SCENE_SAT_P90: f32 = 0.45;

/// Kiểm tra nét chữ trong box có phải kiểu chữ caption (trắng/nhạt) không.
fn ink_looks_like_caption(
    rgb: &[u8],
    img_w: usize,
    img_h: usize,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> bool {
    let x0 = min_x.max(0.0) as usize;
    let y0 = min_y.max(0.0) as usize;
    let x1 = (max_x as usize).min(img_w);
    let y1 = (max_y as usize).min(img_h);
    if x1 <= x0 || y1 <= y0 {
        return false;
    }

    let mut pixels: Vec<(f32, f32)> = Vec::with_capacity((x1 - x0) * (y1 - y0));
    for y in y0..y1 {
        for x in x0..x1 {
            let idx = (y * img_w + x) * 3;
            let (r, g, b) = (rgb[idx] as f32, rgb[idx + 1] as f32, rgb[idx + 2] as f32);
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
            let lum = 0.299 * r + 0.587 * g + 0.114 * b;
            pixels.push((lum, sat));
        }
    }
    if pixels.is_empty() {
        return false;
    }

    // Lấy 25% điểm sáng nhất (nét chữ) rồi đo độ bão hoà trung vị.
    pixels.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let top = (pixels.len() / 4).max(1);
    let mut sats: Vec<f32> = pixels[..top].iter().map(|p| p.1).collect();
    sats.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sats[sats.len() / 2] <= CAPTION_MAX_INK_SAT
}

/// Thống kê pixel trong box, dùng để phân biệt caption với vật thể trong cảnh.
struct BoxPixels {
    lum_med: f32,
    dark_frac: f32,
    sat_med: f32,
    sat_p90: f32,
    sat_hi: f32,
}

fn box_pixel_stats(
    rgb: &[u8],
    img_w: usize,
    img_h: usize,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
) -> BoxPixels {
    let x0 = min_x.max(0.0) as usize;
    let y0 = min_y.max(0.0) as usize;
    let x1 = (max_x as usize).min(img_w);
    let y1 = (max_y as usize).min(img_h);

    let mut lums: Vec<f32> = Vec::new();
    let mut sats: Vec<f32> = Vec::new();
    let mut dark = 0usize;

    if x1 > x0 && y1 > y0 {
        for y in y0..y1 {
            for x in x0..x1 {
                let idx = (y * img_w + x) * 3;
                let (r, g, b) = (rgb[idx] as f32, rgb[idx + 1] as f32, rgb[idx + 2] as f32);
                let mx = r.max(g).max(b);
                let mn = r.min(g).min(b);
                let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
                let lum = 0.299 * r + 0.587 * g + 0.114 * b;
                if lum < 60.0 {
                    dark += 1;
                }
                lums.push(lum);
                sats.push(sat);
            }
        }
    }

    if lums.is_empty() {
        return BoxPixels { lum_med: 0.0, dark_frac: 0.0, sat_med: 0.0, sat_p90: 1.0, sat_hi: 1.0 };
    }

    let n = lums.len();
    lums.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sats.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |sorted: &[f32], q: f32| sorted[((sorted.len() as f32 - 1.0) * q) as usize];
    let sat_hi = sats.iter().filter(|&&s| s > 0.45).count() as f32 / n as f32;

    BoxPixels {
        lum_med: pct(&lums, 0.5),
        dark_frac: dark as f32 / n as f32,
        sat_med: pct(&sats, 0.5),
        sat_p90: pct(&sats, 0.9),
        sat_hi,
    }
}

/// Box là chữ dọc khi cao hơn rộng theo ngưỡng tỷ lệ.
fn is_vertical_box(width: f64, height: f64, aspect_threshold: f32) -> bool {
    height > width * aspect_threshold as f64
}

/// Kiểm tra hình học box có khớp với số ký tự nhận diện được không.
/// Chữ Hán gần như vuông: với caption ngang, mỗi ký tự rộng ~ w/n và cao ~ h;
/// với caption dọc, mỗi ký tự cao ~ h/n và rộng ~ w. Box lệch xa tỷ lệ này là
/// chữ trong cảnh (biển hiệu, chữ vàng trên sách...) chứ không phải caption.
fn shape_matches_text(w: f64, h: f64, text: &str) -> bool {
    let n = text.chars().count().max(1) as f64;
    if w <= 0.0 || h <= 0.0 {
        return false;
    }
    let is_vertical = is_vertical_box(w, h, 1.2);
    let char_aspect = if is_vertical { w * n / h } else { w / (n * h) };
    (0.50..=1.80).contains(&char_aspect)
}

/// Chữ dọc hiếm nét (vd: 訇) có thể bị CTC bỏ qua khi đọc cả cột. Thử lại trên
/// ảnh đảo màu (chữ đen trên nền trắng) — cách đọc đủ "何訇" thay vì chỉ "何" —
/// và chỉ nhận khi kết quả mở rộng đúng chuỗi đã đọc được và khớp số ký tự mà
/// hình học box cho phép.
#[allow(clippy::too_many_arguments)]
fn retry_vertical_missing_chars(
    engine: &OcrEngine,
    img: &DynamicImage,
    img_w: usize,
    img_h: usize,
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    expected_chars: usize,
    known: &str,
) -> Option<String> {
    let x0 = min_x.max(0.0) as u32;
    let y0 = min_y.max(0.0) as u32;
    let x1 = (max_x as u32).min(img_w as u32);
    let y1 = (max_y as u32).min(img_h as u32);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }

    let mut crop = img.crop_imm(x0, y0, x1 - x0, y1 - y0).to_rgb8();
    for p in crop.pixels_mut() {
        p.0 = [255 - p.0[0], 255 - p.0[1], 255 - p.0[2]];
    }

    let results = engine.run_from_image(&DynamicImage::ImageRgb8(crop)).ok()?;
    results
        .iter()
        .map(|r| sanitize_caption_text(&r.text))
        .filter(|t| t.chars().count() == expected_chars && t.starts_with(known))
        .max_by_key(|t| t.chars().count())
}

fn sanitize_caption_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() { return String::new(); }

    let cleaned: String = trimmed
        .chars()
        .filter(|&c| {
            ('\u{4e00}'..='\u{9fff}').contains(&c)
                || ('\u{3400}'..='\u{4dbf}').contains(&c)
                || c.is_ascii_alphanumeric()
                || c == '-' || c == '—' || c == '·' || c == '…'
        })
        .collect();

    let has_cjk = cleaned.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk { return String::new(); }
    cleaned
}

/// Lượng tử hóa vị trí box thành grid key (để nhóm boxes ở vị trí tương tự)
fn region_key(center_x: f64, center_y: f64, img_w: f64, img_h: f64) -> RegionKey {
    let qx = ((center_x / img_w) * 10.0).floor() as u32;
    let qy = ((center_y / img_h) * 10.0).floor() as u32;
    RegionKey { qx: qx.min(9), qy: qy.min(9) }
}

/// Phase 2: Xác định watermark fingerprints
/// Watermark = text xuất hiện ở cùng vùng trong >threshold% tổng frames
fn build_watermark_set(
    all_frame_boxes: &[Vec<TextBox>],
    img_w: f64,
    img_h: f64,
    threshold: f32,
) -> HashMap<RegionKey, String> {
    let total_frames = all_frame_boxes.len();
    if total_frames == 0 {
        return HashMap::new();
    }

    // Đếm: (region, text_prefix_4chars) → số frame xuất hiện
    let mut freq: HashMap<(RegionKey, String), usize> = HashMap::new();
    // Lưu text đại diện cho mỗi (region, prefix)
    let mut representative: HashMap<(RegionKey, String), String> = HashMap::new();

    for frame_boxes in all_frame_boxes {
        // Đánh dấu để không đếm trùng trong 1 frame
        let mut seen_this_frame: HashMap<(RegionKey, String), bool> = HashMap::new();

        for b in frame_boxes {
            let rk = region_key(b.center_x, b.center_y, img_w, img_h);
            // Dùng 3 ký tự đầu làm prefix key (để nhóm các biến thể OCR của cùng watermark)
            let prefix: String = b.text.chars().take(3).collect();
            let key = (rk.clone(), prefix);

            if !seen_this_frame.contains_key(&key) {
                seen_this_frame.insert(key.clone(), true);
                *freq.entry(key.clone()).or_insert(0) += 1;
                representative.entry(key).or_insert_with(|| b.text.clone());
            }
        }
    }

    // Xác định watermark: region+prefix xuất hiện trong >threshold% frames
    let mut watermarks = HashMap::new();
    let min_count = (total_frames as f32 * threshold) as usize;

    for ((rk, _prefix), count) in &freq {
        if *count >= min_count {
            if let Some(rep) = representative.get(&(rk.clone(), _prefix.clone())) {
                watermarks.insert(rk.clone(), rep.clone());
            }
        }
    }

    watermarks
}

/// Kiểm tra box có phải watermark không
fn is_watermark(b: &TextBox, watermarks: &HashMap<RegionKey, String>, img_w: f64, img_h: f64) -> bool {
    let rk = region_key(b.center_x, b.center_y, img_w, img_h);
    watermarks.contains_key(&rk)
}

/// Nhóm boxes dọc thành cột, sắp xếp: mỗi cột y↓, các cột x→
fn group_vertical_columns(boxes: &[TextBox], tolerance_px: f64) -> Vec<Vec<&TextBox>> {
    if boxes.is_empty() { return Vec::new(); }

    let mut sorted_idx: Vec<usize> = (0..boxes.len()).collect();
    sorted_idx.sort_by(|&a, &b| boxes[a].center_x.partial_cmp(&boxes[b].center_x).unwrap());

    let mut columns: Vec<Vec<&TextBox>> = Vec::new();
    for &idx in &sorted_idx {
        let b = &boxes[idx];
        let mut found = false;
        for col in columns.iter_mut() {
            let avg_x: f64 = col.iter().map(|cb| cb.center_x).sum::<f64>() / col.len() as f64;
            let avg_w: f64 = col.iter().map(|cb| cb.width).sum::<f64>() / col.len() as f64;
            // Dung sai phải co theo bề rộng ký tự, không theo bề rộng ROI:
            // ROI full-width mà dùng 6% chiều rộng sẽ gộp 2 cột chữ dọc liền kề
            // (vd: 《大女儿》 và 《陆紫萱》) thành một cột duy nhất.
            let tol = tolerance_px.min(0.6 * b.width.min(avg_w));
            if (b.center_x - avg_x).abs() < tol {
                col.push(b);
                found = true;
                break;
            }
        }
        if !found {
            columns.push(vec![b]);
        }
    }

    for col in columns.iter_mut() {
        col.sort_by(|a, b| a.center_y.partial_cmp(&b.center_y).unwrap());
    }
    columns.sort_by(|a, b| {
        let ax: f64 = a.iter().map(|cb| cb.center_x).sum::<f64>() / a.len() as f64;
        let bx: f64 = b.iter().map(|cb| cb.center_x).sum::<f64>() / b.len() as f64;
        ax.partial_cmp(&bx).unwrap()
    });
    columns
}

/// Ghép filtered boxes thành caption text cho 1 frame
fn assemble_caption(boxes: &[TextBox], column_x_tol: f64) -> String {
    if boxes.is_empty() { return String::new(); }

    let vert: Vec<&TextBox> = boxes.iter().filter(|b| b.is_vertical).collect();
    let horiz: Vec<&TextBox> = boxes.iter().filter(|b| !b.is_vertical).collect();

    let mut parts: Vec<String> = Vec::new();

    // Chữ dọc: nhóm cột
    if !vert.is_empty() {
        let vert_owned: Vec<TextBox> = vert.iter().map(|b| (*b).clone()).collect();
        let columns = group_vertical_columns(&vert_owned, column_x_tol);
        for col in &columns {
            let col_text: String = col.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("");
            if !col_text.is_empty() {
                parts.push(col_text);
            }
        }
    }

    // Chữ ngang
    if !horiz.is_empty() {
        let mut h_sorted: Vec<&&TextBox> = horiz.iter().collect();
        h_sorted.sort_by(|a, b| {
            a.center_y.partial_cmp(&b.center_y).unwrap()
                .then_with(|| a.center_x.partial_cmp(&b.center_x).unwrap())
        });
        for hb in &h_sorted {
            if !hb.text.is_empty() {
                parts.push(hb.text.clone());
            }
        }
    }

    if parts.len() > 1 {
        let all_short = parts.iter().all(|p| p.chars().count() <= 5);
        if all_short { parts.join("-") } else { parts.join("") }
    } else {
        parts.into_iter().next().unwrap_or_default()
    }
}

fn is_same_caption(s1: &str, s2: &str) -> bool {
    if s1 == s2 { return true; }
    if s1.is_empty() || s2.is_empty() { return false; }
    let l1 = s1.chars().count();
    let l2 = s2.chars().count();
    let mx = l1.max(l2);
    let mn = l1.min(l2);
    if mn <= 2 { return s1 == s2; }
    let dist = levenshtein_distance(s1, s2);
    if mn <= 4 { dist <= 1 } else { (1.0 - dist as f32 / mx as f32) >= 0.70 && dist <= 2 }
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let c1: Vec<char> = s1.chars().collect();
    let c2: Vec<char> = s2.chars().collect();
    let (l1, l2) = (c1.len(), c2.len());
    if l1 == 0 { return l2; }
    if l2 == 0 { return l1; }
    let mut prev: Vec<usize> = (0..=l2).collect();
    let mut curr: Vec<usize> = vec![0; l2 + 1];
    for i in 1..=l1 {
        curr[0] = i;
        for j in 1..=l2 {
            let cost = if c1[i - 1] == c2[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    curr[l2]
}

/// Chuẩn hóa khoá so sánh: bỏ dấu nối giữa các cột chữ dọc.
fn caption_key(s: &str) -> String {
    s.chars().filter(|c| *c != '-').collect()
}

fn select_best_caption(texts: &[String]) -> String {
    if texts.is_empty() { return String::new(); }

    // Gom các biến thể chỉ khác nhau ở dấu nối cột (vd: "大女儿-陆紫萱" và
    // "大女儿陆紫萱") về cùng một nhóm để bầu chọn ổn định, và ưu tiên biến
    // thể có dấu nối (giữ đúng thứ tự cột trái → phải).
    let mut groups: HashMap<String, (usize, String)> = HashMap::new();
    for t in texts.iter().filter(|t| !t.is_empty()) {
        let key = caption_key(t);
        let entry = groups.entry(key).or_insert((0, t.clone()));
        entry.0 += 1;
        if t.contains('-') && !entry.1.contains('-') {
            entry.1 = t.clone();
        } else if t.chars().count() > entry.1.chars().count() {
            entry.1 = t.clone();
        }
    }

    groups
        .into_iter()
        .max_by(|(k1, (c1, t1)), (k2, (c2, t2))| {
            c1.cmp(c2)
                .then_with(|| t1.chars().count().cmp(&t2.chars().count()))
                .then_with(|| k2.cmp(k1))
        })
        .map(|(_, (_, t))| t)
        .unwrap_or_default()
}

// =====================================================================
// HÀM CHÍNH
// =====================================================================
pub fn extract_captions(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &CaptionExtractorConfig,
) -> Result<CaptionExtractorOutput> {
    if !video_path.exists() {
        anyhow::bail!("Video không tồn tại: {:?}", video_path);
    }

    crate::shared::tui_events::emit_step(">>> [CAPTION] Trích xuất Caption/Chú thích từ Video...");
    crate::shared::tui_events::emit_log(format!(
        "   {} Caption ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%] | Fps: {}",
        "[CAPTION]".name_utils().violed(),
        config.roi.y_min * 100.0, config.roi.y_max * 100.0,
        config.roi.x_min * 100.0, config.roi.x_max * 100.0, config.fps,
    ));

    let det_path = model_dir.join("ocr/det.onnx");
    let rec_path = model_dir.join("ocr/rec.onnx");
    let keys_path = model_dir.join("ocr/keys.txt");

    crate::shared::tui_events::emit_log(format!(
        "   {} Khởi tạo Caption OCR Engine...", "[CAPTION]".name_utils().violed()
    ));
    let ocr_engine = crate::ocr::det_filter::create_det_engine(
        &det_path.to_string_lossy(), &rec_path.to_string_lossy(), &keys_path.to_string_lossy(),
    )?;

    // Video info
    let probe = Command::new("ffprobe")
        .args(&["-v","error","-select_streams","v:0","-show_entries","stream=width,height","-of","csv=p=0",
            &video_path.to_string_lossy()])
        .output().context("ffprobe failed")?;
    let dim_str = String::from_utf8_lossy(&probe.stdout);
    let dims: Vec<usize> = dim_str.trim().split(',').filter_map(|s| s.trim().parse().ok()).collect();
    let (src_w, src_h) = if dims.len() == 2 { (dims[0], dims[1]) } else { (1920, 1080) };

    let dur = Command::new("ffprobe")
        .args(&["-v","error","-select_streams","v:0","-show_entries","format=duration",
            "-of","default=noprint_wrappers=1:nokey=1", &video_path.to_string_lossy()])
        .output();
    let total_duration: f64 = dur.ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(600.0);

    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (((src_w as f32 * config.roi.x_min).round() as usize) / 2) * 2;
    let crop_y = (((src_h as f32 * config.roi.y_min).round() as usize) / 2) * 2;

    let vf_filter = format!("fps={},crop={}:{}:{}:{}", config.fps, crop_w, crop_h, crop_x, crop_y);
    crate::shared::tui_events::emit_log(format!(
        "   {} ROI: {}x{} @ ({},{}) | {}", "[CAPTION]".name_utils().violed(),
        crop_w, crop_h, crop_x, crop_y, vf_filter
    ));

    let column_x_tol = crop_w as f64 * config.column_x_tolerance as f64;

    // FFmpeg pipe
    let mut ffmpeg = Command::new("ffmpeg")
        .args(&["-i", &video_path.to_string_lossy(), "-vf", &vf_filter,
            "-f", "rawvideo", "-pix_fmt", "rgb24", "-v", "error", "-"])
        .stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().context("FFmpeg caption start failed")?;
    let mut stdout = ffmpeg.stdout.take().context("FFmpeg stdout")?;

    let frame_bytes = crop_w * crop_h * 3;
    let mut buf = vec![0u8; frame_bytes];
    let dt = 1.0 / config.fps as f64;
    let mut frame_idx = 0usize;
    let est_frames = (total_duration * config.fps as f64) as usize;

    // ===== PHASE 1: Thu thập raw boxes mọi frame =====
    crate::shared::tui_events::emit_log(format!(
        "   {} Phase 1: Quét {} frames...", "[CAPTION]".name_utils().violed(), est_frames
    ));

    let mut all_frame_boxes: Vec<Vec<TextBox>> = Vec::new();
    let mut timestamps: Vec<f64> = Vec::new();

    while stdout.read_exact(&mut buf).is_ok() {
        let ts = frame_idx as f64 * dt;
        frame_idx += 1;

        let boxes = detect_and_recognize(
            &ocr_engine, &buf, crop_w, crop_h,
            config.vertical_aspect_threshold, config.min_confidence,
        )?;

        all_frame_boxes.push(boxes);
        timestamps.push(ts);

        if frame_idx % (config.fps as usize * 5) == 0 {
            let pct = (ts / total_duration * 100.0).min(100.0);
            crate::shared::tui_events::emit_task_progress(
                "Caption Phase 1", frame_idx, est_frames, "frames",
                format!("{:.1}s", ts), format!("{:.1}%", pct),
            );
        }
    }
    let _ = ffmpeg.wait();
    let total = all_frame_boxes.len();

    // ===== PHASE 2: Xác định & loại bỏ watermark =====
    crate::shared::tui_events::emit_log(format!(
        "   {} Phase 2: Lọc watermark ({} frames)...", "[CAPTION]".name_utils().violed(), total
    ));

    let watermarks = build_watermark_set(
        &all_frame_boxes, crop_w as f64, crop_h as f64, config.watermark_freq_threshold,
    );

    crate::shared::tui_events::emit_log(format!(
        "   {} Phát hiện {} vùng watermark cố định", "[CAPTION]".name_utils().violed(), watermarks.len()
    ));

    // ===== PHASE 3: Ghép caption text (đã lọc watermark) =====
    let mut records: Vec<String> = Vec::with_capacity(total);
    for frame_boxes in &all_frame_boxes {
        let filtered: Vec<TextBox> = frame_boxes.iter()
            .filter(|b| !is_watermark(b, &watermarks, crop_w as f64, crop_h as f64))
            .cloned()
            .collect();
        records.push(assemble_caption(&filtered, column_x_tol));
    }

    // ===== PHASE 4: Temporal denoising =====
    crate::shared::tui_events::emit_log(format!(
        "   {} Phase 3: Khử nhiễu temporal...", "[CAPTION]".name_utils().violed()
    ));

    // 1-frame dropout fill
    for i in 1..total.saturating_sub(1) {
        if records[i].is_empty() && !records[i-1].is_empty() && !records[i+1].is_empty() {
            if is_same_caption(&records[i-1], &records[i+1]) {
                records[i] = records[i-1].clone();
            }
        }
    }
    // 2-frame dropout fill
    for i in 1..total.saturating_sub(2) {
        if records[i].is_empty() && records[i+1].is_empty()
            && !records[i-1].is_empty() && !records[i+2].is_empty()
            && is_same_caption(&records[i-1], &records[i+2])
        {
            records[i] = records[i-1].clone();
            records[i+1] = records[i-1].clone();
        }
    }
    // 1-frame spike removal
    for i in 1..total.saturating_sub(1) {
        if !records[i].is_empty() && records[i-1].is_empty() && records[i+1].is_empty() {
            records[i] = String::new();
        }
    }
    // 1-frame typo smooth
    for i in 1..total.saturating_sub(1) {
        if !records[i-1].is_empty() && !records[i+1].is_empty() && !records[i].is_empty()
            && records[i] != records[i-1] && records[i-1] == records[i+1]
            && is_same_caption(&records[i-1], &records[i])
        {
            records[i] = records[i-1].clone();
        }
    }

    // Phân cụm temporal
    struct RawSeg { start: usize, end: usize, texts: Vec<String> }
    let mut raw_segs: Vec<RawSeg> = Vec::new();
    let mut cur: Option<RawSeg> = None;

    for i in 0..total {
        let t = &records[i];
        if t.is_empty() {
            if let Some(seg) = cur.take() { raw_segs.push(seg); }
        } else {
            match cur.as_mut() {
                Some(seg) => {
                    let dom = select_best_caption(&seg.texts);
                    if is_same_caption(&dom, t) {
                        seg.end = i;
                        seg.texts.push(t.clone());
                    } else {
                        let old = cur.replace(RawSeg { start: i, end: i, texts: vec![t.clone()] }).unwrap();
                        raw_segs.push(old);
                    }
                }
                None => { cur = Some(RawSeg { start: i, end: i, texts: vec![t.clone()] }); }
            }
        }
    }
    if let Some(seg) = cur.take() { raw_segs.push(seg); }

    // Tạo segments
    let mut segments: Vec<CaptionSegment> = Vec::new();
    for seg in raw_segs {
        // Caption phải hiển thị đủ lâu để đọc; chữ nằm trong cảnh (sách, cuộn,
        // biển hiệu) chỉ lướt qua vài khung hình nên bị loại.
        let min_frames = (config.min_duration_sec * config.fps as f64).ceil() as usize;
        if seg.end.saturating_sub(seg.start) + 1 < min_frames.max(1) { continue; }
        let best = select_best_caption(&seg.texts);
        if best.is_empty() { continue; }

        let start_sec = (seg.start as f64 * dt).max(0.0);
        let end_sec = seg.end as f64 * dt + dt * 0.5;
        let start_sec = if let Some(last) = segments.last() {
            start_sec.max(last.end_sec + 0.001)
        } else { start_sec };

        if end_sec > start_sec + 0.1 {
            segments.push(CaptionSegment {
                start_sec, end_sec,
                text: format!("《{}》", best),
            });
        }
    }

    // Gộp liền kề
    let mut final_segs: Vec<CaptionSegment> = Vec::new();
    for seg in segments {
        if let Some(last) = final_segs.last_mut() {
            let gap = seg.start_sec - last.end_sec;
            if is_same_caption(&last.text, &seg.text) && gap < 0.8 {
                last.end_sec = seg.end_sec;
                if seg.text.chars().count() > last.text.chars().count() { last.text = seg.text; }
                continue;
            }
        }
        final_segs.push(seg);
    }

    // Ghi WebVTT
    if let Some(parent) = output_vtt_path.parent() { fs::create_dir_all(parent)?; }
    let mut vtt = String::from("WEBVTT\n\n");
    for seg in &final_segs {
        let s = CaptionSegment::format_vtt_timestamp(seg.start_sec);
        let e = CaptionSegment::format_vtt_timestamp(seg.end_sec);
        vtt.push_str(&format!("{} --> {} line:20%\n{}\n\n", s, e, seg.text));
    }
    fs::write(output_vtt_path, vtt)
        .with_context(|| format!("Không thể ghi caption: {:?}", output_vtt_path))?;

    crate::shared::tui_events::emit_log(format!(
        "   {} {} Hoàn tất: {:?} ({} caption)",
        "[CAPTION]".name_utils().violed(), Icons::SUCCESS, output_vtt_path, final_segs.len()
    ));

    Ok(CaptionExtractorOutput { vtt_path: output_vtt_path.to_path_buf(), segments: final_segs })
}
