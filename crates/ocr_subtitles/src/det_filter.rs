// src/OCR/det_filter.rs
use anyhow::{Context, Result};
use image::{DynamicImage, RgbImage, imageops::FilterType};
use pure_onnx_ocr::{OcrEngine, OcrEngineBuilder};

#[derive(Debug, Clone)]
pub struct DetectedPatch {
    pub bytes: Vec<u8>,
    pub width: usize,
    pub height: usize,
    pub confidence: f32,
    pub bounds: (f64, f64, f64, f64),
}

#[derive(Debug, Clone)]
struct TextBoxCandidate {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    center_y: f64,
    height: f64,
    area: f64,
    confidence: f32,
    text: String,
}

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
) -> Result<Option<DetectedPatch>> {
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
        valid_boxes.push(TextBoxCandidate {
            min_x: b_min_x,
            min_y: b_min_y,
            max_x: b_max_x,
            max_y: b_max_y,
            center_y,
            height: h,
            area: w * h,
            confidence: result.confidence,
            text: result.text.clone(),
        });
    }

    if valid_boxes.is_empty() {
        return Ok(None);
    }

    // Bước 2: Tìm dòng có box lớn nhất làm mốc, rồi giữ cụm chữ liền mạch
    // gần tâm khung hình thay vì gộp mọi chữ cùng cao độ trong cảnh.
    let anchor = valid_boxes
        .iter()
        .max_by(|a, b| a.area.total_cmp(&b.area))
        .unwrap();
    let anchor_cy = anchor.center_y;
    let anchor_h = anchor.height;

    // Bước 3: Gom box trên cùng dòng thành các cụm ngang riêng.
    let y_threshold = (anchor_h * 0.65).max(20.0);
    let line_boxes: Vec<&TextBoxCandidate> = valid_boxes
        .iter()
        .filter(|candidate| (candidate.center_y - anchor_cy).abs() < y_threshold)
        .collect();
    let group = select_centered_text_cluster(&line_boxes, width as f64, anchor_h);
    if std::env::var_os("OCR_SUB_DEBUG").is_some() {
        for candidate in &line_boxes {
            eprintln!(
                "[DETBOX] ({:.0},{:.0})-({:.0},{:.0}) conf={:.2} text={:?}",
                candidate.min_x,
                candidate.min_y,
                candidate.max_x,
                candidate.max_y,
                candidate.confidence,
                candidate.text,
            );
        }
        eprintln!(
            "[DETGROUP] selected {} boxes near x={:.0}",
            group.len(),
            group
                .iter()
                .map(|candidate| (candidate.min_x + candidate.max_x) * 0.5)
                .sum::<f64>()
                / group.len() as f64
        );
    }

    let group_min_x = group
        .iter()
        .map(|candidate| candidate.min_x)
        .fold(f64::MAX, f64::min);
    let group_min_y = group
        .iter()
        .map(|candidate| candidate.min_y)
        .fold(f64::MAX, f64::min);
    let group_max_x = group
        .iter()
        .map(|candidate| candidate.max_x)
        .fold(f64::MIN, f64::max);
    let group_max_y = group
        .iter()
        .map(|candidate| candidate.max_y)
        .fold(f64::MIN, f64::max);
    let group_confidence_weight = group
        .iter()
        .map(|candidate| candidate.confidence * candidate.area as f32)
        .sum::<f32>();
    let group_area = group.iter().map(|candidate| candidate.area).sum::<f64>();

    // Scale padding with glyph height to avoid surrounding scene pixels dominating
    // single-character recognition while still protecting line-edge strokes.
    let (pad_x, pad_y) = crop_padding(anchor_h);

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
    target_w = target_w.div_ceil(8) * 8;

    // Lanczos3 giữ độ sắc nét các nét chữ Hán tốt hơn Triangle
    let resized = roi.resize_exact(target_w, target_h, FilterType::Lanczos3);
    let patch_bytes = resized.to_rgb8().into_raw();

    Ok(Some(DetectedPatch {
        bytes: patch_bytes,
        width: target_w as usize,
        height: target_h as usize,
        confidence: group_confidence_weight / group_area as f32,
        bounds: (group_min_x, group_min_y, group_max_x, group_max_y),
    }))
}

fn crop_padding(text_height: f64) -> (f64, f64) {
    (
        (text_height * 0.40).clamp(4.0, 32.0),
        (text_height * 0.08).clamp(2.0, 8.0),
    )
}

fn select_centered_text_cluster<'a>(
    line_boxes: &[&'a TextBoxCandidate],
    image_width: f64,
    reference_height: f64,
) -> Vec<&'a TextBoxCandidate> {
    let mut sorted = line_boxes.to_vec();
    sorted.sort_by(|a, b| a.min_x.total_cmp(&b.min_x));

    let mut clusters: Vec<Vec<&TextBoxCandidate>> = Vec::new();
    for candidate in sorted {
        let joins_previous = clusters
            .last()
            .and_then(|cluster| cluster.last())
            .is_some_and(|previous| {
                let max_gap = (previous.height.min(candidate.height) * 0.12).max(4.0);
                candidate.min_x - previous.max_x <= max_gap
            });
        if joins_previous {
            clusters.last_mut().unwrap().push(candidate);
        } else {
            clusters.push(vec![candidate]);
        }
    }

    if clusters.len() <= 1 {
        return clusters.pop().unwrap_or_default();
    }

    clusters
        .into_iter()
        .max_by(|a, b| {
            cluster_score(a, image_width, reference_height).total_cmp(&cluster_score(
                b,
                image_width,
                reference_height,
            ))
        })
        .unwrap_or_default()
}

fn cluster_score(cluster: &[&TextBoxCandidate], image_width: f64, reference_height: f64) -> f64 {
    let min_x = cluster
        .iter()
        .map(|candidate| candidate.min_x)
        .fold(f64::MAX, f64::min);
    let max_x = cluster
        .iter()
        .map(|candidate| candidate.max_x)
        .fold(f64::MIN, f64::max);
    let center = (min_x + max_x) * 0.5;
    let centeredness =
        (1.0 - ((center - image_width * 0.5).abs() / (image_width * 0.5))).clamp(0.0, 1.0);
    let confidence = cluster
        .iter()
        .map(|candidate| candidate.confidence)
        .sum::<f32>()
        / cluster.len() as f32;
    let relative_width = ((max_x - min_x) / reference_height).max(1.0);

    confidence as f64 * centeredness.powi(2) * relative_width.sqrt()
}

#[cfg(test)]
mod tests {
    use super::{TextBoxCandidate, cluster_score, crop_padding, select_centered_text_cluster};

    #[test]
    fn crop_padding_scales_with_text_and_respects_limits() {
        assert_eq!(crop_padding(35.0), (14.0, 2.8000000000000003));
        assert_eq!(crop_padding(200.0), (32.0, 8.0));
    }

    #[test]
    fn line_selection_prefers_a_coherent_centered_subtitle_cluster() {
        let scene = TextBoxCandidate {
            min_x: 120.0,
            min_y: 20.0,
            max_x: 250.0,
            max_y: 70.0,
            center_y: 45.0,
            height: 50.0,
            area: 6500.0,
            confidence: 0.95,
            text: "scene".to_string(),
        };
        let subtitle = TextBoxCandidate {
            min_x: 560.0,
            min_y: 20.0,
            max_x: 650.0,
            max_y: 70.0,
            center_y: 45.0,
            height: 50.0,
            area: 4500.0,
            confidence: 0.90,
            text: "dialogue".to_string(),
        };
        let boxes = [&scene, &subtitle];

        assert_eq!(select_centered_text_cluster(&boxes, 1280.0, 50.0).len(), 1);
        assert!(cluster_score(&[&subtitle], 1280.0, 50.0) > cluster_score(&[&scene], 1280.0, 50.0));
    }
}
