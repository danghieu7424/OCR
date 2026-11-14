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
//! Chạy:
//! - Hàng loạt (mặc định): `cargo run --release --example extract_captions`
//!   quét mọi video trong `input/`, bỏ qua tập đã có VTT hợp lệ (checkpoint/resume).
//! - Một tập: `cargo run --release --example extract_captions -- 002`

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use OCR::{extract_captions, CaptionExtractorConfig, CaptionRoi};



/// Tải file mô hình PP-OCRv6 từ URL
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

/// Kiểm tra & nạp model PP-OCRv6 (giải nén từ ocr.zip hoặc tải tự động từ HuggingFace PP-OCRv6 Medium)
fn ensure_models(model_dir: &Path) -> Result<()> {
    let ocr = model_dir.join("ocr");
    let det = ocr.join("det.onnx");
    let rec = ocr.join("rec.onnx");
    let keys = ocr.join("keys.txt");

    if det.exists() && rec.exists() && keys.exists() {
        return Ok(());
    }

    // Ưu tiên giải nén từ file ocr.zip
    let zip_candidates = [
        model_dir.join("ocr.zip"),
        std::path::PathBuf::from("ocr.zip"),
        std::path::PathBuf::from("models/ocr.zip"),
    ];

    for zip in &zip_candidates {
        if zip.exists() {
            println!(">>> [MODEL CHECK] Đang giải nén bộ mô hình PP-OCRv6 từ {:?}...", zip);
            let _ = std::fs::create_dir_all(model_dir);
            let status = Command::new("tar")
                .args(&["-xf", &zip.to_string_lossy(), "-C", &model_dir.to_string_lossy()])
                .status();
            if let Ok(s) = status {
                if s.success() && det.exists() && rec.exists() && keys.exists() {
                    println!("    \x1b[32m[OK]\x1b[0m Giải nén thành công bộ mô hình PP-OCRv6!");
                    return Ok(());
                }
            }
        }
    }

    println!(">>> [MODEL] Tự động tải bộ mô hình PP-OCRv6 (Medium)...");
    std::fs::create_dir_all(&ocr)?;

    if !det.exists() {
        println!(">>> [1/3] det.onnx (PP-OCRv6 Medium Detection)...");
        download_file(
            "https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/main/inference.onnx",
            &det,
        )?;
    }
    if !keys.exists() {
        println!(">>> [2/3] keys.txt (PP-OCRv6 Dictionary)...");
        download_file(
            "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt",
            &keys,
        )?;
    }
    if !rec.exists() {
        println!(">>> [3/3] rec.onnx (PP-OCRv6 Medium Recognition)...");
        download_file(
            "https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/main/inference.onnx",
            &rec,
        )?;
    }

    println!(">>> [MODEL] Sẵn sàng bộ mô hình PP-OCRv6!\n");
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
            y_min: 0.10,
            y_max: 0.80,
            x_min: 0.00,
            x_max: 1.00,
        },
        vertical_aspect_threshold: 1.2,
        column_x_tolerance: 0.06,
        min_confidence: 0.80,   // caption mờ/nhòe: hạ về 0.30
        min_duration_sec: 1.0, // caption rất ngắn: hạ về ~0.4
        watermark_freq_threshold: 0.40,
    };

    println!(
        "    [CONFIG] FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%] | V_Threshold: {:.1} | MinConf: {:.2} | MinDur: {:.1}s",
        config.fps,
        config.roi.y_min * 100.0, config.roi.y_max * 100.0,
        config.roi.x_min * 100.0, config.roi.x_max * 100.0,
        config.vertical_aspect_threshold,
        config.min_confidence,
        config.min_duration_sec,
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

    // === Chế độ chạy ===
    //   (không tham số) hoặc `-- all` : quét cả input/
    //   `-- 002`                      : chỉ tập 002
    match std::env::args().nth(1).as_deref() {
        Some(ep) if ep != "all" => run_one(ep, model_dir, &config),
        _ => run_batch(model_dir, &config),
    }
}

/// Trích xuất 1 video và in preview kết quả.
fn extract_and_report(
    video: &Path,
    output_vtt: &Path,
    model_dir: &Path,
    config: &CaptionExtractorConfig,
) -> Result<()> {
    println!("\n--------------------------------------------------");
    println!(">>> Video : {:?}", video);
    println!(">>> Output: {:?}", output_vtt);
    println!("--------------------------------------------------");

    let output = extract_captions(video, output_vtt, model_dir, config)?;

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

/// Chạy 1 tập: `第<episode>集.mp4` trong input/ hoặc video_input/.
fn run_one(episode: &str, model_dir: &Path, config: &CaptionExtractorConfig) -> Result<()> {
    let video_name = format!("第{}集.mp4", episode);
    let candidates = [
        Path::new("input").join(&video_name),
        Path::new("video_input").join(&video_name),
    ];
    let video = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| Path::new("input").join(&video_name));

    let output_dir = Path::new("output");
    std::fs::create_dir_all(output_dir)?;
    let output_vtt = output_dir.join(format!("第{}集.captions.zh.vtt", episode));

    if !video.exists() {
        eprintln!("\n    [ERROR] Video không tìm thấy: {:?}", video);
        eprintln!("    [INFO] Đặt video vào 'input/' hoặc 'video_input/'");
        return Ok(());
    }

    extract_and_report(&video, &output_vtt, model_dir, config)
}

/// Đường dẫn VTT caption tương ứng với 1 video: `output/<stem>.captions.zh.vtt`.
fn caption_path(video: &Path, output_dir: &Path) -> Option<std::path::PathBuf> {
    let stem = video.file_stem()?.to_str()?;
    Some(output_dir.join(format!("{}.captions.zh.vtt", stem)))
}

/// Checkpoint: VTT đã tồn tại và có nội dung => coi như đã xong.
fn is_done(video: &Path, output_dir: &Path) -> bool {
    caption_path(video, output_dir)
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len() > 0)
        .unwrap_or(false)
}

/// Chạy hàng loạt: quét mọi video trong input/, resume từ tập chưa có VTT.
fn run_batch(model_dir: &Path, config: &CaptionExtractorConfig) -> Result<()> {
    let input_dir = Path::new("input");
    let output_dir = Path::new("output");
    std::fs::create_dir_all(output_dir)?;

    let mut videos: Vec<std::path::PathBuf> = std::fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "mov"))
                    .unwrap_or(false)
        })
        .collect();

    if videos.is_empty() {
        eprintln!("    [ERROR] Không tìm thấy video nào trong 'input/'");
        return Ok(());
    }
    videos.sort();

    let pending: Vec<&std::path::PathBuf> =
        videos.iter().filter(|v| !is_done(v, output_dir)).collect();

    println!(
        "    [CHECKPOINT] Đã có VTT: {}/{} video | Còn lại: {} video",
        videos.len() - pending.len(),
        videos.len(),
        pending.len()
    );

    if pending.is_empty() {
        println!("    [DONE] Toàn bộ video đã có caption trong output!");
        return Ok(());
    }

    let mut failed = 0usize;
    for (idx, video) in pending.iter().enumerate() {
        let Some(out) = caption_path(video, output_dir) else { continue };
        println!(
            "\n>>> [{}/{}] Đang xử lý: {:?}",
            idx + 1,
            pending.len(),
            video.file_name().unwrap_or_default()
        );
        if let Err(err) = extract_and_report(video, &out, model_dir, config) {
            eprintln!(
                "    [ERROR] Thất bại {:?}: {}",
                video.file_name().unwrap_or_default(),
                err
            );
            failed += 1;
        }
    }

    println!(
        "\n>>> [DONE] Xong {}/{} video, lỗi: {}",
        pending.len() - failed,
        pending.len(),
        failed
    );
    Ok(())
}
