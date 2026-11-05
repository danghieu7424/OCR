// src/aidub/OCR/opencv_filter.rs
use anyhow::Result;
use opencv::{
    core::{self, Mat, Point, Rect, Scalar, Size, Vector},
    imgproc,
    prelude::*,
};

pub fn extract_and_filter_patch(
    rgb_bytes: &[u8],
    width: usize,
    height: usize,
) -> Result<Option<(Vec<u8>, Vec<u8>, usize, usize)>> {
    if width < 16 || height < 12 || rgb_bytes.len() < width * height * 3 {
        return Ok(None);
    }

    let raw_mat = Mat::from_slice(rgb_bytes)?;
    let mat = raw_mat.reshape(3, height as i32)?;

    // 1. UPSCALE 2X (Giúp mô hình AI nhận diện sắc nét hơn)
    let mut img_large = Mat::default();
    imgproc::resize(&mat, &mut img_large, Size::new(width as i32 * 2, height as i32 * 2), 0.0, 0.0, imgproc::INTER_CUBIC)?;

    // 2. TÁCH KÊNH MÀU VÀ TĂNG CƯỜNG ĐỘ TƯƠNG PHẢN (CLAHE)
    let mut channels = Vector::<Mat>::new();
    core::split(&img_large, &mut channels)?;
    
    let mut gray_max = Mat::default();
    let mut temp_max = Mat::default();
    core::max(&channels.get(0)?, &channels.get(1)?, &mut temp_max)?;
    core::max(&temp_max, &channels.get(2)?, &mut gray_max)?;

    let mut enhanced = Mat::default();
    let mut clahe = imgproc::create_clahe(3.0, Size::new(8, 8))?;
    clahe.apply(&gray_max, &mut enhanced)?;

    // 3. TÌM VÙNG CHỨA PHỤ ĐỀ (TEXT BOUNDING BOX) THAY THẾ CHO CÁCH CŨ
    let mut thresh = Mat::default();
    imgproc::threshold(&enhanced, &mut thresh, 160.0, 255.0, imgproc::THRESH_BINARY)?;

    // Dùng kernel ngang để gom các ký tự phụ đề thành một dải liền mạch (giống cách VSE xử lý)
    let kernel = imgproc::get_structuring_element(imgproc::MORPH_RECT, Size::new(30, 35), Point::new(-1, -1))?;
    let mut morphed = Mat::default();
    imgproc::morphology_ex(&thresh, &mut morphed, imgproc::MORPH_CLOSE, &kernel, Point::new(-1, -1), 1, core::BORDER_CONSTANT, Scalar::all(f64::MAX))?;

    let mut contours = Vector::<Vector<Point>>::new();
    imgproc::find_contours(&morphed, &mut contours, imgproc::RETR_EXTERNAL, imgproc::CHAIN_APPROX_SIMPLE, Point::new(0, 0))?;

    let mut min_x = width as i32 * 2;
    let mut min_y = height as i32 * 2;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for cnt in contours.iter() {
        let rect = imgproc::bounding_rect(&cnt)?;
        // Nới lỏng điều kiện lọc để bắt cả sub ngắn hoặc sub ở các vị trí biên
        if rect.width > 15 && rect.height > 8 && (rect.width as f64 / rect.height as f64) > 0.8 {
            found = true;
            min_x = min_x.min(rect.x);
            min_y = min_y.min(rect.y);
            max_x = max_x.max(rect.x + rect.width);
            max_y = max_y.max(rect.y + rect.height);
        }
    }

    // Nếu không tìm thấycontour chuẩn, fallback lấy toàn bộ vùng ROI trung tâm
    if !found || max_x <= min_x || max_y <= min_y {
        min_x = (width as i32 * 2) / 10;
        max_x = (width as i32 * 2) * 9 / 10;
        min_y = 2;
        max_y = height as i32 * 2 - 2;
    }

    // Nới rộng padding an toàn tránh mất viền chữ
    let pad_x = 12;
    let pad_y = 6;
    min_x = (min_x - pad_x).max(0);
    min_y = (min_y - pad_y).max(0);
    max_x = (max_x + pad_x).min((width as i32) * 2);
    max_y = (max_y + pad_y).min((height as i32) * 2);

    let box_w = max_x - min_x;
    let box_h = max_y - min_y;

    if box_w < 16 || box_h < 10 {
        return Ok(None);
    }

    // 4. CHUẨN HÓA KÍCH THƯỚC ĐẦU RA CHO MODEL (Chiều cao cố định 48px)
    let target_h = 48usize;
    let aspect = box_w as f32 / box_h as f32;
    let mut target_w = (target_h as f32 * aspect).round() as usize;
    target_w = target_w.clamp(48, 1280);
    target_w = ((target_w + 7) / 8) * 8; // Đảm bảo chia hết cho 8 cho ONNX tensor

    let rect = Rect::new(min_x, min_y, box_w, box_h);

    // --- RAW PATCH ---
    let raw_roi = img_large.roi(rect)?;
    let mut raw_resized = Mat::default();
    imgproc::resize(&raw_roi, &mut raw_resized, Size::new(target_w as i32, target_h as i32), 0.0, 0.0, imgproc::INTER_LINEAR)?;
    let raw_bytes = raw_resized.data_bytes()?.to_vec();

    // --- CLAHE PATCH ---
    let enhanced_roi = enhanced.roi(rect)?;
    let mut enhanced_resized = Mat::default();
    imgproc::resize(&enhanced_roi, &mut enhanced_resized, Size::new(target_w as i32, target_h as i32), 0.0, 0.0, imgproc::INTER_LINEAR)?;
    
    let mut clahe_rgb = Mat::default();
    imgproc::cvt_color(&enhanced_resized, &mut clahe_rgb, imgproc::COLOR_GRAY2RGB, 0, core::AlgorithmHint::ALGO_HINT_DEFAULT)?;
    let clahe_bytes = clahe_rgb.data_bytes()?.to_vec();

    Ok(Some((raw_bytes, clahe_bytes, target_w, target_h)))
}