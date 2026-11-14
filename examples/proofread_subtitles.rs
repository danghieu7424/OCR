//! Example: Hiệu đính phụ đề (Proofread) kết hợp Entity Glossary & Qwen2.5-VL Context Engine
//!
//! Chạy với LLM (mặc định):
//!   cargo run --release --example proofread_subtitles -- 002
//!
//! Chạy siêu nhanh Fast Mode (tắt LLM, chỉ dùng Entity Glossary & Sanitizer, 0.00s/câu):
//!   cargo run --release --example proofread_subtitles -- 002 --fast
//!   cargo run --release --example proofread_subtitles -- all --fast

use anyhow::Result;
use std::env;
use std::path::PathBuf;
use OCR::{EntityGlossary, Proofreader};

fn main() -> Result<()> {
    println!(">>> [PROOFREADER] Hậu xử lý & Hiệu đính ngữ cảnh phụ đề WebVTT\n");

    let args: Vec<String> = env::args().collect();
    let use_fast_mode = args.iter().any(|a| a == "--fast" || a == "--no-llm")
        || std::env::var("OCR_USE_LLM")
            .map(|v| v == "0" || v.to_lowercase() == "false" || v.to_lowercase() == "off")
            .unwrap_or(false);

    let target = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .unwrap_or("all");

    // 1. Tự động thu thập danh từ riêng từ tất cả các file caption
    println!(">>> [1/3] Quét danh từ riêng (Entity Glossary) từ Caption...");
    let caption_files: Vec<PathBuf> = (1..=20)
        .map(|i| PathBuf::from(format!("output/第{:03}集.captions.zh.vtt", i)))
        .filter(|p| p.exists())
        .collect();

    let glossary = EntityGlossary::from_caption_files(&caption_files)?;
    println!(
        "    [GLOSSARY] Đã tìm thấy {} thực thể chuẩn từ Caption:",
        glossary.terms().len()
    );
    for (idx, term) in glossary.terms().iter().take(10).enumerate() {
        println!("      {:2}. {}", idx + 1, term);
    }
    if glossary.terms().len() > 10 {
        println!("      ... và {} thực thể khác", glossary.terms().len() - 10);
    }
    println!();

    // 2. Khởi tạo Proofreader (hỗ trợ bật/tắt LLM)
    let proofreader = if use_fast_mode {
        println!(">>> [2/3] Chế độ SIÊU NHANH (Fast Mode: Đã TẮT LLM để tiết kiệm thời gian)...");
        println!("    \x1b[32m[FAST MODE]\x1b[0m Sử dụng Entity Glossary & Bộ lọc khử nhiễu (tốc độ 0.00s/câu)");
        Proofreader::with_fast_mode()
    } else {
        println!(">>> [2/3] Khởi tạo Proofreader (Tự động kích hoạt Ollama & mô hình Qwen2.5-VL)...");
        println!("    \x1b[33m[MẸO]\x1b[0m Thêm cờ `--fast` hoặc `--no-llm` để chạy tức thì không cần đợi LLM.");
        let model = std::env::var("OCR_LLM_MODEL").unwrap_or_else(|_| "qwen2.5vl:3b".to_string());
        println!("    [OLLAMA] Đang kết nối tới mô hình `{}` qua ollama-rs...", model);

        match Proofreader::with_ollama(&model) {
            Ok(p) => {
                println!(
                    "    \x1b[32m[OLLAMA]\x1b[0m Thư viện Ollama Rust đã sẵn sàng với mô hình `{}`",
                    model
                );
                p
            }
            Err(e) => {
                println!(
                    "    \x1b[33m[FALLBACK]\x1b[0m Không thể kết nối Ollama ({}). Sử dụng Fast Engine nội bộ CPU",
                    e
                );
                Proofreader::with_fast_mode()
            }
        }
    };
    println!();

    // 3. Xử lý tập chỉ định hoặc toàn bộ
    println!(">>> [3/3] Hiệu đính phụ đề...");
    let ep_numbers: Vec<usize> = if target == "all" {
        (1..=20).collect()
    } else if let Ok(n) = target.parse::<usize>() {
        vec![n]
    } else {
        vec![1]
    };

    let mut total_fixed = 0;

    for ep in ep_numbers {
        let ep_str = format!("{:03}", ep);
        let in_vtt = PathBuf::from(format!("output/第{}集.zh.vtt", ep_str));
        if !in_vtt.exists() {
            continue;
        }
        let out_vtt = PathBuf::from(format!("output/第{}集.refined.zh.vtt", ep_str));

        let summary = proofreader.proofread_vtt_file(&in_vtt, &out_vtt, Some(&glossary))?;
        if summary.corrected_lines > 0 {
            println!(
                "  ▶ Tập {}: Đã chuẩn hóa {}/{} dòng câu",
                ep_str, summary.corrected_lines, summary.total_lines
            );
            for (old, new) in &summary.corrections {
                println!("     [-] \"{}\" \n     [+] \"{}\"", old, new);
            }
            total_fixed += summary.corrected_lines;
        } else {
            println!("  ✔ Tập {}: Phụ đề đã chuẩn xác (0 câu cần sửa)", ep_str);
        }
    }

    println!("\n--------------------------------------------------");
    println!(">>> [HOÀN TẤT] Tổng số câu được chuẩn hóa: {}", total_fixed);
    println!("--------------------------------------------------");

    Ok(())
}
