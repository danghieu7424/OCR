// src/OCR/det_filter.rs
use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops::FilterType};
use pure_onnx_ocr::{OcrEngine, OcrEngineBuilder};

/// Khởi tạo engine DBNet từ mô hình det.onnx
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
        .context("Không thể khởi tạo DBNet Engine. Hãy kiểm tra lại model.")?;
    Ok(engine)
}


/// Tìm vùng chứa phụ đề bằng DBNet và cắt ảnh patch để đưa vào mô hình nhận dạng
pub fn extract_patch_with_det(
    engine: &OcrEngine,
    rgb_bytes: &[u8],
    width: usize,
    height: usize,
) -> Result<Option<(Vec<u8>, usize, usize)>> {
    // 1. Tạo DynamicImage trực tiếp từ byte RGB (không cần qua OpenCV Mat)
    let img_buf = RgbImage::from_raw(width as u32, height as u32, rgb_bytes.to_vec())
        .ok_or_else(|| anyhow::anyhow!("Không thể tạo RgbImage từ byte RGB"))?;
    let dynamic_img = DynamicImage::ImageRgb8(img_buf);

    // 2. Chạy pipeline DBNet (detection + recognition)
    let results = engine.run_from_image(&dynamic_img)?;

    if results.is_empty() {
        return Ok(None);
    }

    // 3. Tìm bounding box có diện tích lớn nhất (dòng phụ đề chính)
    let best_result = results
        .iter()
        .max_by(|a, b| {
            let area_a = polygon_area(&a.bounding_box);
            let area_b = polygon_area(&b.bounding_box);
            area_a.partial_cmp(&area_b).unwrap_or(std::cmp::Ordering::Equal)
        });

    if let Some(result) = best_result {
        let points: Vec<(f64, f64)> = result.bounding_box.exterior().points()
            .map(|p| (p.x(), p.y()))
            .collect();

        if points.is_empty() {
            return Ok(None);
        }

                // Lấy hình chữ nhật bao quanh đa giác (axis-aligned bounding box)
        let min_x = points.iter().map(|p| p.0).fold(f64::MAX, f64::min);
        let min_y = points.iter().map(|p| p.1).fold(f64::MAX, f64::min);
        let max_x = points.iter().map(|p| p.0).fold(f64::MIN, f64::max);
        let max_y = points.iter().map(|p| p.1).fold(f64::MIN, f64::max);

        // ✅ THÊM PADDING (Mở rộng hộp ra 4px mỗi phía để không cắt xén nét chữ)
        let pad_x = 6.0;
        let pad_y = 4.0;

        let crop_x = (min_x - pad_x).max(0.0) as u32;
        let crop_y = (min_y - pad_y).max(0.0) as u32;
        let crop_max_x = (max_x + pad_x).min(width as f64);
        let crop_max_y = (max_y + pad_y).min(height as f64);

        let w = (crop_max_x - crop_x as f64) as u32;
        let h = (crop_max_y - crop_y as f64) as u32;

        if w < 10 || h < 5 {
            return Ok(None);
        }

        // 4. Cắt ảnh (Crop) bằng crate `image` với tọa độ đã có padding
        let roi = dynamic_img.crop_imm(crop_x, crop_y, w, h);

        // 5. Resize chiều cao về 48px chuẩn của PP-OCRv5 server
        let target_h = 48u32;
        let aspect = w as f32 / h as f32;
        let mut target_w = (target_h as f32 * aspect).round() as u32;
        target_w = target_w.clamp(64, 960);
        target_w = ((target_w + 7) / 8) * 8;

        let resized = roi.resize_exact(target_w, target_h, FilterType::Triangle);

        // 6. Chuyển về mảng byte RGB24
        let patch_bytes = resized.to_rgb8().into_raw();

        return Ok(Some((patch_bytes, target_w as usize, target_h as usize)));
    }

    Ok(None)
}

/// Hàm phụ trợ tính diện tích đa giác (Shoelace formula) để chọn bounding box lớn nhất
fn polygon_area(poly: &geo_types::Polygon<f64>) -> f64 {
    let points: Vec<(f64, f64)> = poly.exterior().points().map(|p| (p.x(), p.y())).collect();
    if points.len() < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        area += points[i].0 * points[j].1;
        area -= points[j].0 * points[i].1;
    }
    area.abs() / 2.0
}