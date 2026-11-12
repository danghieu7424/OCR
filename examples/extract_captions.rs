//! Example: Trích xuất chú thích (Caption) từ video bằng OCR Library
//!
//! Caption là các chú thích như tên nhân vật (《陆清安》, 《林清雪》),
//! chú thích ngữ cảnh (《直到徐清莹突破大帝这天...》), thường xuất hiện
//! ở phần trên hoặc bên trái màn hình dưới dạng chữ dọc trên thanh đen.
//!
//! Hỗ trợ:
//! - Chữ NGANG: đọc bình thường trái → phải
//! - Chữ DỌC: đọc trên → dưới, cột trái → cột phải
//!
//! Chạy: cargo run --release --example extract_captions

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use OCR::{extract_captions, CaptionExtractorConfig, CaptionRoi};

/// Tải file từ URL
fn download_file(url: &str, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    println!("    [DOWNLOAD] {}", url);

    let status = Command::new("curl.exe")
        .args(&["-L", "--progress-bar", "-o", &output.to_string_lossy(), url])
        .status();

    match status {
        Ok(s) if s.success() && output.exists() => {
            println!("    \x1b[32m[OK]\x1b[0m {:?}", output.file_name().unwrap_or_default());
            Ok(())
        }
        _ => {
            println!("    [FALLBACK] PowerShell...");
            let ps = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                url, output.to_string_lossy()
            );
            Command::new("powershell").args(&["-Command", &ps]).status()?;
            if output.exists() {
                Ok(())
            } else {
                anyhow::bail!("Tải thất bại: {}", url);
            }
        }
    }
}

/// Kiểm tra & tải model nếu thiếu
fn ensure_models(model_dir: &Path) -> Result<()> {
    let ocr = model_dir.join("ocr");
    let det = ocr.join("det.onnx");
    let rec = ocr.join("rec.onnx");
    let keys = ocr.join("keys.txt");

    if det.exists() && rec.exists() && keys.exists() {
        return Ok(());
    }

    println!(">>> [MODEL] Phát hiện thiếu file mô hình. Tự động tải...");
    std::fs::create_dir_all(&ocr)?;

    if !det.exists() {
        println!(">>> [1/3] det.onnx (Text Detection)...");
        download_file(
            "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/main/inference.onnx",
            &det,
        )?;
    }
    if !keys.exists() {
        println!(">>> [2/3] keys.txt (Dictionary)...");
        download_file(
            "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt",
            &keys,
        )?;
    }
    if !rec.exists() {
        println!(">>> [3/3] rec.onnx (Text Recognition)...");
        download_file(
            "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/main/inference.onnx",
            &rec,
        )?;
    }
    println!(">>> [MODEL] Sẵn sàng!\n");
    Ok(())
}

fn main() -> Result<()> {
    println!(">>> [CAPTION EXAMPLE] Trích xuất chú thích (Caption) từ video\n");

    // === Model ===
    let model_dir = Path::new("models");
    let det = model_dir.join("ocr/det.onnx");
    let rec = model_dir.join("ocr/rec.onnx");
    let keys = model_dir.join("ocr/keys.txt");

    if det.exists() && rec.exists() && keys.exists() {
        println!("    [MODEL] Trọn bộ mô hình tại 'models/ocr/'");
    } else {
        println!("    [MODEL] Thiếu file, sẽ tự động tải...");
    }
    ensure_models(model_dir)?;

    // === Config ===
    let config = CaptionExtractorConfig {
        fps: 5,
        roi: CaptionRoi {
            y_min: 0.05,
            y_max: 0.75,
            x_min: 0.00,
            x_max: 0.50,
        },
        vertical_aspect_threshold: 1.2,
        column_x_tolerance: 0.10,
        min_confidence: 0.30,
    };

    println!(
        "    [CONFIG] FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%] | V_Threshold: {:.1}",
        config.fps,
        config.roi.y_min * 100.0, config.roi.y_max * 100.0,
        config.roi.x_min * 100.0, config.roi.x_max * 100.0,
        config.vertical_aspect_threshold,
    );

    // === Progress bar ===
    let task_widget = Arc::new(Mutex::new(TaskWidget::new_loading(
        "caption_task",
        "Caption Extraction",
        "Đang chuẩn bị...",
        Color::Yellow,
    )));
    OCR::attach_cli_gui_task(Arc::clone(&task_widget));

    let (filled, empty) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_w = 25usize;

    OCR::on_progress(move |p| {
        let total = p.total.max(1);
        let cur = p.current.min(total);
        let ratio = (cur as f32 / total as f32).clamp(0.0, 1.0);
        let filled_n = ((ratio * bar_w as f32).round() as usize).min(bar_w);
        let empty_n = bar_w.saturating_sub(filled_n);

        let bar = filled.repeat(filled_n) + &empty.repeat(empty_n);

        print!(
            "\r \x1b[33m▶\x1b[0m \x1b[1m{}\x1b[0m \x1b[32m[{}]\x1b[0m \x1b[33m{:>5.1}%\x1b[0m \x1b[90m({}/{} {})\x1b[0m \x1b[35m[{}]\x1b[0m   ",
            p.title, bar, ratio * 100.0, cur, total, p.unit, p.elapsed
        );
        let _ = io::stdout().flush();
    });

    OCR::on_step(|step| {
        println!("\n    \x1b[34m[STEP]\x1b[0m {}", step);
    });

    // === Video ===
    let candidates = [
        Path::new("input/第001集.mp4"),
        Path::new("video_input/第001集.mp4"),
    ];
    let video = candidates.iter().copied().find(|p| p.exists())
        .unwrap_or(Path::new("input/第001集.mp4"));

    let output_dir = Path::new("output");
    std::fs::create_dir_all(output_dir)?;
    let output_vtt = output_dir.join("第001集.captions.zh.vtt");

    if !video.exists() {
        eprintln!("\n    [ERROR] Video không tìm thấy: {:?}", video);
        eprintln!("    [INFO] Đặt video vào 'input/' hoặc 'video_input/'");
        return Ok(());
    }

    println!("\n--------------------------------------------------");
    println!(">>> Video : {:?}", video);
    println!(">>> Output: {:?}", output_vtt);
    println!("--------------------------------------------------");

    let output = extract_captions(video, &output_vtt, model_dir, &config)?;

    println!("\n--------------------------------------------------");
    println!(
        ">>> \x1b[32m[SUCCESS]\x1b[0m Tổng số caption: \x1b[1m{}\x1b[0m",
        output.segments.len()
    );
    println!(">>> \x1b[32m[SUCCESS]\x1b[0m File: {:?}", output.vtt_path);

    // In preview
    println!("\n--- Preview ---");
    for (i, seg) in output.segments.iter().take(15).enumerate() {
        println!(
            "  {:2}. {} --> {} | {}",
            i + 1,
            OCR::CaptionSegment::format_vtt_timestamp(seg.start_sec),
            OCR::CaptionSegment::format_vtt_timestamp(seg.end_sec),
            seg.text
        );
    }
    if output.segments.len() > 15 {
        println!("  ... và {} caption nữa", output.segments.len() - 15);
    }

    Ok(())
}
