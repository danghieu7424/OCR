// src/lib.rs
#![allow(non_snake_case)]
//! # OCR Library - Industrial Grade Hardcoded Subtitle & Caption Extractor
//!
//! Thư viện trích xuất phụ đề cứng (Hardcoded Subtitles), chú thích (Caption)
//! và hậu xử lý ngữ cảnh (LLM Proofreading) được cấu trúc thành các crate chuyên biệt:
//! - `OCR_Subtitles`: Trích xuất phụ đề lời thoại (khu vực dưới khung hình).
//! - `OCR_Captions`: Trích xuất chú thích nhân vật/công pháp (khu vực trên/trái khung hình).
//! - `OCR_LLM`: Hậu xử lý ngữ cảnh bằng Qwen2.5-VL qua Ollama và từ điển thực thể động (Entity Glossary).
//! - `OCR_Merge`: Ghép phụ đề và chú thích thành một file WebVTT đồng bộ thời gian hoàn chỉnh.

// Re-export các sub-crates dưới dạng namespace chuẩn
pub mod OCR_Subtitles {
    pub use OCR_Subtitles::*;
}

pub mod OCR_Captions {
    pub use OCR_Captions::*;
}

pub mod OCR_LLM {
    pub use OCR_LLM::*;
}

pub mod OCR_Merge {
    pub use OCR_Merge::*;
}

// Giữ các module alias cũ để đảm bảo tương thích ngược 100%
pub mod ocr {
    pub use OCR_Subtitles::*;
}
pub mod caption {
    pub use OCR_Captions::*;
}
pub mod temporal_voting {
    pub use OCR_Subtitles::temporal_voting::*;
}
pub mod glossary {
    pub use OCR_LLM::glossary::*;
}
pub mod ollama {
    pub use OCR_LLM::ollama::*;
}
pub mod proofread {
    pub use OCR_LLM::proofread::*;
}
pub mod shared {
    pub use ocr_shared::*;
}

// Re-export trực tiếp các struct/hàm quan trọng vào root namespace:
// 1. Phụ đề lời thoại (Subtitles)
pub use OCR_Subtitles::{
    extract_hardcoded_subtitles,
    OcrExtractorConfig,
    OcrExtractorOutput,
    SubtitleRoi,
    SubtitleSegment,
    vote_character_consensus,
    vote_majority_string,
};

// 2. Chú thích nhân vật (Captions)
pub use OCR_Captions::{
    extract_captions,
    CaptionExtractorConfig,
    CaptionExtractorOutput,
    CaptionRoi,
    CaptionSegment,
};

// 3. Hiệu đính ngữ cảnh & LLM (LLM & Glossary)
pub use OCR_LLM::{
    EntityGlossary,
    OllamaManager,
    ProofreadConfig,
    ProofreadSummary,
    Proofreader,
};

// 4. Hợp nhất phụ đề và chú thích (Merge)
pub use OCR_Merge::{
    format_vtt_timestamp,
    merge_cues,
    merge_vtt_files,
    parse_vtt,
    parse_vtt_timestamp,
    serialize_vtt,
    MergeSummary,
    VttCue,
};

// 5. Shared & TUI Events
pub use ocr_shared::tui_events::{
    attach_cli_gui_task,
    clear_handlers,
    emit_log,
    emit_step,
    emit_task_progress,
    on_log,
    on_progress,
    on_step,
    ProgressPayload,
};
