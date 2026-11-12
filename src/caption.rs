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
 * Quy trình:
 * 1. ROI Cropping: vùng caption (y 5%-75%, x 0%-50% nửa trái)
 * 2. DBNet + CTC (pure_onnx_ocr): detect boxes + recognize text → OcrResult{text, confidence, bounding_box}
 * 3. Layout Analysis: aspect ratio h>w*1.5 → dọc; h<=w*1.5 → ngang
 * 4. Vertical Column Grouping: nhóm boxes dọc thành cột → sắp xếp y↓ rồi x→ → ghép ký tự
 * 5. Temporal Denoising & Majority Voting → phân đoạn caption segments
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
            y_min: 0.05, // Bỏ watermark trên cùng
            y_max: 0.75, // Không lấn vào vùng subtitle 85%-100%
            x_min: 0.00, // Quét từ lề trái
            x_max: 0.50, // Nửa trái — caption dọc luôn ở bên trái
        }
    }
}

/// Cấu hình trích xuất caption
#[derive(Debug, Clone)]
pub struct CaptionExtractorConfig {
    /// Tần số lấy mẫu (mặc định: 5fps — caption thay đổi chậm hơn subtitle)
    pub fps: u32,
    /// Vùng ROI quét caption
    pub roi: CaptionRoi,
    /// Ngưỡng aspect ratio phân biệt dọc/ngang: h > w * threshold → dọc
    pub vertical_aspect_threshold: f32,
    /// Ngưỡng khoảng cách X gom cột (tỷ lệ so với chiều rộng ROI)
    pub column_x_tolerance: f32,
    /// Ngưỡng confidence tối thiểu để chấp nhận kết quả OCR
    pub min_confidence: f32,
}

impl Default for CaptionExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 5,
            roi: CaptionRoi::default(),
            vertical_aspect_threshold: 1.2, // h > 1.2*w → chữ dọc (ký tự CJK gần vuông)
            column_x_tolerance: 0.10,        // box cách < 10% chiều rộng ROI → cùng cột
            min_confidence: 0.30,
        }
    }
}

/// Một phân đoạn caption
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

/// Kết quả đầu ra
#[derive(Debug, Clone)]
pub struct CaptionExtractorOutput {
    pub vtt_path: PathBuf,
    pub segments: Vec<CaptionSegment>,
}

/// Thông tin text box (AABB từ polygon)
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
    /// Text đã nhận diện từ OcrResult
    text: String,
    confidence: f32,
}

/// Detect + recognize tất cả text trong frame
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
        // Lọc confidence thấp
        if result.confidence < min_confidence {
            continue;
        }

        let points: Vec<(f64, f64)> = result
            .bounding_box
            .exterior()
            .points()
            .map(|p| (p.x(), p.y()))
            .collect();

        if points.is_empty() {
            continue;
        }

        let min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);

        let w = max_x - min_x;
        let h = max_y - min_y;

        if w < 5.0 || h < 5.0 {
            continue;
        }

        // Lọc text rỗng hoặc chỉ có ký tự rác
        let clean_text = sanitize_caption_text(&result.text);
        if clean_text.is_empty() {
            continue;
        }

        let is_vertical = h > w * vertical_threshold as f64;

        boxes.push(TextBox {
            min_x, min_y, max_x, max_y,
            width: w, height: h,
            center_x: (min_x + max_x) / 2.0,
            center_y: (min_y + max_y) / 2.0,
            is_vertical,
            text: clean_text,
            confidence: result.confidence,
        });
    }

    Ok(boxes)
}

/// Làm sạch caption text
fn sanitize_caption_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let cleaned: String = trimmed
        .chars()
        .filter(|&c| {
            ('\u{4e00}'..='\u{9fff}').contains(&c)      // CJK Unified
                || ('\u{3400}'..='\u{4dbf}').contains(&c) // CJK Extension A
                || c.is_ascii_alphanumeric()
                || c == '-' || c == '—' || c == '·'
                || c == '…'
        })
        .collect();

    // Phải có ít nhất 1 ký tự CJK
    let has_cjk = cleaned.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    if !has_cjk {
        return String::new();
    }

    cleaned
}

/// Nhóm boxes dọc thành cột, sắp xếp: mỗi cột y↓, các cột x→
fn group_vertical_columns(boxes: &[TextBox], tolerance_px: f64) -> Vec<Vec<&TextBox>> {
    if boxes.is_empty() {
        return Vec::new();
    }

    let mut sorted_indices: Vec<usize> = (0..boxes.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        boxes[a].center_x.partial_cmp(&boxes[b].center_x).unwrap()
    });

    let mut columns: Vec<Vec<&TextBox>> = Vec::new();

    for &idx in &sorted_indices {
        let b = &boxes[idx];
        let mut found = false;
        for col in columns.iter_mut() {
            let avg_x: f64 = col.iter().map(|cb| cb.center_x).sum::<f64>() / col.len() as f64;
            if (b.center_x - avg_x).abs() < tolerance_px {
                col.push(b);
                found = true;
                break;
            }
        }
        if !found {
            columns.push(vec![b]);
        }
    }

    // Mỗi cột: sắp theo Y↓
    for col in columns.iter_mut() {
        col.sort_by(|a, b| a.center_y.partial_cmp(&b.center_y).unwrap());
    }

    // Các cột: sắp theo X→
    columns.sort_by(|a, b| {
        let ax: f64 = a.iter().map(|cb| cb.center_x).sum::<f64>() / a.len() as f64;
        let bx: f64 = b.iter().map(|cb| cb.center_x).sum::<f64>() / b.len() as f64;
        ax.partial_cmp(&bx).unwrap()
    });

    columns
}

/// So sánh 2 caption text
fn is_same_caption(s1: &str, s2: &str) -> bool {
    if s1 == s2 { return true; }
    if s1.is_empty() || s2.is_empty() { return false; }

    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let max_len = len1.max(len2);
    let min_len = len1.min(len2);

    if min_len <= 2 { return s1 == s2; }

    let dist = levenshtein_distance(s1, s2);
    if min_len <= 4 {
        dist <= 1
    } else {
        let sim = 1.0 - (dist as f32 / max_len as f32);
        sim >= 0.70 && dist <= 2
    }
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

/// Majority voting: chọn text nhiều nhất, ưu tiên dài hơn
fn select_best_caption(texts: &[String]) -> String {
    if texts.is_empty() { return String::new(); }
    let mut counts: HashMap<String, usize> = HashMap::new();
    for t in texts {
        if !t.is_empty() {
            *counts.entry(t.clone()).or_insert(0) += 1;
        }
    }
    if counts.is_empty() { return String::new(); }

    counts
        .into_iter()
        .max_by(|(t1, c1), (t2, c2)| {
            c1.cmp(c2).then_with(|| t1.chars().count().cmp(&t2.chars().count()))
        })
        .map(|(t, _)| t)
        .unwrap_or_default()
}

/// ========================================================================
/// HÀM CHÍNH: Trích xuất caption từ video
/// ========================================================================
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
        "   {} Caption ROI: Y[{:.0}%-{:.0}%] | X[{:.0}%-{:.0}%] | Fps: {}",
        "[CAPTION]".name_utils().violed(),
        config.roi.y_min * 100.0, config.roi.y_max * 100.0,
        config.roi.x_min * 100.0, config.roi.x_max * 100.0,
        config.fps,
    ));

    // Nạp OCR engine (detection + recognition trong 1 lần gọi)
    let det_path = model_dir.join("ocr/det.onnx");
    let rec_path = model_dir.join("ocr/rec.onnx");
    let keys_path = model_dir.join("ocr/keys.txt");

    crate::shared::tui_events::emit_log(format!(
        "   {} Khởi tạo Caption OCR Engine (DBNet + CTC)...",
        "[CAPTION]".name_utils().violed()
    ));
    let ocr_engine = crate::ocr::det_filter::create_det_engine(
        &det_path.to_string_lossy(),
        &rec_path.to_string_lossy(),
        &keys_path.to_string_lossy(),
    )?;

    // Video dimensions
    let probe_out = Command::new("ffprobe")
        .args(&[
            "-v", "error", "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0",
            &video_path.to_string_lossy(),
        ])
        .output()
        .context("Không thể thực thi ffprobe")?;

    let dim_str = String::from_utf8_lossy(&probe_out.stdout);
    let dims: Vec<usize> = dim_str.trim().split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect();
    let (src_w, src_h) = if dims.len() == 2 { (dims[0], dims[1]) } else { (1920, 1080) };

    // Duration
    let dur_out = Command::new("ffprobe")
        .args(&[
            "-v", "error", "-select_streams", "v:0",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            &video_path.to_string_lossy(),
        ])
        .output();
    let total_duration: f64 = dur_out
        .ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
        .unwrap_or(600.0);

    // ROI crop
    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (((src_w as f32 * config.roi.x_min).round() as usize) / 2) * 2;
    let crop_y = (((src_h as f32 * config.roi.y_min).round() as usize) / 2) * 2;

    let vf_filter = format!("fps={},crop={}:{}:{}:{}", config.fps, crop_w, crop_h, crop_x, crop_y);
    crate::shared::tui_events::emit_log(format!(
        "   {} Caption ROI: {}x{} @ ({},{}) | {}",
        "[CAPTION]".name_utils().violed(), crop_w, crop_h, crop_x, crop_y, vf_filter
    ));

    let column_x_tol = crop_w as f64 * config.column_x_tolerance as f64;

    // FFmpeg pipe
    let mut ffmpeg = Command::new("ffmpeg")
        .args(&[
            "-i", &video_path.to_string_lossy(),
            "-vf", &vf_filter,
            "-f", "rawvideo", "-pix_fmt", "rgb24",
            "-v", "error", "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("Không thể khởi động FFmpeg cho caption")?;

    let mut stdout = ffmpeg.stdout.take().context("FFmpeg stdout")?;

    let frame_bytes = crop_w * crop_h * 3;
    let mut buf = vec![0u8; frame_bytes];
    let dt = 1.0 / (config.fps as f64);
    let mut frame_idx = 0usize;

    let mut records: Vec<(usize, f64, String)> = Vec::new();
    let est_frames = (total_duration * config.fps as f64) as usize;

    crate::shared::tui_events::emit_log(format!(
        "   {} Quét caption (ước tính {} frames @ {}fps)...",
        "[CAPTION]".name_utils().violed(), est_frames, config.fps
    ));

    while stdout.read_exact(&mut buf).is_ok() {
        let ts = frame_idx as f64 * dt;
        frame_idx += 1;

        // Detect + recognize tất cả text trong ROI
        let text_boxes = detect_and_recognize(
            &ocr_engine, &buf, crop_w, crop_h,
            config.vertical_aspect_threshold,
            config.min_confidence,
        )?;

        let caption_text = if text_boxes.is_empty() {
            String::new()
        } else {
            // Tách boxes dọc vs ngang
            let vert: Vec<&TextBox> = text_boxes.iter().filter(|b| b.is_vertical).collect();
            let horiz: Vec<&TextBox> = text_boxes.iter().filter(|b| !b.is_vertical).collect();

            let mut parts: Vec<String> = Vec::new();

            // === CHỮ DỌC: nhóm cột, ghép ký tự ===
            if !vert.is_empty() {
                let vert_owned: Vec<TextBox> = vert.iter().map(|b| (*b).clone()).collect();
                let columns = group_vertical_columns(&vert_owned, column_x_tol);

                for col in &columns {
                    // Mỗi box trong cột đã có text → ghép lại (trên→dưới)
                    let col_text: String = col.iter()
                        .map(|b| b.text.as_str())
                        .collect::<Vec<_>>()
                        .join("");

                    if !col_text.is_empty() {
                        parts.push(col_text);
                    }
                }
            }

            // === CHỮ NGANG: sắp xếp theo Y rồi X, lấy text ===
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

            // Ghép parts: nếu nhiều cột ngắn (tên 2 phần) → dùng "-"
            if parts.len() > 1 {
                let all_short = parts.iter().all(|p| p.chars().count() <= 5);
                if all_short {
                    parts.join("-")
                } else {
                    parts.join("")
                }
            } else {
                parts.into_iter().next().unwrap_or_default()
            }
        };

        records.push((frame_idx - 1, ts, caption_text));

        // Progress
        if frame_idx % (config.fps as usize * 5) == 0 {
            let pct = (ts / total_duration * 100.0).min(100.0);
            crate::shared::tui_events::emit_task_progress(
                "Caption Extraction",
                frame_idx, est_frames, "frames",
                format!("{:.1}s", ts), format!("{:.1}%", pct),
            );
        }
    }

    let _ = ffmpeg.wait();
    let total = records.len();
    crate::shared::tui_events::emit_log(format!(
        "   {} Quét xong {} frames. Khử nhiễu...",
        "[CAPTION]".name_utils().violed(), total
    ));

    // === TEMPORAL DENOISING ===
    let mut sm: Vec<String> = records.iter().map(|(_, _, t)| t.clone()).collect();

    // 1-frame dropout fill: A, "", A → A, A, A
    for i in 1..total.saturating_sub(1) {
        if sm[i].is_empty() && !sm[i - 1].is_empty() && !sm[i + 1].is_empty() {
            if is_same_caption(&sm[i - 1], &sm[i + 1]) {
                sm[i] = sm[i - 1].clone();
            }
        }
    }

    // 2-frame dropout fill: A, "", "", A → A, A, A, A
    for i in 1..total.saturating_sub(2) {
        if sm[i].is_empty() && sm[i + 1].is_empty()
            && !sm[i - 1].is_empty() && !sm[i + 2].is_empty()
        {
            if is_same_caption(&sm[i - 1], &sm[i + 2]) {
                sm[i] = sm[i - 1].clone();
                sm[i + 1] = sm[i - 1].clone();
            }
        }
    }

    // 1-frame spike removal: "", A, "" → "", "", ""
    for i in 1..total.saturating_sub(1) {
        if !sm[i].is_empty() && sm[i - 1].is_empty() && sm[i + 1].is_empty() {
            sm[i] = String::new();
        }
    }

    // 1-frame typo smooth: A, B, A → A, A, A
    for i in 1..total.saturating_sub(1) {
        if !sm[i - 1].is_empty() && !sm[i + 1].is_empty() && !sm[i].is_empty()
            && sm[i] != sm[i - 1] && sm[i - 1] == sm[i + 1]
        {
            if is_same_caption(&sm[i - 1], &sm[i]) {
                sm[i] = sm[i - 1].clone();
            }
        }
    }

    // === PHÂN CỤM TEMPORAL ===
    struct RawSeg {
        start: usize,
        end: usize,
        texts: Vec<String>,
    }

    let mut raw_segs: Vec<RawSeg> = Vec::new();
    let mut cur_seg: Option<RawSeg> = None;

    for i in 0..total {
        let t = &sm[i];
        if t.is_empty() {
            if let Some(seg) = cur_seg.take() {
                raw_segs.push(seg);
            }
        } else {
            match cur_seg.as_mut() {
                Some(seg) => {
                    let dom = select_best_caption(&seg.texts);
                    if is_same_caption(&dom, t) {
                        seg.end = i;
                        seg.texts.push(t.clone());
                    } else {
                        let old = cur_seg.replace(RawSeg {
                            start: i, end: i, texts: vec![t.clone()],
                        }).unwrap();
                        raw_segs.push(old);
                    }
                }
                None => {
                    cur_seg = Some(RawSeg {
                        start: i, end: i, texts: vec![t.clone()],
                    });
                }
            }
        }
    }
    if let Some(seg) = cur_seg.take() {
        raw_segs.push(seg);
    }

    // === TẠO CAPTION SEGMENTS ===
    let mut segments: Vec<CaptionSegment> = Vec::new();

    for seg in raw_segs {
        // Tối thiểu 2 frames (ở 5fps = 400ms)
        if seg.end < seg.start + 1 {
            continue;
        }

        let best = select_best_caption(&seg.texts);
        if best.is_empty() {
            continue;
        }

        let t_start = seg.start as f64 * dt;
        let t_end = seg.end as f64 * dt;
        let start_sec = t_start.max(0.0);
        let end_sec = t_end + dt * 0.5;

        // Không overlap
        let start_sec = if let Some(last) = segments.last() {
            start_sec.max(last.end_sec + 0.001)
        } else {
            start_sec
        };

        if end_sec > start_sec + 0.1 {
            segments.push(CaptionSegment {
                start_sec,
                end_sec,
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
                if seg.text.chars().count() > last.text.chars().count() {
                    last.text = seg.text;
                }
                continue;
            }
        }
        final_segs.push(seg);
    }

    // Ghi WebVTT
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut vtt = String::from("WEBVTT\n\n");
    for seg in &final_segs {
        let s = CaptionSegment::format_vtt_timestamp(seg.start_sec);
        let e = CaptionSegment::format_vtt_timestamp(seg.end_sec);
        vtt.push_str(&format!("{} --> {} line:20%\n{}\n\n", s, e, seg.text));
    }

    fs::write(output_vtt_path, vtt)
        .with_context(|| format!("Không thể ghi file caption: {:?}", output_vtt_path))?;

    crate::shared::tui_events::emit_log(format!(
        "   {} {} Hoàn tất: {:?} ({} caption)",
        "[CAPTION]".name_utils().violed(), Icons::SUCCESS,
        output_vtt_path, final_segs.len()
    ));

    Ok(CaptionExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments: final_segs,
    })
}
