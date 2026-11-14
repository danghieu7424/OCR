// src/OCR/det_filter.rs
use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops::FilterType};
use pure_onnx_ocr::{OcrEngine, OcrEngineBuilder};

pub fn create_det_engine(
    det_model_path: &str,
    rec_model_path: &str,
    dict_path: &str,
) -> Result<OcrEngine> {
    create_det_engine_with_limit(det_model_path, rec_model_path, dict_path, 960)
}

/// Như `create_det_engine` nhưng chỉnh cạnh dài tối đa của ảnh đưa vào DBNet.
/// Ảnh càng nhỏ thì detect (và cả rec nội bộ của engine) càng nhanh.
pub fn create_det_engine_with_limit(
    det_model_path: &str,
    rec_model_path: &str,
    dict_path: &str,
    limit_side_len: u32,
) -> Result<OcrEngine> {
    let engine = OcrEngineBuilder::new()
        .det_model_path(det_model_path)
        .rec_model_path(rec_model_path)
        .dictionary_path(dict_path)
        .det_limit_side_len(limit_side_len)
        .build()
        .context("Không thể khởi tạo DBNet Engine.")?;
    Ok(engine)
}

pub fn extract_patch_with_det(
    engine: &OcrEngine,
    rgb_bytes: &[u8],
    width: usize,
    height: usize,
) -> Result<Option<(Vec<u8>, usize, usize)>> {
    let img_buf = RgbImage::from_raw(width as u32, height as u32, rgb_bytes.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Không thể tạo RgbImage"))?;
    let dynamic_img = DynamicImage::ImageRgb8(img_buf);

    let results = engine.run_from_image(&dynamic_img)?;

    if results.is_empty() {
        return Ok(None);
    }

    // Bước 1: Thu thập tất cả các box có kích thước hợp lệ
    let mut valid_boxes = Vec::new();
    for result in &results {
        let points: Vec<(f64, f64)> = result
            .bounding_box
            .exterior()
            .points()
            .map(|p| (p.x(), p.y()))
            .collect();

        if points.is_empty() {
            continue;
        }

        let b_min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let b_min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let b_max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let b_max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);

        let w = b_max_x - b_min_x;
        let h = b_max_y - b_min_y;

        // Lọc bỏ các box rác cực nhỏ (cho phép nét thanh như chữ 一 hoặc dấu chấm)
        if w < 6.0 || h < 4.0 {
            continue;
        }

        let center_y = (b_min_y + b_max_y) / 2.0;
        valid_boxes.push((b_min_x, b_min_y, b_max_x, b_max_y, center_y, h, w * h));
    }

    if valid_boxes.is_empty() {
        return Ok(None);
    }

    // Bước 2: Tìm box có diện tích lớn nhất làm mốc (Anchor)
    let anchor = valid_boxes
        .iter()
        .max_by(|a, b| a.6.partial_cmp(&b.6).unwrap())
        .unwrap();
    let anchor_cy = anchor.4;
    let anchor_h = anchor.5;

    // Bước 3: Gom tất cả các box nằm trên cùng dòng với Anchor (chênh lệch tâm Y < 65% chiều cao chữ)
    let y_threshold = (anchor_h * 0.65).max(20.0);
    let mut group_min_x = f64::MAX;
    let mut group_min_y = f64::MAX;
    let mut group_max_x = f64::MIN;
    let mut group_max_y = f64::MIN;

    for &(b_min_x, b_min_y, b_max_x, b_max_y, cy, _, _) in &valid_boxes {
        if (cy - anchor_cy).abs() < y_threshold {
            group_min_x = group_min_x.min(b_min_x);
            group_min_y = group_min_y.min(b_min_y);
            group_max_x = group_max_x.max(b_max_x);
            group_max_y = group_max_y.max(b_max_y);
        }
    }

    // Bước 4: Cắt ảnh với Padding an toàn để không xén nét chữ đầu/cuối (như chữ 一)
    let pad_x = 16.0;
    let pad_y = 8.0;

    let crop_x = (group_min_x - pad_x).max(0.0) as u32;
    let crop_y = (group_min_y - pad_y).max(0.0) as u32;
    let crop_max_x = (group_max_x + pad_x).min(width as f64);
    let crop_max_y = (group_max_y + pad_y).min(height as f64);

    let w = (crop_max_x - crop_x as f64) as u32;
    let h = (crop_max_y - crop_y as f64) as u32;

    if w < 8 || h < 5 {
        return Ok(None);
    }

    let roi = dynamic_img.crop_imm(crop_x, crop_y, w, h);

    let target_h = 48u32;
    let aspect = w as f32 / h as f32;
    let mut target_w = (target_h as f32 * aspect).round() as u32;
    // Giới hạn width từ 48 (cho chữ đơn vuông) đến 1280
    target_w = target_w.clamp(48, 1280);
    target_w = ((target_w + 7) / 8) * 8;

    // Lanczos3 giữ độ sắc nét các nét chữ Hán tốt hơn Triangle
    let resized = roi.resize_exact(target_w, target_h, FilterType::Lanczos3);
    let patch_bytes = resized.to_rgb8().into_raw();

    Ok(Some((patch_bytes, target_w as usize, target_h as usize)))
}
