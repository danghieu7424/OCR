// src/OCR/det_filter.rs
use anyhow::{Context, Result};
use opencv::{
    core::{self, Mat, Rect, Size},
    imgproc,
    prelude::*,
};
use pure_onnx_ocr::{OcrEngine, OcrEngineBuilder};

/// Khởi tạo engine DBNet từ mô hình det.onnx
pub fn create_det_engine(det_model_path: &str) -> Result<OcrEngine> {
    let engine = OcrEngineBuilder::new()
        .det_model_path(det_model_path)
        .build()
        .context("Không thể khởi tạo DBNet Engine từ det.onnx. Hãy kiểm tra lại model.")?;
    Ok(engine)
}

/// Tìm vùng chứa phụ đề bằng DBNet và cắt ảnh patch để đưa vào mô hình nhận dạng
pub fn extract_patch_with_det(
    engine: &OcrEngine,
    rgb_bytes: &[u8],
    width: usize,
    height: usize,
) -> Result<Option<(Vec<u8>, usize, usize)>> {
    // 1. Tạo ảnh OpenCV Mat từ byte RGB
    let raw_mat = Mat::from_slice(rgb_bytes)?;
    let mat = raw_mat.reshape(3, height as i32)?;

    // 2. Chạy mô hình DBNet để lấy danh sách các vùng văn bản (bounding boxes)
    let result = engine.detect(&mat)?;
    
    if result.regions.is_empty() {
        return Ok(None);
    }

    // 3. Lọc vùng có chiều ngang lớn nhất (thường là dòng phụ đề trung tâm)
    let best_box = result.regions
        .iter()
        .max_by(|a, b| {
            let area_a = a.bounding_box.width * a.bounding_box.height;
            let area_b = b.bounding_box.width * b.bounding_box.height;
            area_a.partial_cmp(&area_b).unwrap()
        });

    if let Some(region) = best_box {
        // Đảm bảo box không vượt quá biên ảnh
        let x = region.bounding_box.x.max(0.0) as i32;
        let y = region.bounding_box.y.max(0.0) as i32;
        let w = region.bounding_box.width.min(width as f32 - x as f32) as i32;
        let h = region.bounding_box.height.min(height as f32 - y as f32) as i32;

        if w < 10 || h < 5 {
            return Ok(None);
        }

        let rect = Rect::new(x, y, w, h);

        // 4. Cắt vùng ảnh phụ đề
        let roi = mat.roi(rect)?;
        
        // 5. Resize chiều cao về 48px chuẩn của PP-OCRv5 server
        let target_h = 48usize;
        let aspect = w as f32 / h as f32;
        let mut target_w = (target_h as f32 * aspect).round() as usize;
        target_w = target_w.clamp(64, 960);
        target_w = ((target_w + 7) / 8) * 8;

        let mut resized = Mat::default();
        imgproc::resize(
            &roi,
            &mut resized,
            Size::new(target_w as i32, target_h as i32),
            0.0,
            0.0,
            imgproc::INTER_LINEAR,
        )?;

        // 6. Chuyển đổi sang mảng byte (RGB24)
        let patch_bytes = resized.data_bytes()?.to_vec();

        return Ok(Some((patch_bytes, target_w, target_h)));
    }

    Ok(None)
}