// src/lib.rs
#![allow(non_snake_case)]
//! # OCR Library - Industrial Grade Hardcoded Subtitle & Caption Extractor
//!
//! Module trích xuất phụ đề cứng (Hardcoded Subtitles) và chú thích (Caption)
//! từ video sang chuẩn WebVTT (.zh.vtt / .captions.zh.vtt).
//! Tích hợp PP-OCRv4/v5 ONNX, DBNet Text Detection, Temporal Denoising và Majority Voting.

#[path = "OCR/mod.rs"]
pub mod ocr;

pub mod caption;
pub mod temporal_voting;
pub mod shared;

// Re-export Subtitle extraction (phụ đề lời thoại — vùng dưới màn hình)
pub use ocr::{
    extract_hardcoded_subtitles,
    OcrExtractorConfig,
    OcrExtractorOutput,
    SubtitleRoi,
    SubtitleSegment,
};

// Re-export Caption extraction (chú thích nhân vật — vùng trên/bên trái)
pub use caption::{
    extract_captions,
    CaptionExtractorConfig,
    CaptionExtractorOutput,
    CaptionRoi,
    CaptionSegment,
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

