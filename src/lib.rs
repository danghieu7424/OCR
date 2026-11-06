// src/lib.rs
#![allow(non_snake_case)]
//! # OCR Library - Industrial Grade Hardcoded Subtitle Extractor
//!
//! Module trích xuất phụ đề cứng (Hardcoded Subtitles) từ video sang chuẩn WebVTT (.zh.vtt).
//! Tích hợp PP-OCRv4/v5 ONNX, DBNet Text Detection, Temporal Denoising và Majority Voting.

#[path = "OCR/mod.rs"]
pub mod ocr;

pub mod shared;

// Re-export các kiểu cấu hình và hàm trích xuất chính
pub use ocr::{
    extract_hardcoded_subtitles,
    OcrExtractorConfig,
    OcrExtractorOutput,
    SubtitleRoi,
    SubtitleSegment,
};

// Re-export các hàm lắng nghe sự kiện tiến trình & tích hợp cli_gui_form
pub use shared::tui_events::{
    attach_cli_gui_task,
    clear_handlers,
    on_log,
    on_progress,
    on_step,
    ProgressPayload,
};

