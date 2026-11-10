//! Example: Trích xuất phụ đề cứng bằng OCR Library & Cấu hình đường dẫn mô hình
//!
//! Minh họa:
//! 1. Cấu hình vị trí cặp mô hình ONNX v6: `det.onnx` và `rec.onnx`.
//! 2. Tự động tải về PP-OCRv6 nếu phát hiện thiếu file.
//! 3. Điều chỉnh ROI trích xuất từ 10% đến 80% chiều cao màn hình.

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig, SubtitleRoi};

/// Tải file từ URL về máy qua lệnh curl hoặc PowerShell
fn download_model_file(url: &str, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    println!("    [DOWNLOAD] Đang tải từ : {}", url);
    println!("    [DOWNLOAD] Lưu vào file : {:?}", output_path);

    let curl_status = Command::new("curl.exe")
        .args(&["-L", "--progress-bar", "-o", &output_path.to_string_lossy(), url])
        .status();

    let success = match curl_status {
        Ok(s) if s.success() && output_path.exists() => true,
        _ => {
            println!("    [FALLBACK] Đang tải dự phòng qua PowerShell...");
            let ps_cmd = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                url,
                output_path.to_string_lossy()
            );
            let ps_status = Command::new("powershell")
                .args(&["-Command", &ps_cmd])
                .status();
            matches!(ps_status, Ok(s) if s.success() && output_path.exists())
        }
    };

    if success {
        println!("    \x1b[32m[DOWNLOAD SUCCESS]\x1b[0m Đã tải xong {:?}", output_path.file_name().unwrap_or_default());
        Ok(())
    } else {
        anyhow::bail!("Không thể tải file từ {}", url);
    }
}

/// Kiểm tra trọn bộ mô hình v6 (det.onnx + rec.onnx + keys.txt). Nếu thiếu sẽ tự động tải về.
fn ensure_models_exist(model_dir: &Path) -> Result<()> {
    let ocr_dir = model_dir.join("ocr");
    let det_model = ocr_dir.join("det.onnx");
    let rec_model = ocr_dir.join("rec.onnx");
    let keys_file = ocr_dir.join("keys.txt");

    if !det_model.exists() || !rec_model.exists() || !keys_file.exists() {
        println!(">>> [MODEL CHECK] Phát hiện thiếu file mô hình trong {:?}. Bắt đầu tự động tải v6...", ocr_dir);
        std::fs::create_dir_all(&ocr_dir)?;

        // 1. det.onnx: PP-OCRv6 Text Detection
        if !det_model.exists() {
            println!(">>> [1/3] Đang tải mô hình phát hiện chữ PP-OCRv6 (det.onnx)...");
            download_model_file(
                "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_det_onnx/resolve/main/inference.onnx",
                &det_model,
            )?;
        }

        // 2. keys.txt: Từ điển ký tự chuẩn PP-OCRv6 (18,708 dòng)
        if !keys_file.exists() {
            println!(">>> [2/3] Đang tải từ điển ký tự PP-OCRv6 (keys.txt)...");
            download_model_file(
                "https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/ppocr/utils/dict/ppocrv6_dict.txt",
                &keys_file,
            )?;
        }

        // 3. rec.onnx: PP-OCRv6 Text Recognition
        if !rec_model.exists() {
            println!(">>> [3/3] Đang tải mô hình nhận diện chữ PP-OCRv6 (rec.onnx)...");
            download_model_file(
                "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_onnx/resolve/main/inference.onnx",
                &rec_model,
            )?;
        }
        println!(">>> [MODEL CHECK] Tất cả các mô hình v6 đã sẵn sàng!\n");
    }
    Ok(())
}

fn main() -> Result<()> {
    println!(">>> [EXAMPLE] Khởi chạy trích xuất phụ đề cứng với OCR Library (PP-OCRv6)...\n");

    let det_model = Path::new("models/ocr/det.onnx");
    let rec_model = Path::new("models/ocr/rec.onnx");
    let keys_file = Path::new("models/ocr/keys.txt");

    let (model_dir, model_info) = if det_model.exists() && rec_model.exists() && keys_file.exists() {
        (
            Path::new("models"),
            "Trọn bộ mô hình v6 hoàn chỉnh tại 'models/ocr/' (det.onnx + rec.onnx + keys.txt)",
        )
    } else {
        (
            Path::new("models"),
            "Tự động kích hoạt tải mô hình v6 vào thư mục 'models/ocr/'",
        )
    };

    println!("    [MODEL CONFIG] Thư mục mô hình: {:?}", model_dir);
    println!("    [MODEL STATUS] {}\n", model_info);

    ensure_models_exist(model_dir)?;

    // =========================================================================
    // 2. Cấu hình OCR Extractor & Tùy chỉnh ROI (10% -> 80% chiều cao)
    // =========================================================================
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;
    config.accuracy_mode = true;

    // Thay đổi thông số ROI quét chữ từ 10% đến 80% chiều cao màn hình:
    config.roi = SubtitleRoi {
        y_min: 0.10, // Bắt đầu từ 10% chiều cao (từ trên xuống)
        y_max: 0.80, // Kết thúc ở 80% chiều cao
        x_min: 0.00, // Quét từ lề trái
        x_max: 1.00, // Quét tới lề phải
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
    // 3. Tích hợp thanh tiến trình & TaskWidget
    // =========================================================================
    let task_widget = Arc::new(Mutex::new(TaskWidget::new_loading(
        "ocr_video_task",
        "OCR Video Subtitles",
        "Đang chuẩn bị mô hình AI...",
        Color::Cyan,
    )));

    OCR::attach_cli_gui_task(Arc::clone(&task_widget));

    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_width = 25usize;

    OCR::on_progress(move |p| {
        let total = p.total.max(1);
        let current = p.current.min(total);
        let ratio = (current as f32 / total as f32).clamp(0.0, 1.0);
        let filled_len = ((ratio * bar_width as f32).round() as usize).min(bar_width);
        let empty_len = bar_width.saturating_sub(filled_len);

        let bar_visual = filled_char.repeat(filled_len) + &empty_char.repeat(empty_len);

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

    OCR::on_step(|step| {
        println!("\n    \x1b[34m[STEP]\x1b[0m {}", step);
    });

    // =========================================================================
    // 4. Đường dẫn video đầu vào & đầu ra
    // =========================================================================
    let candidates = [
        Path::new("input/第001集.mp4"),
        Path::new("video_input/第001集.mp4"),
    ];

    let video_path = candidates
        .iter()
        .copied()
        .find(|p| p.exists())
        .unwrap_or(Path::new("input/第001集.mp4"));

    let output_dir = Path::new("output");
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
    }
    let output_vtt_path = output_dir.join("第001集_example.zh.vtt");

    if !video_path.exists() {
        eprintln!("\n    [ERROR] Không tìm thấy video tại {:?}.", video_path);
        eprintln!("    [INFO] Vui lòng đặt file video vào thư mục 'input/' hoặc 'video_input/'.");
        return Ok(());
    }

    println!("\n--------------------------------------------------");
    println!(">>> Bắt đầu trích xuất từ: {:?}", video_path);
    println!(">>> Kết quả sẽ lưu tại : {:?}", output_vtt_path);
    println!("--------------------------------------------------");

    let output = extract_hardcoded_subtitles(video_path, &output_vtt_path, model_dir, &config)?;

    println!("\n--------------------------------------------------");
    println!(
        ">>> \x1b[32m[SUCCESS]\x1b[0m Hoàn thành trích xuất! Tổng số câu phụ đề: \x1b[1m{}\x1b[0m",
        output.segments.len()
    );
    println!(">>> \x1b[32m[SUCCESS]\x1b[0m File WebVTT: {:?}", output.vtt_path);

    Ok(())
}