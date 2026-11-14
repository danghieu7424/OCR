//! Example: OCR-only pipeline (extract subtitles + captions, then merge)
//!
//! 1. Trích xuất VTT subtitle và caption thô (song song theo mặc định).
//! 2. Giữ nguyên nội dung OCR trong hai file VTT riêng.
//! 3. Hợp nhất trực tiếp hai VTT, giữ nguyên cue settings.
//!
//! Cách chạy:
//! - Cho một tập chỉ định (mặc định song song):
//!     cargo run --release --example extract_and_merge -- 002
//!     cargo run --release --example extract_and_merge -- input/第002集.mp4
//! - Xuất ảnh bằng chứng cho caption:
//!     cargo run --release --example extract_and_merge -- 001 --caption-evidence
//! - Chạy tuần tự (nếu muốn tiết kiệm CPU):
//!     cargo run --release --example extract_and_merge -- 002 --sequential
//! - Cho toàn bộ video trong input/:
//!     cargo run --release --example extract_and_merge -- all
//!     cargo run --release --example extract_and_merge

use anyhow::{Context, Result, bail};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use OCR::{
    CaptionExtractorConfig, OcrExtractorConfig, extract_captions, extract_hardcoded_subtitles,
    merge_vtt_files,
};

/// Tải file mô hình PP-OCRv6 từ URL
fn download_file(url: &str, output: &Path) -> Result<()> {
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    println!("    [DOWNLOAD] {}", url);

    let status = Command::new("curl.exe")
        .args(&["-L", "--progress-bar", "-o", &output.to_string_lossy(), url])
        .status();

    match status {
        Ok(s) if s.success() && output.exists() => {
            println!(
                "    \x1b[32m[OK]\x1b[0m {:?}",
                output.file_name().unwrap_or_default()
            );
            Ok(())
        }
        _ => {
            println!("    [FALLBACK] PowerShell...");
            let ps = format!(
                "Invoke-WebRequest -Uri '{}' -OutFile '{}'",
                url,
                output.to_string_lossy()
            );
            Command::new("powershell")
                .args(&["-Command", &ps])
                .status()?;
            if output.exists() {
                Ok(())
            } else {
                bail!("Tải thất bại: {}", url);
            }
        }
    }
}

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
                println!(
                    ">>> [MODEL CHECK] Đang giải nén bộ mô hình PP-OCRv6 từ {:?}...",
                    zip
                );
                let _ = fs::create_dir_all(model_dir);
                let status = Command::new("tar")
                    .args(&[
                        "-xf",
                        &zip.to_string_lossy(),
                        "-C",
                        &model_dir.to_string_lossy(),
                    ])
                    .status();
                if let Ok(s) = status {
                    if s.success() && det.exists() && rec.exists() && keys.exists() {
                        println!(
                            "    \x1b[32m[OK]\x1b[0m Giải nén thành công bộ mô hình PP-OCRv6!"
                        );
                        unzipped = true;
                        break;
                    }
                }
            }
        }

        if !unzipped && (!det.exists() || !rec.exists() || !keys.exists()) {
            println!(">>> [MODEL CHECK] Tự động tải bộ mô hình PP-OCRv6 (Medium)...");
            fs::create_dir_all(&ocr)?;

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
            println!(">>> [MODEL CHECK] Tất cả các mô hình PP-OCRv6 đã sẵn sàng!\n");
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

fn log_caption_review(output: &OCR::CaptionExtractorOutput) {
    if let Some(dir) = &output.evidence_dir {
        println!("    [CAP: EVIDENCE] Ảnh kiểm chứng: {:?}", dir);
    }
    for warning in &output.warnings {
        println!(
            "    [CAP: REVIEW] {} | {:.2}s | confidence={:.3} | {}",
            warning.text,
            warning.duration_sec,
            warning.confidence,
            warning.reasons.join(", ")
        );
    }
}

/// Xử lý một video bằng OCR thô, sau đó hợp nhất hai VTT.
fn process_video(
    video_path: &Path,
    model_dir: &Path,
    sub_config: &OcrExtractorConfig,
    cap_config: &CaptionExtractorConfig,
    parallel: bool,
    caption_evidence: bool,
) -> Result<()> {
    let file_stem = video_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let out_dir = Path::new("output");
    fs::create_dir_all(out_dir)?;

    let sub_vtt = out_dir.join(format!("{}.subtitles.zh.vtt", file_stem));
    let cap_vtt = out_dir.join(format!("{}.captions.zh.vtt", file_stem));
    let merged_vtt = out_dir.join(format!("{}.merged.zh.vtt", file_stem));
    let mut cap_config = cap_config.clone();
    if caption_evidence {
        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        cap_config.evidence_dir = Some(
            out_dir
                .join("caption_evidence")
                .join(file_stem)
                .join(format!("run-{}", run_id)),
        );
    }

    println!("\n================================================================================");
    println!(
        ">>> [PIPELINE] Bắt đầu xử lý: \x1b[1m{}\x1b[0m (Chế độ: {})",
        file_stem,
        if parallel {
            "\x1b[32mSong song (Parallel)\x1b[0m"
        } else {
            "\x1b[33mTuần tự (Sequential)\x1b[0m"
        }
    );
    println!("    Video đầu vào : {:?}", video_path);
    println!("    Merged output : {:?}", merged_vtt);
    println!("================================================================================");

    let total_start = Instant::now();

    // -------------------------------------------------------------------------
    // BƯỚC 1: Trích xuất VTT thô cho Subtitles và Captions.
    // -------------------------------------------------------------------------
    if parallel {
        println!(
            "\n>>> [1/2] Đang trích xuất SONG SONG: Subtitles (lời thoại) + Captions (chú thích)..."
        );
        let t_par = Instant::now();

        let (sub_res, cap_res) = thread::scope(|s| {
            let sub_handle = s.spawn(|| {
                if is_valid_vtt(&sub_vtt) {
                    println!("\n    \x1b[33m[SUB: SKIP]\x1b[0m Đã có file phụ đề hợp lệ: {:?}", sub_vtt);
                    Ok(())
                } else {
                    let t0 = Instant::now();
                    println!("\n    \x1b[36m[SUB: START]\x1b[0m Bắt đầu trích xuất Subtitles lời thoại...");
                    let sub_out = extract_hardcoded_subtitles(video_path, &sub_vtt, model_dir, sub_config)
                        .with_context(|| format!("Lỗi trích xuất Subtitles từ {:?}", video_path))?;
                    println!(
                        "\n    \x1b[32m[SUB: DONE]\x1b[0m Trích xuất xong {} câu lời thoại ({:.1}s)",
                        sub_out.segments.len(),
                        t0.elapsed().as_secs_f32()
                    );
                    Ok(())
                }
            });

            let cap_handle = s.spawn(|| {
                if is_valid_vtt(&cap_vtt) && !caption_evidence {
                    println!(
                        "\n    \x1b[33m[CAP: SKIP]\x1b[0m Đã có file chú thích hợp lệ: {:?}",
                        cap_vtt
                    );
                    Ok(())
                } else {
                    let t0 = Instant::now();
                    println!(
                        "\n    \x1b[35m[CAP: START]\x1b[0m Bắt đầu trích xuất Captions chú thích..."
                    );
                    let cap_out = extract_captions(video_path, &cap_vtt, model_dir, &cap_config)
                        .with_context(|| format!("Lỗi trích xuất Captions từ {:?}", video_path))?;
                    log_caption_review(&cap_out);
                    println!(
                        "\n    \x1b[32m[CAP: DONE]\x1b[0m Trích xuất xong {} chú thích ({:.1}s)",
                        cap_out.segments.len(),
                        t0.elapsed().as_secs_f32()
                    );
                    Ok(())
                }
            });

            let r1 = sub_handle
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Subtitles thread panic")));
            let r2 = cap_handle
                .join()
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Captions thread panic")));
            (r1, r2)
        });

        sub_res?;
        cap_res?;

        println!(
            "\n    \x1b[32m[PARALLEL SUCCESS]\x1b[0m Cả 2 luồng hoàn tất trong {:.1}s!",
            t_par.elapsed().as_secs_f32()
        );
    } else {
        println!("\n>>> [1/2] Trích xuất Subtitles lời thoại...");
        if is_valid_vtt(&sub_vtt) {
            println!(
                "    \x1b[33m[SKIP]\x1b[0m Đã có file phụ đề hợp lệ: {:?}",
                sub_vtt
            );
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

        println!("\n>>> [2/2] Trích xuất Captions chú thích...");
        if is_valid_vtt(&cap_vtt) && !caption_evidence {
            println!(
                "    \x1b[33m[SKIP]\x1b[0m Đã có file chú thích hợp lệ: {:?}",
                cap_vtt
            );
        } else {
            let t0 = Instant::now();
            let cap_out = extract_captions(video_path, &cap_vtt, model_dir, &cap_config)
                .with_context(|| format!("Lỗi trích xuất Captions từ {:?}", video_path))?;
            log_caption_review(&cap_out);
            println!(
                "    \x1b[32m[DONE]\x1b[0m Trích xuất xong {} chú thích ({:.1}s)",
                cap_out.segments.len(),
                t0.elapsed().as_secs_f32()
            );
        }
    }

    // -------------------------------------------------------------------------
    // BƯỚC 3: Hợp nhất trực tiếp hai VTT OCR thô.
    // -------------------------------------------------------------------------
    println!("\n>>> [3/3] Hợp nhất (Merge) hai VTT OCR thô...");
    let merge_summary = merge_vtt_files(&sub_vtt, &cap_vtt, &merged_vtt)?;

    println!(
        "    \x1b[32m[THÀNH CÔNG]\x1b[0m File hoàn chỉnh: \x1b[1m{:?}\x1b[0m",
        merged_vtt
    );
    println!(
        "    [THỐNG KÊ] Tổng cộng: {} cues ({} lời thoại + {} chú thích)",
        merge_summary.total_merged_cues, merge_summary.subtitle_cues, merge_summary.caption_cues
    );
    println!(
        "    [THỜI GIAN] Hoàn tất tập trong: {:.2}s",
        total_start.elapsed().as_secs_f32()
    );

    Ok(())
}

fn main() -> Result<()> {
    println!("================================================================================");
    println!("  OCR PIPELINE: RAW SUBTITLES + RAW CAPTIONS + MERGE");
    println!("  (Không hiệu đính hoặc sửa nội dung OCR)");
    println!("================================================================================\n");

    let model_dir = Path::new("models");
    ensure_models(model_dir)?;

    let sub_config = OcrExtractorConfig::default();
    let cap_config = CaptionExtractorConfig::default();

    // Lắng nghe thanh tiến trình khung hình thời gian thực (hỗ trợ cả song song và tuần tự)
    let progress_state = Arc::new(Mutex::new((0.0f32, 0.0f32, String::new())));
    let state_clone = Arc::clone(&progress_state);
    let (filled_char, empty_char) = cli_gui_form::Icons::PROGRESS_CHARS_PARALLELOGRAM;
    let bar_width = 12usize;

    OCR::on_progress(move |p| {
        let total = p.total.max(1);
        let current = p.current.min(total);
        let ratio = (current as f32 / total as f32).clamp(0.0, 1.0);
        let pct = ratio * 100.0;

        if let Ok(mut st) = state_clone.lock() {
            if p.title.contains("Caption") {
                st.1 = pct;
            } else {
                st.0 = pct;
            }
            st.2 = p.elapsed.clone();

            let sub_filled = ((st.0 / 100.0 * bar_width as f32).round() as usize).min(bar_width);
            let sub_empty = bar_width.saturating_sub(sub_filled);
            let sub_bar = filled_char.repeat(sub_filled) + &empty_char.repeat(sub_empty);

            let cap_filled = ((st.1 / 100.0 * bar_width as f32).round() as usize).min(bar_width);
            let cap_empty = bar_width.saturating_sub(cap_filled);
            let cap_bar = filled_char.repeat(cap_filled) + &empty_char.repeat(cap_empty);

            print!(
                "\r \x1b[36m▶\x1b[0m \x1b[1m[SUB]\x1b[0m \x1b[32m[{}]\x1b[0m \x1b[33m{:>5.1}%\x1b[0m │ \x1b[1m[CAP]\x1b[0m \x1b[35m[{}]\x1b[0m \x1b[33m{:>5.1}%\x1b[0m \x1b[90m[{}]\x1b[0m   ",
                sub_bar, st.0, cap_bar, st.1, st.2
            );
            let _ = io::stdout().flush();
        }
    });

    let args: Vec<String> = env::args().collect();
    let parallel = !args
        .iter()
        .any(|a| a == "--sequential" || a == "--no-parallel");
    let caption_evidence = args.iter().any(|a| a == "--caption-evidence");
    let target = args
        .iter()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .map(|s| s.as_str())
        .unwrap_or("all");

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
        println!(
            ">>> [THÔNG BÁO] Không tìm thấy video nào để xử lý trong 'input/' hoặc 'video_input/'."
        );
        return Ok(());
    }

    println!(">>> Tìm thấy {} video cần xử lý:\n", videos.len());
    for (i, v) in videos.iter().enumerate() {
        println!("  {:2}. {:?}", i + 1, v.file_name().unwrap_or_default());
    }

    let pipeline_start = Instant::now();

    let mut completed = 0usize;
    let mut failed = 0usize;
    for video in &videos {
        match process_video(
            video,
            model_dir,
            &sub_config,
            &cap_config,
            parallel,
            caption_evidence,
        ) {
            Ok(()) => completed += 1,
            Err(e) => {
                failed += 1;
                eprintln!(
                    "\n\x1b[31m[ERROR]\x1b[0m Lỗi khi xử lý video {:?}: {:?}",
                    video, e
                );
            }
        }
    }

    println!("\n================================================================================");
    println!(
        ">>> [PIPELINE] Hoàn thành {}/{} video, lỗi {} trong {:.2}s!",
        completed,
        videos.len(),
        failed,
        pipeline_start.elapsed().as_secs_f32()
    );
    println!(">>> Các file kết quả đã sẵn sàng tại thư mục: output/*.merged.zh.vtt");
    println!("================================================================================");
    if failed > 0 {
        bail!("Có {} video xử lý thất bại.", failed);
    }

    Ok(())
}
