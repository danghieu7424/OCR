//! Example: Trích xuất phụ đề cứng bằng OCR Library & Cấu hình đường dẫn mô hình
//!
//! Minh họa:
//! 1. Cấu hình vị trí cặp mô hình ONNX: `det.onnx` (DBNet Text Detection) và `rec.onnx` (PP-OCR Recognition).
//!    Tự động tải về nếu phát hiện thiếu file mô hình hoặc từ điển.
//! 2. Hiển thị thanh tiến trình trực quan thời gian thực (Real-time Progress Bar) trên Terminal.
//! 3. Cắm tự động vào `cli_gui_form::TaskWidget` (cấu trúc sẵn sàng để gắn vào TUI Form).
//! 4. Trích xuất phụ đề video từ thư mục `input/` sang WebVTT (.zh.vtt).

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ocr::{OcrExtractorConfig, extract_hardcoded_subtitles};
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Tải file mô hình PP-OCRv6 từ URL
fn download_model_file(url: &str, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    println!("    [DOWNLOAD] {}", url);

    let status = Command::new("curl.exe")
        .args([
            "-L",
            "--progress-bar",
            "-o",
            &output_path.to_string_lossy(),
            url,
        ])
        .status();

    match status {
        Ok(s) if s.success() && output_path.exists() => {
            println!(
                "    \x1b[32m[OK]\x1b[0m {:?}",
                output_path.file_name().unwrap_or_default()
            );
            Ok(())
        }
        _ => {
            println!("    [FALLBACK] PowerShell...");
            let ps = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                url,
                output_path.to_string_lossy()
            );
            Command::new("powershell")
                .args(["-Command", &ps])
                .status()?;
            if output_path.exists() {
                Ok(())
            } else {
                anyhow::bail!("Tải thất bại: {}", url);
            }
        }
    }
}

/// Kiểm tra trọn bộ mô hình PP-OCRv6 (det.onnx + rec.onnx + keys.txt).
fn ensure_models_exist(model_dir: &Path) -> Result<()> {
    let ocr_dir = model_dir.join("ocr");
    let det_model = ocr_dir.join("det.onnx");
    let rec_model = ocr_dir.join("rec.onnx");
    let keys_file = ocr_dir.join("keys.txt");

    if !det_model.exists() || !rec_model.exists() || !keys_file.exists() {
        let zip_candidates = [
            model_dir.join("ocr.zip"),
            std::path::PathBuf::from("ocr.zip"),
            std::path::PathBuf::from("models/ocr.zip"),
        ];

        let mut unzipped = false;
        for zip in &zip_candidates {
            if zip.exists() {
                println!(
                    ">>> [MODEL CHECK] Đang giải nén bộ mô hình PP-OCRv6 từ {:?}...",
                    zip
                );
                let _ = std::fs::create_dir_all(model_dir);
                let status = Command::new("tar")
                    .args([
                        "-xf",
                        &zip.to_string_lossy(),
                        "-C",
                        &model_dir.to_string_lossy(),
                    ])
                    .status();
                if let Ok(s) = status
                    && s.success()
                    && det_model.exists()
                    && rec_model.exists()
                    && keys_file.exists()
                {
                    println!("    \x1b[32m[OK]\x1b[0m Giải nén thành công bộ mô hình PP-OCRv6!");
                    unzipped = true;
                    break;
                }
            }
        }

        if !unzipped && (!det_model.exists() || !rec_model.exists() || !keys_file.exists()) {
            println!(">>> [MODEL CHECK] Tự động tải bộ mô hình PP-OCRv6 (Medium)...");
            std::fs::create_dir_all(&ocr_dir)?;

            if !det_model.exists() {
                println!(">>> [1/3] det.onnx (PP-OCRv6 Medium Detection)...");
                download_model_file(
                    "https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_det_onnx/resolve/main/inference.onnx",
                    &det_model,
                )?;
            }
            if !keys_file.exists() {
                println!(">>> [2/3] keys.txt (PP-OCRv6 Dictionary)...");
                download_model_file(
                    "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt",
                    &keys_file,
                )?;
            }
            if !rec_model.exists() {
                println!(">>> [3/3] rec.onnx (PP-OCRv6 Medium Recognition)...");
                download_model_file(
                    "https://huggingface.co/PaddlePaddle/PP-OCRv6_medium_rec_onnx/resolve/main/inference.onnx",
                    &rec_model,
                )?;
            }
            println!(">>> [MODEL CHECK] Tất cả các mô hình PP-OCRv6 đã sẵn sàng!\n");
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    println!(">>> [EXAMPLE] Khởi chạy trích xuất phụ đề cứng với OCR Library...\n");

    // =========================================================================
    // 1. Cấu hình vị trí cặp mô hình (det.onnx + rec.onnx + keys.txt)
    // =========================================================================
    // Hệ thống OCR cần cả 2 mô hình song kiếm hợp bích:
    // - det.onnx: DBNet Text Detection (phát hiện hộp tọa độ các dòng chữ trong khung hình)
    // - rec.onnx: PP-OCR Text Recognition (nhận diện các ký tự tiếng Trung từ hộp tọa độ)
    // - keys.txt: Từ điển ánh xạ index CTC sang chữ Hán
    let det_model = Path::new("models/ocr/det.onnx");
    let rec_model = Path::new("models/ocr/rec.onnx");
    let keys_file = Path::new("models/ocr/keys.txt");

    let (model_dir, model_info) = if det_model.exists() && rec_model.exists() && keys_file.exists()
    {
        (
            Path::new("models"),
            "Trọn bộ mô hình hoàn chỉnh tại 'models/ocr/' (det.onnx + rec.onnx + keys.txt)",
        )
    } else if Path::new("ocr/det.onnx").exists() && Path::new("ocr/rec.onnx").exists() {
        (Path::new("."), "Tìm thấy mô hình tại thư mục gốc './ocr/'")
    } else {
        (
            Path::new("models"),
            "Tự động kích hoạt tải mô hình vào thư mục 'models/ocr/'",
        )
    };

    println!("    [MODEL CONFIG] Thư mục mô hình: {:?}", model_dir);
    println!("    [MODEL STATUS] {}\n", model_info);

    // Tự động kiểm tra và tải về nếu thiếu bất kỳ file nào trong cặp det.onnx + rec.onnx + keys.txt
    ensure_models_exist(model_dir)?;

    // =========================================================================
    // 2. Cấu hình OCR Extractor (giữ trọn vẹn thông số công nghiệp độ chuẩn >99%)
    // =========================================================================
    // Tốc độ 10 fps tiêu chuẩn; có thể thử nhanh bằng OCR_SUB_FPS=12
    let config = OcrExtractorConfig {
        fps: std::env::var("OCR_SUB_FPS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10),
        accuracy_mode: true,
        ..OcrExtractorConfig::default()
    };
    println!(
        "    [ENGINE CONFIG] Mode: ACCURACY | FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%]",
        config.fps,
        config.roi.y_min * 100.0,
        config.roi.y_max * 100.0,
        config.roi.x_min * 100.0,
        config.roi.x_max * 100.0,
    );

    // =========================================================================
    // 3. Tích hợp thanh tiến trình (Progress Bar) & TaskWidget của cli_gui_form
    // =========================================================================
    let task_widget = Arc::new(Mutex::new(TaskWidget::new_loading(
        "ocr_video_task",
        "OCR Video Subtitles",
        "Đang chuẩn bị mô hình AI...",
        Color::Cyan,
    )));

    // A. Cắm widget trực tiếp vào hệ thống phát tiến trình của thư viện OCR
    ocr::attach_cli_gui_task(Arc::clone(&task_widget));

    // B. Hiển thị Progress Bar trực quan thời gian thực trên màn hình Console
    // Sử dụng ký tự khối bình hành hiện đại từ cli_gui_form (▰ và ▱)
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_width = 25usize;

    ocr::on_progress(move |p| {
        let total = p.total.max(1);
        let current = p.current.min(total);
        let ratio = (current as f32 / total as f32).clamp(0.0, 1.0);
        let filled_len = ((ratio * bar_width as f32).round() as usize).min(bar_width);
        let empty_len = bar_width.saturating_sub(filled_len);

        let bar_visual = filled_char.repeat(filled_len) + &empty_char.repeat(empty_len);

        // In đè tiến trình trên cùng một dòng bằng '\r' (In-place Terminal Progress Bar)
        print!(
            "\r \x1b[36m▶\x1b[0m \x1b[1m{}\x1b[0m \x1b[32m[{}]\x1b[0m \x1b[33m{:>5.1}%\x1b[0m \x1b[90m({}/{} {})\x1b[0m \x1b[35m[{}]\x1b[0m   ",
            p.title,
            bar_visual,
            ratio * 100.0,
            current,
            total,
            p.unit,
            p.elapsed
        );
        let _ = io::stdout().flush();
    });

    // C. Lắng nghe sự kiện chuyển bước
    ocr::on_step(|step| {
        println!("\n    \x1b[34m[STEP]\x1b[0m {}", step);
    });

    // =========================================================================
    // 4. Chọn video đầu vào (Ưu tiên thư mục `input/` theo yêu cầu)
    // =========================================================================
    // Chọn tập qua tham số: cargo run --release --example extract_subtitles -- 005
    let episode = std::env::args().nth(1).unwrap_or_else(|| "001".to_string());
    let video_name = format!("第{}集.mp4", episode);
    let candidates = [
        Path::new("input").join(&video_name),
        Path::new("video_input").join(&video_name),
    ];

    let video_path = candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| Path::new("input").join(&video_name));

    let output_dir = Path::new("output");
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    }
    let output_vtt_path = output_dir.join(format!("第{}集.zh.vtt", episode));

    if !video_path.exists() {
        eprintln!("\n    [ERROR] Không tìm thấy video tại {:?}.", video_path);
        eprintln!("    [INFO] Vui lòng đặt file video vào thư mục 'input/' hoặc 'video_input/'.");
        return Ok(());
    }

    println!("\n--------------------------------------------------");
    println!(">>> Bắt đầu trích xuất từ: {:?}", video_path);
    println!(">>> Kết quả sẽ lưu tại : {:?}", output_vtt_path);
    println!("--------------------------------------------------");

    // Khởi chạy xử lý trích xuất
    let output = extract_hardcoded_subtitles(&video_path, &output_vtt_path, model_dir, &config)?;

    // Xuống dòng sau khi Progress Bar hoàn tất
    println!("\n--------------------------------------------------");
    println!(
        ">>> \x1b[32m[SUCCESS]\x1b[0m Hoàn thành trích xuất! Tổng số câu phụ đề: \x1b[1m{}\x1b[0m",
        output.segments.len()
    );
    println!(
        ">>> \x1b[32m[SUCCESS]\x1b[0m File WebVTT: {:?}",
        output.vtt_path
    );

    // =========================================================================
    // 5. Kiểm tra dữ liệu TaskWidget của cli_gui_form đã được đồng bộ
    // =========================================================================
    if let Ok(w) = task_widget.lock()
        && let cli_gui_form::TaskState::Running {
            current,
            total,
            duration,
            status,
            ..
        } = &w.state
    {
        println!(
            "    \x1b[90m[CLI_GUI_FORM SYNC] TaskWidget State -> Current: {}/{} frames | Duration: {} | Status: {}\x1b[0m",
            current, total, duration, status
        );
    }

    Ok(())
}
