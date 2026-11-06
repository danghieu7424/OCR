//! Example: Trích xuất phụ đề cứng bằng OCR Library & Cấu hình đường dẫn mô hình
//!
//! Minh họa:
//! 1. Cấu hình vị trí model ONNX (tự động kiểm tra file và ưu tiên đường dẫn phù hợp).
//! 2. Cắm tiến trình vào `cli_gui_form::TaskWidget` (cấu trúc để sau này gắn vào TUI).
//! 3. Gọi thư viện `extract_hardcoded_subtitles` để sinh file WebVTT.

use anyhow::Result;
use cli_gui_form::TaskWidget;
use ratatui::style::Color;
use std::path::Path;
use std::sync::{Arc, Mutex};
use OCR::{extract_hardcoded_subtitles, OcrExtractorConfig};

fn main() -> Result<()> {
    println!(">>> [EXAMPLE] Khởi chạy trích xuất phụ đề cứng với OCR Library...");

    // Cấu hình vị trí model
    let model = Path::new("models/ocr/rec.onnx");

    // Chọn model cần dùng (tự động ưu tiên thư mục models/ocr nếu tồn tại, hoặc thư mục chỉ định)
    let (model_dir, _use) = if model.exists() {
        (Path::new("models"), "ppocr_v4_local")
    } else if Path::new("models/rec.onnx").exists() {
        (Path::new("."), "root_models")
    } else {
        (Path::new("models"), "default_models_dir")
    };
    println!("    [CONFIG] Vị trí thư mục model: {:?} (Engine: {})", model_dir, _use);

    // Cấu hình OCR Extractor (giữ trọn vẹn thông số tối ưu công nghiệp)
    let mut config = OcrExtractorConfig::default();
    config.fps = 10;
    config.accuracy_mode = true;
    println!(
        "    [CONFIG] Mode: ACCURACY | FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%]",
        config.fps,
        config.roi.y_min * 100.0,
        config.roi.y_max * 100.0,
        config.roi.x_min * 100.0,
        config.roi.x_max * 100.0,
    );

    // Cắm vào TaskWidget của cli_gui_form (cấu trúc sẵn sàng để sau này gắn vào TUI)
    let task_widget = Arc::new(Mutex::new(TaskWidget::new_loading(
        "ocr_video_task",
        "OCR Video Subtitles",
        "Đang khởi tạo mô hình ONNX...",
        Color::Cyan,
    )));

    // Kết nối widget trực tiếp với bộ phát tiến trình của thư viện OCR
    OCR::attach_cli_gui_task(Arc::clone(&task_widget));

    // Lắng nghe sự kiện tiến trình (progress callback)
    OCR::on_progress(|p| {
        println!(
            "    [PROGRESS] {} - {}/{} {} (Thời gian: {}, Hoàn thành: {})",
            p.title, p.current, p.total, p.unit, p.elapsed, p.detail
        );
    });

    // Lắng nghe thông điệp bước xử lý (step callback)
    OCR::on_step(|step| {
        println!("    [STEP] {}", step);
    });

    // Chọn video mẫu để trích xuất
    let video_path = Path::new("video_input/第001集.mp4");
    let output_vtt_path = Path::new("output/第001集_example.zh.vtt");

    if !video_path.exists() {
        println!("    [INFO] Video mẫu {:?} không tồn tại.", video_path);
        println!("    [INFO] Thư viện đã sẵn sàng, hãy truyền đường dẫn video thực tế để trích xuất.");
        return Ok(());
    }

    println!("--------------------------------------------------");
    println!(">>> Bắt đầu xử lý: {:?}", video_path);

    let output = extract_hardcoded_subtitles(video_path, output_vtt_path, model_dir, &config)?;

    println!("--------------------------------------------------");
    println!(
        ">>> [SUCCESS] Hoàn thành trích xuất! Tổng số câu: {}",
        output.segments.len()
    );
    println!(">>> [SUCCESS] File VTT đầu ra: {:?}", output.vtt_path);

    Ok(())
}

