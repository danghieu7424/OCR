// src/main.rs
use ocr::{OcrExtractorConfig, extract_hardcoded_subtitles};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

fn init_logger(log_path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let mut lock = LOG_FILE.lock().unwrap();
    *lock = Some(file);
    Ok(())
}

fn log_msg(msg: &str) {
    println!("{}", msg);
    if let Ok(mut lock) = LOG_FILE.lock()
        && let Some(ref mut f) = *lock
    {
        let _ = writeln!(f, "{}", msg);
        let _ = f.flush();
    }
}

fn log_err(msg: &str) {
    eprintln!("{}", msg);
    if let Ok(mut lock) = LOG_FILE.lock()
        && let Some(ref mut f) = *lock
    {
        let _ = writeln!(f, "{}", msg);
        let _ = f.flush();
    }
}

macro_rules! log_println {
    () => {
        log_msg("")
    };
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log_msg(&msg);
    }};
}

macro_rules! log_eprintln {
    () => {
        log_err("")
    };
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log_err(&msg);
    }};
}

fn main() -> anyhow::Result<()> {
    // Khởi tạo ghi log vào logs/1.log (chế độ append để bảo toàn log trước đó)
    let log_path = PathBuf::from("logs/1.log");
    if let Err(e) = init_logger(&log_path) {
        eprintln!("    [WARN] Không thể mở file log {:?}: {}", log_path, e);
    }

    log_println!(">>> [TEST] Khởi chạy kiểm tra module OCR phụ đề cứng cho tất cả video...");

    let input_dir = PathBuf::from("C:/Users/dangh/Videos/Hongguo/和离后我带女儿横扫魔域");
    if !input_dir.exists() {
        fs::create_dir_all(&input_dir)?;
        log_println!("    [INFO] Đã tạo thư mục 'video_input/'. Vui lòng bỏ file video vào đây!");
        return Ok(());
    }

    // Lấy tất cả các file video hợp lệ trong thư mục và sắp xếp theo thứ tự
    let mut video_paths: Vec<PathBuf> = input_dir
        .read_dir()?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| matches!(ext.to_lowercase().as_str(), "mp4" | "mkv" | "avi" | "mov"))
                    .unwrap_or(false)
        })
        .collect();

    if video_paths.is_empty() {
        return Err(anyhow::anyhow!(
            "Không tìm thấy file video nào trong thư mục input!"
        ));
    }

    // Sắp xếp danh sách video theo thứ tự tên file (第001集, 第002集, ...)
    video_paths.sort();

    let total_videos = video_paths.len();
    log_println!("    [INFO] Tìm thấy {} video cần xử lý.", total_videos);

    let output_dir = PathBuf::from("output");
    if !output_dir.exists() {
        fs::create_dir_all(&output_dir)?;
    }
    let model_dir = PathBuf::from("models");

    let config = OcrExtractorConfig {
        fps: 10,
        accuracy_mode: true,
        ..OcrExtractorConfig::default()
    };

    log_println!(
        "    [INFO] Mode: {} | FPS: {} | ROI: Y[{:.0}%-{:.0}%] X[{:.0}%-{:.0}%]\n",
        if config.accuracy_mode {
            "ACCURACY"
        } else {
            "NORMAL"
        },
        config.fps,
        config.roi.y_min * 100.0,
        config.roi.y_max * 100.0,
        config.roi.x_min * 100.0,
        config.roi.x_max * 100.0,
    );

    // Kiểm tra xem output đã chạy tới tệp nào để resume khi chạy lại
    let mut finished_count = 0usize;
    let mut first_pending_idx: Option<usize> = None;

    for (idx, video_path) in video_paths.iter().enumerate() {
        let video_stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("subtitles");
        let output_vtt_path = output_dir.join(format!("{}.zh.vtt", video_stem));

        let is_done = output_vtt_path.exists()
            && fs::metadata(&output_vtt_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false);

        if is_done {
            finished_count += 1;
        } else if first_pending_idx.is_none() {
            first_pending_idx = Some(idx);
        }
    }

    log_println!(
        "    [CHECKPOINT] Đã hoàn thành: {}/{} video trong output. Còn lại: {} video.",
        finished_count,
        total_videos,
        total_videos.saturating_sub(finished_count)
    );

    if let Some(start_idx) = first_pending_idx {
        log_println!(
            "    [RESUME] Sẽ tiếp tục xử lý từ video [{}/{}]: {:?}\n",
            start_idx + 1,
            total_videos,
            video_paths[start_idx].file_name().unwrap_or_default()
        );
    } else {
        log_println!("    [DONE] Toàn bộ video đã được xử lý xong trong output!");
        return Ok(());
    }

    // Duyệt qua từng video và tiến hành trích xuất phụ đề (bỏ qua các video đã hoàn thành)
    for (idx, video_path) in video_paths.iter().enumerate() {
        let video_stem = video_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("subtitles");

        let output_vtt_path = output_dir.join(format!("{}.zh.vtt", video_stem));

        // Kiểm tra nếu file output đã tồn tại và hợp lệ thì bỏ qua (tiếp tục từ file chưa chạy)
        if output_vtt_path.exists()
            && fs::metadata(&output_vtt_path)
                .map(|m| m.len() > 0)
                .unwrap_or(false)
        {
            continue;
        }

        log_println!("--------------------------------------------------");
        log_println!(
            ">>>  [{}/{}] Đang xử lý: {:?}",
            idx + 1,
            total_videos,
            video_path
        );

        match extract_hardcoded_subtitles(video_path, &output_vtt_path, &model_dir, &config) {
            Ok(result) => {
                log_println!(
                    " [SUCCESS] Xong video {:?}! Tổng câu sub: {}",
                    video_path.file_name().unwrap_or_default(),
                    result.segments.len()
                );
                log_println!(" [SUCCESS] VTT: {:?}", result.vtt_path);
            }
            Err(err) => {
                log_eprintln!("   [ERROR] Thất bại khi xử lý {:?}: {}", video_path, err);
            }
        }
    }

    log_println!("\n>>> [DONE] Đã xử lý xong toàn bộ danh sách video.");
    Ok(())
}
