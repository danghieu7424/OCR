#![allow(non_snake_case)]
// crates/OCR_LLM/src/lib.rs
//! # OCR_LLM - LLM Post-processing & Context Proofreading
//!
//! Tầng hậu xử lý thông minh:
//! - Quản lý mô hình ngôn ngữ lớn (Qwen2.5-VL-3B-Instruct) tự động qua thư viện `ollama-rs`.
//! - Hỗ trợ bật/tắt LLM linh hoạt (`Proofreader::with_fast_mode` cho tốc độ 0.00s hoặc `Proofreader::with_ollama`).
//! - Khai thác từ điển thực thể động `EntityGlossary` từ file caption.
//! - Bộ lọc làm sạch nhiễu OCR thuật toán (`sanitize_ocr_noise`).

pub mod glossary;
pub mod ollama;
pub mod proofread;

pub use glossary::EntityGlossary;
pub use ollama::OllamaManager;
pub use proofread::{ProofreadConfig, ProofreadSummary, Proofreader};
