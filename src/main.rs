// src/main.rs
mod OCR;

use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    println!(">>> [TEST] Khởi chạy kiểm tra module OCR phụ đề cứng...");

    // Đường dẫn video đầu vào trong thư mục video_input
    let input_dir = PathBuf::from("video_input");
    let video_path = input_dir.read_dir()?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "mov"))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy file video nào trong thư mục 'video_input/'!"))?;

    println!("   {} Đã tìm thấy video test: {:?}", "[INFO]", video_path);

    let output_vtt_path = PathBuf::from("output_subtitles.vtt");
    let model_dir = PathBuf::from("models"); // Thư mục chứa models/ocr/rec.onnx và keys.txt
    let config = OcrExtractorConfig::default();

    // Gọi hàm trích xuất phụ đề giữ nguyên nguyên bản API
    let result = extract_hardcoded_subtitles(
        &video_path,
        &output_vtt_path,
        &model_dir,
        &config,
    )?;

    println!("{} Trích xuất thành công! Tổng số câu sub: {}", "[SUCCESS]", result.segments.len());
    println!("{} File VTT đã lưu tại: {:?}", "[SUCCESS]", result.vtt_path);

    Ok(())
}