//! Example: Hợp nhất (Merge) phụ đề lời thoại và chú thích nhân vật thành 1 file WebVTT
//!
//! Chạy: cargo run --release --example merge_subtitles_captions -- [tập]
//! Ví dụ:
//!   cargo run --release --example merge_subtitles_captions -- 002
//!   cargo run --release --example merge_subtitles_captions -- all

use anyhow::Result;
use ocr::merge_vtt_files;
use std::env;
use std::path::PathBuf;

fn main() -> Result<()> {
    println!(">>> [MERGE] Hợp nhất Subtitles và Captions thành một file WebVTT đồng bộ\n");

    let args: Vec<String> = env::args().collect();
    let target = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    let ep_numbers: Vec<usize> = if target == "all" {
        (1..=20).collect()
    } else if let Ok(n) = target.parse::<usize>() {
        vec![n]
    } else {
        vec![1]
    };

    let mut total_merged = 0;

    for ep in ep_numbers {
        let ep_str = format!("{:03}", ep);
        // Ưu tiên VTT OCR thô theo tên hiện tại, rồi tương thích tên output cũ.
        let sub_vtt = {
            let raw = PathBuf::from(format!("output/第{}集.subtitles.zh.vtt", ep_str));
            if raw.exists() {
                raw
            } else {
                PathBuf::from(format!("output/第{}集.zh.vtt", ep_str))
            }
        };

        let cap_vtt = PathBuf::from(format!("output/第{}集.captions.zh.vtt", ep_str));
        let out_vtt = PathBuf::from(format!("output/第{}集.merged.zh.vtt", ep_str));

        if !sub_vtt.exists() && !cap_vtt.exists() {
            continue;
        }

        let summary = merge_vtt_files(&sub_vtt, &cap_vtt, &out_vtt)?;
        println!(
            "  ✔ Tập {}: Đã ghép thành công {} câu ({} lời thoại + {} chú thích) -> {:?}",
            ep_str, summary.total_merged_cues, summary.subtitle_cues, summary.caption_cues, out_vtt
        );
        total_merged += summary.total_merged_cues;
    }

    println!("\n--------------------------------------------------");
    println!(">>> [HOÀN TẤT] Tổng số cue được hợp nhất: {}", total_merged);
    println!("--------------------------------------------------");

    Ok(())
}
