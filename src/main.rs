// src/main.rs
mod OCR;

use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig};
use std::path::PathBuf;

// Giả lập module shared để dự án độc lập không bị thiếu dependency từ tool chính
pub mod shared {
    pub struct CpuOptimizer;
    impl CpuOptimizer {
        pub fn single_task_threads() -> usize {
            2
        }
    }

    pub mod terminal_color {
        pub trait TermColorExt {
            fn violed(&self) -> String;
        }
        impl TermColorExt for &str {
            fn violed(&self) -> String {
                (*self).to_string()
            }
        }
        impl TermColorExt for String {
            fn violed(&self) -> String {
                self.clone()
            }
        }
    }

    pub mod tui_events {
        pub fn emit_step(_msg: &str) {}
        pub fn emit_log(_msg: impl Into<String>) {}
    }
}

fn main() -> anyhow::Result<()> {
    println!(">>> [TEST] Khởi chạy kiểm tra module OCR phụ đề cứng...");

    // 1. Kiểm tra thư mục video_input và tìm file video
    let input_dir = PathBuf::from("video_input");
    if !input_dir.exists() {
        std::fs::create_dir_all(&input_dir)?;
        println!("   {} Đã tạo thư mục 'video_input/'. Vui lòng bỏ file video vào đây!", "[INFO]");
        return Ok(());
    }

    let video_path = input_dir.read_dir()?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "mov"))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy file video nào trong thư mục 'video_input/'! Hãy chép 1 video vào đó."))?;

    println!("   {} Đã tìm thấy video test: {:?}", "[INFO]", video_path);

    let output_vtt_path = PathBuf::from("output_subtitles.vtt");
    let model_dir = PathBuf::from("models"); // Thư mục chứa models/ocr/rec.onnx và keys.txt
    let config = OcrExtractorConfig::default();

    // 2. Chạy trích xuất phụ đề
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