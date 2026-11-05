// src/OCR/det_filter.rs
use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops::FilterType};
use pure_onnx_ocr::{OcrEngine, OcrEngineBuilder};

pub fn create_det_engine(
    det_model_path: &str,
    rec_model_path: &str,
    dict_path: &str,
) -> Result<OcrEngine> {
    let engine = OcrEngineBuilder::new()
        .det_model_path(det_model_path)
        .rec_model_path(rec_model_path)
        .dictionary_path(dict_path)
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

    // ✅ Vì ROI đã được crop từ mod.rs, toàn bộ ảnh này là vùng chứa sub.
    // Ta gom TẤT CẢ các box có kích thước hợp lệ lại thành 1 hộp lớn.
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    let mut has_box = false;

    for result in &results {
        let points: Vec<(f64, f64)> = result.bounding_box.exterior().points()
            .map(|p| (p.x(), p.y()))
            .collect();

        if points.is_empty() { continue; }

        let b_min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let b_min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let b_max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let b_max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);

        let w = b_max_x - b_min_x;
        let h = b_max_y - b_min_y;

        // Chỉ lọc các box quá nhỏ (rác)
        if w < 15.0 || h < 5.0 {
            continue;
        }

        has_box = true;
        min_x = min_x.min(b_min_x);
        min_y = min_y.min(b_min_y);
        max_x = max_x.max(b_max_x);
        max_y = max_y.max(b_max_y);
    }

    if !has_box {
        return Ok(None);
    }

    let pad_x = 10.0;
    let pad_y = 6.0;

    let crop_x = (min_x - pad_x).max(0.0) as u32;
    let crop_y = (min_y - pad_y).max(0.0) as u32;
    let crop_max_x = (max_x + pad_x).min(width as f64);
    let crop_max_y = (max_y + pad_y).min(height as f64);

    let w = (crop_max_x - crop_x as f64) as u32;
    let h = (crop_max_y - crop_y as f64) as u32;

    if w < 10 || h < 5 {
        return Ok(None);
    }

    let roi = dynamic_img.crop_imm(crop_x, crop_y, w, h);

    let target_h = 48u32;
    let aspect = w as f32 / h as f32;
    let mut target_w = (target_h as f32 * aspect).round() as u32;
    target_w = target_w.clamp(64, 960);
    target_w = ((target_w + 7) / 8) * 8;

    let resized = roi.resize_exact(target_w, target_h, FilterType::Triangle);
    let patch_bytes = resized.to_rgb8().into_raw();

    return Ok(Some((patch_bytes, target_w as usize, target_h as usize)));
}