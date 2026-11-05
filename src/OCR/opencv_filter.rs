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

    // 1. Upscale 2x
    let mut img_large = Mat::default();
    imgproc::resize(
        &mat,
        &mut img_large,
        Size::new((width * 2) as i32, (height * 2) as i32),
        0.0,
        0.0,
        imgproc::INTER_CUBIC,
    )?;

    // 2. Grayscale + CLAHE
    let mut gray = Mat::default();
    imgproc::cvt_color(
        &img_large,
        &mut gray,
        imgproc::COLOR_RGB2GRAY,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;

    let mut enhanced = Mat::default();
    let mut clahe = imgproc::create_clahe(4.0, Size::new(8, 8))?;
    clahe.apply(&gray, &mut enhanced)?;

    // 3. HSV mask
    let mut hsv = Mat::default();
    imgproc::cvt_color(
        &img_large,
        &mut hsv,
        imgproc::COLOR_RGB2HSV,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;

    let mut hsv_ch = Vector::<Mat>::new();
    core::split(&hsv, &mut hsv_ch)?;
    let s_ch = hsv_ch.get(1)?;
    let v_ch = hsv_ch.get(2)?;

    let mut mask_s = Mat::default();
    let mut mask_v = Mat::default();
    imgproc::threshold(&s_ch, &mut mask_s, 30.0, 255.0, imgproc::THRESH_BINARY)?;
    imgproc::threshold(&v_ch, &mut mask_v, 160.0, 255.0, imgproc::THRESH_BINARY)?;

    let mut color_mask = Mat::default();
    core::bitwise_or(&mask_s, &mask_v, &mut color_mask, &core::no_array())?;

    // 4. Otsu threshold trên ảnh CLAHE
    let mut otsu_mask = Mat::default();
    imgproc::threshold(
        &enhanced,
        &mut otsu_mask,
        0.0,
        255.0,
        imgproc::THRESH_BINARY | imgproc::THRESH_OTSU,
    )?;

    // 5. Hợp nhất mask
    let mut text_mask = Mat::default();
    core::bitwise_or(&otsu_mask, &color_mask, &mut text_mask, &core::no_array())?;

    // 6. Morphology đóng lỗ
    let kernel = imgproc::get_structuring_element(
        imgproc::MORPH_RECT,
        Size::new(3, 3),
        Point::new(-1, -1),
    )?;
    let mut text_mask_closed = Mat::default();
    imgproc::morphology_ex(
        &text_mask,
        &mut text_mask_closed,
        imgproc::MORPH_CLOSE,
        &kernel,
        Point::new(-1, -1),
        2,
        core::BORDER_CONSTANT,
        Scalar::all(f64::MAX),
    )?;

    // 7. ✅ DILATE MẠNH: 31x3, 3 iterations → bridge gap lên tới ±45px mỗi phía
    // (Trước đây 25x3 × 1 iter = ±12px → không bridge được khoảng cách giữa các ký tự)
    let horizontal_kernel = imgproc::get_structuring_element(
        imgproc::MORPH_RECT,
        Size::new(31, 3),
        Point::new(-1, -1),
    )?;
    let mut text_lines = Mat::default();
    imgproc::dilate(
        &text_mask_closed,
        &mut text_lines,
        &horizontal_kernel,
        Point::new(-1, -1),
        3,   // ← 3 iterations
        core::BORDER_CONSTANT,
        Scalar::all(f64::MAX),
    )?;

    // ✅ THÊM LẠI: tìm contour trên text_lines (bị thiếu khi paste lần trước)
    let mut contours = Vector::<Vector<Point>>::new();
    imgproc::find_contours(
        &text_lines,
        &mut contours,
        imgproc::RETR_EXTERNAL,
        imgproc::CHAIN_APPROX_SIMPLE,
        Point::new(0, 0),
    )?;

    // 8. Thu thập tất cả contour pass filter
    let mut rects: Vec<Rect> = Vec::new();
    for cnt in contours.iter() {
        let rect = imgproc::bounding_rect(&cnt)?;
        if rect.width >= 15
            && rect.height >= 4
            && rect.height < 500
            && (rect.width as f64 / rect.height as f64) >= 0.3
            && (rect.width * rect.height) >= 25
        {
            rects.push(rect);
        }
    }

    if rects.is_empty() {
        return Ok(None);
    }

    // ✅ Group theo hàng dọc (cùng text line) để tách sub chính khỏi watermark góc
    rects.sort_by_key(|r| r.y + r.height / 2);

    let mut groups: Vec<Vec<Rect>> = Vec::new();
    for r in rects {
        let center_y = r.y + r.height / 2;
        let mut matched = false;
        for g in groups.iter_mut() {
            let g_center = g[0].y + g[0].height / 2;
            if (center_y - g_center).abs() < 20 {
                g.push(r);
                matched = true;
                break;
            }
        }
        if !matched {
            groups.push(vec![r]);
        }
    }

    // Chọn group có tổng chiều rộng lớn nhất (sub chính dài hơn watermark)
    let best_group = groups
        .iter()
        .max_by_key(|g| g.iter().map(|r| r.width as i64).sum::<i64>())
        .unwrap();

    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = 0i32;
    let mut max_y = 0i32;
    for r in best_group {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.x + r.width);
        max_y = max_y.max(r.y + r.height);
    }

    if max_x <= min_x || max_y <= min_y {
        return Ok(None);
    }

    // 9. Padding rộng hơn (24x14 thay vì 16x10)
    let pad_x = 12;
    let pad_y = 8;
    min_x = (min_x - pad_x).max(0);
    min_y = (min_y - pad_y).max(0);
    max_x = (max_x + pad_x).min((width * 2) as i32);
    max_y = (max_y + pad_y).min((height * 2) as i32);

    let box_w = max_x - min_x;
    let box_h = max_y - min_y;
    if box_w < 16 || box_h < 12 {
        return Ok(None);
    }

    // 10. Resize về chuẩn OCR
    let target_h = 48usize;
    let aspect = box_w as f32 / box_h as f32;
    let mut target_w = (target_h as f32 * aspect).round() as usize;
    target_w = target_w.clamp(64, 960);
    target_w = ((target_w + 7) / 8) * 8;

    let rect = Rect::new(min_x, min_y, box_w, box_h);

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

    let enhanced_roi = enhanced.roi(rect)?;
    let mut enhanced_resized = Mat::default();
    imgproc::resize(
        &enhanced_roi,
        &mut enhanced_resized,
        Size::new(target_w as i32, target_h as i32),
        0.0,
        0.0,
        imgproc::INTER_LINEAR,
    )?;

    let mut clahe_rgb = Mat::default();
    imgproc::cvt_color(
        &enhanced_resized,
        &mut clahe_rgb,
        imgproc::COLOR_GRAY2RGB,
        0,
        core::AlgorithmHint::ALGO_HINT_DEFAULT,
    )?;
    let clahe_bytes = clahe_rgb.data_bytes()?.to_vec();

    Ok(Some((raw_bytes, clahe_bytes, target_w, target_h)))
}