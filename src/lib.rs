// src/lib.rs
//! # OCR Library - Hardcoded Subtitle & Caption Extractor
//!
//! Thư viện trích xuất phụ đề cứng (Hardcoded Subtitles) và chú thích (Caption)
//! được cấu trúc thành các crate chuyên biệt:
//! - `ocr_subtitles`: Trích xuất phụ đề lời thoại (khu vực dưới khung hình).
//! - `ocr_captions`: Trích xuất chú thích nhân vật/công pháp (khu vực trên/trái khung hình).
//! - `ocr_merge`: Ghép phụ đề và chú thích thành một file WebVTT đồng bộ thời gian hoàn chỉnh.

pub use ocr_captions;
pub use ocr_merge;
pub use ocr_subtitles;

// Giữ các module alias cũ để đảm bảo tương thích ngược 100%
pub mod ocr {
    pub use ocr_subtitles::*;
}
pub mod caption {
    pub use ocr_captions::*;
}
pub mod temporal_voting {
    pub use ocr_subtitles::temporal_voting::*;
}
pub mod shared {
    pub use ocr_shared::*;
}

// Re-export trực tiếp các struct/hàm quan trọng vào root namespace:
// 1. Phụ đề lời thoại (Subtitles)
pub use ocr_subtitles::{
    OcrExtractorConfig, OcrExtractorOutput, SubtitleRoi, SubtitleSegment,
    extract_hardcoded_subtitles, vote_character_consensus, vote_majority_string,
};

// 2. Chú thích nhân vật (Captions)
pub use ocr_captions::{
    CaptionExtractorConfig, CaptionExtractorOutput, CaptionRoi, CaptionSegment, CaptionWarning,
    extract_captions,
};

// 3. Hợp nhất phụ đề và chú thích (Merge)
pub use ocr_merge::{
    MergeSummary, VttCue, format_vtt_timestamp, merge_cues, merge_vtt_files, parse_vtt,
    parse_vtt_timestamp, serialize_vtt,
};

// 4. Shared & TUI Events
pub use ocr_shared::tui_events::{
    ProgressPayload, attach_cli_gui_task, clear_handlers, emit_log, emit_step, emit_task_progress,
    on_log, on_progress, on_step,
};
