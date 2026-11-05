// src/OCR/opencv_filter.rs
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

    // 1. UPSCALE 2X (Giúp nét chữ sắc nét hơn)
    let mut img_large = Mat::default();
    imgproc::resize(
        &mat,
        &mut img_large,
        Size::new(width as i32 * 2, height as i32 * 2),
        0.0,
        0.0,
        imgproc::INTER_CUBIC,
    )?;

    // 2. TÁCH KÊNH MÀU VÀ LỌC LẤY NÉT CHỮ TRẮNG (loại bỏ nhiễu nền đỏ/vàng)
    let mut channels = Vector::<Mat>::new();
    core::split(&img_large, &mut channels)?;

    // Lấy Max(R, G, B) để giữ nguyên độ sáng chữ
    let mut gray_max = Mat::default();
    let mut temp_max = Mat::default();
    core::max(&channels.get(0)?, &channels.get(1)?, &mut temp_max)?;
    core::max(&temp_max, &channels.get(2)?, &mut gray_max)?;

    // 3. TẠO MASK BẮT VÙNG CHỮ VÀ TÌM BOUNDING BOX
    let mut thresh_mask = Mat::default();
    // Bắt các điểm ảnh sáng (thân chữ phụ đề) với ngưỡng > 175
    imgproc::threshold(&gray_max, &mut thresh_mask, 175.0, 255.0, imgproc::THRESH_BINARY)?;

    // Dùng morphology kernel ngang để nối các chữ thành dải liền
    let kernel = imgproc::get_structuring_element(imgproc::MORPH_RECT, Size::new(25, 5), Point::new(-1, -1))?;
    let mut morphed = Mat::default();
    imgproc::morphology_ex(
        &thresh_mask,
        &mut morphed,
        imgproc::MORPH_CLOSE,
        &kernel,
        Point::new(-1, -1),
        1,
        core::BORDER_CONSTANT,
        Scalar::all(f64::MAX),
    )?;

    let mut contours = Vector::<Vector<Point>>::new();
    imgproc::find_contours(
        &morphed,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )?;

    let mut min_x = width as i32 * 2;
    let mut min_y = height as i32 * 2;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found = false;

    for cnt in contours.iter() {
        let rect = imgproc::bounding_rect(&cnt)?;
        if rect.width > 20 && rect.height > 10 && (rect.width as f64 / rect.height as f64) > 0.8 {
            found = true;
            min_x = min_x.min(rect.x);
            min_y = min_y.min(rect.y);
            max_x = max_x.max(rect.x + rect.width);
            max_y = max_y.max(rect.y + rect.height);
        }
    }

    if !found || max_x <= min_x || max_y <= min_y {
        min_x = (width as i32 * 2) / 10;
        max_x = (width as i32 * 2) * 9 / 10;
        min_y = 2;
        max_y = height as i32 * 2 - 2;
    }

    // Nới padding 10px để không bị xén chữ
    let pad_x = 10;
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

    let rect = Rect::new(min_x, min_y, box_w, box_h);

    // 4. TẠO ẢNH XỬ LÝ SẠCH NỀN ĐƯA VÀO AI
    let target_h = 48usize;
    let aspect = box_w as f32 / box_h as f32;
    let mut target_w = (target_h as f32 * aspect).round() as usize;
    target_w = target_w.clamp(48, 1280);
    target_w = ((target_w + 7) / 8) * 8;

    // Ảnh Raw Patch gốc để debug
    let raw_roi = img_large.roi(rect)?;
    let mut raw_resized = Mat::default();
    imgproc::resize(
        &raw_roi,
        &mut raw_resized,
        Size::new(target_w as i32, target_h as i32),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;
    let raw_bytes = raw_resized.data_bytes()?.to_vec();

    // Ảnh Clean Patch (Lọc sạch nền đỏ/vàng, chỉ giữ nguyên ảnh gốc nơi có chữ)
    let gray_roi = gray_max.roi(rect)?;
    let mut gray_resized = Mat::default();
    imgproc::resize(
        &gray_roi,
        &mut gray_resized,
        Size::new(target_w as i32, target_h as i32),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    // Chuyển sang RGB 3 kênh cho ONNX PP-OCR
    let mut clean_rgb = Mat::default();
    imgproc::cvt_color(
        &gray_resized,
        &mut clean_rgb,
        imgproc::COLOR_GRAY2RGB,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;
    let clahe_bytes = clean_rgb.data_bytes()?.to_vec();

    Ok(Some((raw_bytes, clahe_bytes, target_w, target_h)))
}