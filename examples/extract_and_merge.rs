//! Example: Pipeline Trọn Gói Tự Động (Trích xuất Subtitle + Caption + Merge không cần LLM)
//!
//! Quy trình 4 bước chuẩn công nghiệp:
//! 1. Trích xuất phụ đề lời thoại (Subtitles) ở khu vực dưới màn hình -> `output/第XXX集.zh.vtt`.
//! 2. Trích xuất chú thích nhân vật/công pháp (Captions) -> `output/第XXX集.captions.zh.vtt`.
//! 3. Tự động bóc tách danh từ riêng (Entity Glossary) và chuẩn hóa phụ đề ở chế độ Fast Mode (0.01s, không dùng LLM) -> `output/第XXX集.refined.zh.vtt`.
//! 4. Ghép nối (Merge) lời thoại và chú thích thành file WebVTT đồng bộ thời gian hoàn chỉnh -> `output/第XXX集.merged.zh.vtt`.
//!
//! Cách chạy:
//! - Cho một tập chỉ định:
//!     cargo run --release --example extract_and_merge -- 002
//!     cargo run --release --example extract_and_merge -- input/第002集.mp4
//! - Cho toàn bộ video trong input/ (mặc định):
//!     cargo run --release --example extract_and_merge -- all
//!     cargo run --release --example extract_and_merge

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use OCR::{
    extract_captions, extract_hardcoded_subtitles, merge_vtt_files, CaptionExtractorConfig,
    EntityGlossary, OcrExtractorConfig, Proofreader,
};

/// Kiểm tra bộ mô hình PP-OCRv6 (`det.onnx`, `rec.onnx`, `keys.txt`)
fn ensure_models(model_dir: &Path) -> Result<()> {
    let ocr = model_dir.join("ocr");
    let det = ocr.join("det.onnx");
    let rec = ocr.join("rec.onnx");
    let keys = ocr.join("keys.txt");

    if !det.exists() || !rec.exists() || !keys.exists() {
        // Ưu tiên giải nén từ file `ocr.zip` chứa đầy đủ mô hình PP-OCRv6
        let zip_candidates = [
            model_dir.join("ocr.zip"),
            PathBuf::from("ocr.zip"),
            PathBuf::from("models/ocr.zip"),
        ];

        let mut unzipped = false;
        for zip in &zip_candidates {
            if zip.exists() {
                println!(">>> [MODEL CHECK] Đang giải nén bộ mô hình PP-OCRv6 từ {:?}...", zip);
                let _ = fs::create_dir_all(model_dir);
                let status = Command::new("tar")
                    .args(&["-xf", &zip.to_string_lossy(), "-C", &model_dir.to_string_lossy()])
                    .status();
                if let Ok(s) = status {
                    if s.success() && det.exists() && rec.exists() && keys.exists() {
                        println!("    \x1b[32m[OK]\x1b[0m Giải nén thành công bộ mô hình PP-OCRv6!");
                        unzipped = true;
                        break;
                    }
                }
            }
        }

        if !unzipped && (!det.exists() || !rec.exists() || !keys.exists()) {
            bail!(
                "Thiếu bộ mô hình PP-OCRv6 trong {:?}. Vui lòng đảm bảo các file 'det.onnx', 'rec.onnx', 'keys.txt' tồn tại trong 'models/ocr/' (hoặc cung cấp file 'models/ocr.zip').",
                ocr
            );
        }
    }
    Ok(())
}

/// Kiểm tra xem file VTT có hợp lệ và có dữ liệu không
fn is_valid_vtt(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    if let Ok(meta) = fs::metadata(path) {
        if meta.len() < 20 {
            return false;
        }
    }
    fs::read_to_string(path)
        .map(|s| s.contains("-->"))
        .unwrap_or(false)
}

/// Xử lý trọn gói 1 tập video (Subtitles -> Captions -> Fast Proofread -> Merge)
fn process_video(
    video_path: &Path,
    model_dir: &Path,
    sub_config: &OcrExtractorConfig,
    cap_config: &CaptionExtractorConfig,
) -> Result<()> {
    let file_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let out_dir = Path::new("output");
    fs::create_dir_all(out_dir)?;

    let sub_vtt = out_dir.join(format!("{}.subtitles.zh.vtt", file_stem));
    let cap_vtt = out_dir.join(format!("{}.captions.zh.vtt", file_stem));
    let refined_vtt = out_dir.join(format!("{}.refined.zh.vtt", file_stem));
    let merged_vtt = out_dir.join(format!("{}.merged.zh.vtt", file_stem));

    println!("\n================================================================================");
    println!(">>> [PIPELINE] Bắt đầu xử lý: \x1b[1m{}\x1b[0m", file_stem);
    println!("    Video đầu vào : {:?}", video_path);
    println!("    Merged output : {:?}", merged_vtt);
    println!("================================================================================");

    let total_start = Instant::now();

    // -------------------------------------------------------------------------
    // BƯỚC 1: Trích xuất phụ đề lời thoại (Subtitles)
    // -------------------------------------------------------------------------
    println!("\n>>> [1/4] Trích xuất Subtitles lời thoại...");
    if is_valid_vtt(&sub_vtt) {
        println!("    \x1b[33m[SKIP]\x1b[0m Đã có file phụ đề hợp lệ: {:?}", sub_vtt);
    } else {
        let t0 = Instant::now();
        let sub_out = extract_hardcoded_subtitles(video_path, &sub_vtt, model_dir, sub_config)
            .with_context(|| format!("Lỗi trích xuất Subtitles từ {:?}", video_path))?;
        println!(
            "    \x1b[32m[DONE]\x1b[0m Trích xuất xong {} câu lời thoại ({:.1}s)",
            sub_out.segments.len(),
            t0.elapsed().as_secs_f32()
        );
    }

    // -------------------------------------------------------------------------
    // BƯỚC 2: Trích xuất chú thích nhân vật/công pháp (Captions)
    // -------------------------------------------------------------------------
    println!("\n>>> [2/4] Trích xuất Captions chú thích...");
    if is_valid_vtt(&cap_vtt) {
        println!("    \x1b[33m[SKIP]\x1b[0m Đã có file chú thích hợp lệ: {:?}", cap_vtt);
    } else {
        let t0 = Instant::now();
        let cap_out = extract_captions(video_path, &cap_vtt, model_dir, cap_config)
            .with_context(|| format!("Lỗi trích xuất Captions từ {:?}", video_path))?;
        println!(
            "    \x1b[32m[DONE]\x1b[0m Trích xuất xong {} chú thích ({:.1}s)",
            cap_out.segments.len(),
            t0.elapsed().as_secs_f32()
        );
    }

    // -------------------------------------------------------------------------
    // BƯỚC 3: Hậu xử lý & Hiệu đính siêu tốc (Fast Mode: Entity Glossary, KHÔNG DÙNG LLM)
    // -------------------------------------------------------------------------
    println!("\n>>> [3/4] Hiệu đính phụ đề (Fast Mode - Không LLM, 0.01s)...");
    let glossary = if cap_vtt.exists() {
        EntityGlossary::from_caption_files(&[cap_vtt.clone()]).unwrap_or_default()
    } else {
        EntityGlossary::new()
    };

    println!(
        "    [GLOSSARY] Tự động học {} thực thể chuẩn từ Caption của tập này",
        glossary.terms().len()
    );

    let proofreader = Proofreader::with_fast_mode();
    let proofread_summary =
        proofreader.proofread_vtt_file(&sub_vtt, &refined_vtt, Some(&glossary))?;

    if proofread_summary.corrected_lines > 0 {
        println!(
            "    \x1b[32m[DONE]\x1b[0m Đã chuẩn hóa {}/{} câu phụ đề nghi vấn",
            proofread_summary.corrected_lines, proofread_summary.total_lines
        );
        for (old, new) in proofread_summary.corrections.iter().take(5) {
            println!("      [-] \"{}\" -> [+] \"{}\"", old, new);
        }
    } else {
        println!(
            "    \x1b[32m[DONE]\x1b[0m Toàn bộ {} câu phụ đề đều chuẩn xác!",
            proofread_summary.total_lines
        );
    }

    // -------------------------------------------------------------------------
    // BƯỚC 4: Hợp nhất (Merge) Subtitles + Captions thành 1 file WebVTT
    // -------------------------------------------------------------------------
    println!("\n>>> [4/4] Hợp nhất (Merge) Subtitles & Captions...");
    let merge_summary = merge_vtt_files(&refined_vtt, &cap_vtt, &merged_vtt)?;

    println!(
        "    \x1b[32m[THÀNH CÔNG]\x1b[0m File hoàn chỉnh: \x1b[1m{:?}\x1b[0m",
        merged_vtt
    );
    println!(
        "    [THỐNG KÊ] Tổng cộng: {} cues ({} lời thoại + {} chú thích)",
        merge_summary.total_merged_cues,
        merge_summary.subtitle_cues,
        merge_summary.caption_cues
    );
    println!("    [THỜI GIAN] Hoàn tất tập trong: {:.2}s", total_start.elapsed().as_secs_f32());

    Ok(())
}

fn main() -> Result<()> {
    println!("================================================================================");
    println!("  OCR PIPELINE TRỌN GÓI: SUBTITLES + CAPTIONS + FAST PROOFREAD + MERGE");
    println!("  (Chế độ Pure CPU, Không LLM, Tốc độ tối đa, Tương thích 100% mọi dự án)");
    println!("================================================================================\n");

    let model_dir = Path::new("models");
    ensure_models(model_dir)?;

    let sub_config = OcrExtractorConfig::default();
    let cap_config = CaptionExtractorConfig::default();

    // Lắng nghe thanh tiến trình khung hình
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
            p.title, bar_visual, ratio * 100.0, current, total, p.unit, p.elapsed
        );
        let _ = io::stdout().flush();
    });

    let args: Vec<String> = env::args().collect();
    let target = args.get(1).map(|s| s.as_str()).unwrap_or("all");

    // Xác định danh sách video cần xử lý
    let mut videos: Vec<PathBuf> = Vec::new();

    if target == "all" {
        // Quét toàn bộ input/ hoặc video_input/
        for dir in &["input", "video_input"] {
            let p = Path::new(dir);
            if p.exists() {
                if let Ok(entries) = fs::read_dir(p) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                            if ["mp4", "mkv", "avi", "mov", "ts", "flv"]
                                .contains(&ext.to_lowercase().as_str())
                            {
                                if !videos.iter().any(|v| v.file_name() == path.file_name()) {
                                    videos.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
        videos.sort();
    } else {
        // Một tập chỉ định: có thể là số (002, 2) hoặc tên file hoặc đường dẫn
        let path = PathBuf::from(target);
        if path.exists() {
            videos.push(path);
        } else {
            let ep_str = if let Ok(n) = target.parse::<usize>() {
                format!("{:03}", n)
            } else {
                target.to_string()
            };

            let candidates = [
                PathBuf::from(format!("input/第{}集.mp4", ep_str)),
                PathBuf::from(format!("video_input/第{}集.mp4", ep_str)),
                PathBuf::from(format!("input/{}.mp4", ep_str)),
            ];

            if let Some(found) = candidates.iter().find(|p| p.exists()) {
                videos.push(found.clone());
            } else {
                bail!(
                    "Không tìm thấy video ứng với tham số '{}'. Vui lòng kiểm tra lại đường dẫn.",
                    target
                );
            }
        }
    }

    if videos.is_empty() {
        println!(">>> [THÔNG BÁO] Không tìm thấy video nào để xử lý trong 'input/' hoặc 'video_input/'.");
        return Ok(());
    }

    println!(">>> Tìm thấy {} video cần xử lý:\n", videos.len());
    for (i, v) in videos.iter().enumerate() {
        println!("  {:2}. {:?}", i + 1, v.file_name().unwrap_or_default());
    }

    let pipeline_start = Instant::now();

    for video in &videos {
        if let Err(e) = process_video(video, model_dir, &sub_config, &cap_config) {
            eprintln!("\n\x1b[31m[ERROR]\x1b[0m Lỗi khi xử lý video {:?}: {:?}", video, e);
        }
    }

    println!("\n================================================================================");
    println!(
        ">>> [TOÀN TẤT PIPELINE] Đã xử lý xong {}/{} video trong {:.2}s!",
        videos.len(),
        videos.len(),
        pipeline_start.elapsed().as_secs_f32()
    );
    println!(">>> Các file kết quả đã sẵn sàng tại thư mục: output/*.merged.zh.vtt");
    println!("================================================================================");

    Ok(())
}
