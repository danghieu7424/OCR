//! Example: Trích xuất phụ đề cứng bằng OCR Library & Cấu hình đường dẫn mô hình
//!
//! Minh họa:
//! 1. Cấu hình vị trí cặp mô hình ONNX: `det.onnx` (DBNet Text Detection) và `rec.onnx` (PP-OCR Recognition).
//! 2. Hiển thị thanh tiến trình trực quan thời gian thực (Real-time Progress Bar) trên Terminal.
//! 3. Cắm tự động vào `cli_gui_form::TaskWidget` (cấu trúc sẵn sàng để gắn vào TUI Form).
//! 4. Trích xuất phụ đề video từ thư mục `input/` sang WebVTT (.zh.vtt).

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ratatui::style::Color;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig};

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

    let (model_dir, model_info) = if det_model.exists() && rec_model.exists() && keys_file.exists() {
        (
            Path::new("models"),
            "Trọn bộ mô hình hoàn chỉnh tại 'models/ocr/' (det.onnx + rec.onnx + keys.txt)",
        )
    } else if Path::new("ocr/det.onnx").exists() && Path::new("ocr/rec.onnx").exists() {
        (
            Path::new("."),
            "Tìm thấy mô hình tại thư mục gốc './ocr/'",
        )
    } else {
        (
            Path::new("models"),
            "Mặc định thư mục fallback 'models/'",
        )
    };

    println!("    [MODEL CONFIG] Thư mục mô hình: {:?}", model_dir);
    println!("    [MODEL STATUS] {}\n", model_info);

    // =========================================================================
    // 2. Cấu hình OCR Extractor (giữ trọn vẹn thông số công nghiệp độ chuẩn >99%)
    // =========================================================================
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;                // Tốc độ 10 fps tiêu chuẩn
    config.accuracy_mode = true;    // Chế độ nhận diện chính xác cao
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
    OCR::attach_cli_gui_task(Arc::clone(&task_widget));

    // B. Hiển thị Progress Bar trực quan thời gian thực trên màn hình Console
    // Sử dụng ký tự khối bình hành hiện đại từ cli_gui_form (▰ và ▱)
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_width = 25usize;

    OCR::on_progress(move |p| {
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
    OCR::on_step(|step| {
        println!("\n    \x1b[34m[STEP]\x1b[0m {}", step);
    });

    // =========================================================================
    // 4. Chọn video đầu vào (Ưu tiên thư mục `input/` theo yêu cầu)
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

    // Khởi chạy xử lý trích xuất
    let output = extract_hardcoded_subtitles(video_path, &output_vtt_path, model_dir, &config)?;

    // Xuống dòng sau khi Progress Bar hoàn tất
    println!("\n--------------------------------------------------");
    println!(
        ">>> \x1b[32m[SUCCESS]\x1b[0m Hoàn thành trích xuất! Tổng số câu phụ đề: \x1b[1m{}\x1b[0m",
        output.segments.len()
    );
    println!(">>> \x1b[32m[SUCCESS]\x1b[0m File WebVTT: {:?}", output.vtt_path);

    // =========================================================================
    // 5. Kiểm tra dữ liệu TaskWidget của cli_gui_form đã được đồng bộ
    // =========================================================================
    if let Ok(w) = task_widget.lock() {
        if let cli_gui_form::TaskState::Running {
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
    }

    Ok(())
}
