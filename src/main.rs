// src/main.rs
mod ocr;

use ocr::{extract_hardcoded_subtitles, OcrExtractorConfig};
use std::path::PathBuf;

pub mod shared {
    pub struct CpuOptimizer;
    impl CpuOptimizer {
        pub fn single_task_threads() -> usize {
            2
        }
    }

        pub mod terminal_color {
        pub struct NameWrapper(pub String);
        impl NameWrapper {
            pub fn violed(self) -> String { self.0 }
        }
        pub trait NameUtilsExt {
            fn name_utils(&self) -> NameWrapper;
        }
        impl NameUtilsExt for str {
            fn name_utils(&self) -> NameWrapper {
                NameWrapper(self.to_string())
            }
        }
        impl NameUtilsExt for &str {
            fn name_utils(&self) -> NameWrapper {
                NameWrapper((*self).to_string())
            }
        }
        impl NameUtilsExt for String {
            fn name_utils(&self) -> NameWrapper {
                NameWrapper(self.clone())
            }
        }

        // ✅ THÊM TRAIT NÀY để đủ cho import `TermColorExt`
        pub trait TermColorExt {
            fn violed(&self) -> String;
        }
        impl TermColorExt for str {
            fn violed(&self) -> String { self.to_string() }
        }
        impl TermColorExt for &str {
            fn violed(&self) -> String { (*self).to_string() }
        }
        impl TermColorExt for String {
            fn violed(&self) -> String { self.clone() }
        }
    }

    pub mod tui_events {
        pub fn emit_step(_msg: &str) {}
        pub fn emit_log(_msg: impl Into<String>) {}
        pub fn emit_task_progress(
            _title: &str,
            _curr: usize,
            _total: usize,
            _unit: &str,
            _elapsed: impl Into<String>,
            _detail: impl Into<String>,
        ) {
        }
    }

    pub mod icons {
        pub struct Icons;
        impl Icons {
            pub const SUCCESS: &'static str = "✔ ";
        }
    }
}

fn main() -> anyhow::Result<()> {
    println!(">>> [TEST] Khởi chạy kiểm tra module OCR phụ đề cứng...");

    let input_dir = PathBuf::from("video_input");
    if !input_dir.exists() {
        std::fs::create_dir_all(&input_dir)?;
        println!("   [INFO] Đã tạo thư mục 'video_input/'. Vui lòng bỏ file video vào đây!");
        return Ok(());
    }

    let video_path = input_dir
        .read_dir()?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .find(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "mov"))
                .unwrap_or(false)
        })
        .ok_or_else(|| anyhow::anyhow!("Không tìm thấy file video nào trong 'video_input/'!"))?;

    println!("   [INFO] Video test: {:?}", video_path);

    let output_vtt_path = PathBuf::from("output_subtitles.vtt");
    let model_dir = PathBuf::from("models");
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;
    config.accuracy_mode = true;
    println!(
        "   [INFO] Mode: {} | FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%]",
        if config.accuracy_mode { "ACCURACY" } else { "NORMAL" },
        config.fps,
        config.roi.y_min * 100.0, config.roi.y_max * 100.0,
        config.roi.x_min * 100.0, config.roi.x_max * 100.0,
    );

    let result = extract_hardcoded_subtitles(&video_path, &output_vtt_path, &model_dir, &config)?;

    println!("[SUCCESS] Xong! Tổng câu sub: {}", result.segments.len());
    println!("[SUCCESS] VTT: {:?}", result.vtt_path);
    Ok(())
}