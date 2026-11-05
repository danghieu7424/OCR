/****
 * [MODULE]: OCR (Video Subtitle Optical Character Recognition)
 * Giữ nguyên 100% cấu trúc Public API, Struct và các hàm để tương thích hoàn toàn với tool chính.
 ****/

use anyhow::{bail, Context, Result};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ort::{
    session::builder::GraphOptimizationLevel,
    session::Session,
    value::Tensor,
};

use crate::shared::terminal_color::TermColorExt;
use cli_gui_form::Icons;

pub mod opencv_filter;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubtitleRoi {
    pub y_min: f32,
    pub y_max: f32,
    pub x_min: f32,
    pub x_max: f32,
}

impl Default for SubtitleRoi {
    fn default() -> Self {
        Self {
            y_min: 0.80,
            y_max: 1.00,
            x_min: 0.02,
            x_max: 0.98,
        }
    }
}

impl SubtitleRoi {
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
        if parts.len() != 4 {
            return None;
        }
        Some(Self {
            y_min: parts[0].parse().ok()?,
            y_max: parts[1].parse().ok()?,
            x_min: parts[2].parse().ok()?,
            x_max: parts[3].parse().ok()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OcrExtractorConfig {
    pub fps: u32,
    pub roi: SubtitleRoi,
    pub lang: String,
    pub diff_threshold: f32,
}

impl Default for OcrExtractorConfig {
    fn default() -> Self {
        Self {
            fps: 15, // Tối ưu tần số mẫu 15fps giúp cân bằng tốc độ và độ chính xác thời gian
            roi: SubtitleRoi::default(),
            lang: "zh".to_string(),
            diff_threshold: 0.03,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleSegment {
    pub start_sec: f64,
    pub end_sec: f64,
    pub text: String,
}

impl SubtitleSegment {
    pub fn format_vtt_timestamp(seconds: f64) -> String {
        let total_ms = (seconds * 1000.0).round() as u64;
        let ms = total_ms % 1000;
        let total_sec = total_ms / 1000;
        let sec = total_sec % 60;
        let total_min = total_sec / 60;
        let min = total_min % 60;
        let hours = total_min / 60;
        format!("{:02}:{:02}:{:02}.{:03}", hours, min, sec, ms)
    }
}

#[derive(Debug, Clone)]
pub struct OcrExtractorOutput {
    pub vtt_path: PathBuf,
    pub segments: Vec<SubtitleSegment>,
}

#[derive(Default, Clone)]
pub struct OcrFrameRecognition {
    pub text: String,
    pub raw_patch: Vec<u8>,
    pub clahe_patch: Vec<u8>,
    pub width: usize,
    pub height: usize,
}

fn save_rgb_as_bmp(path: &Path, rgb: &[u8], width: usize, height: usize) -> std::io::Result<()> {
    use std::io::Write;
    if width == 0 || height == 0 || rgb.is_empty() {
        return Ok(());
    }
    let row_stride = (width * 3 + 3) & !3;
    let image_size = row_stride * height;
    let file_size = 54 + image_size;

    let mut header = [0u8; 54];
    header[0] = b'B';
    header[1] = b'M';
    header[2..6].copy_from_slice(&(file_size as u32).to_le_bytes());
    header[10..14].copy_from_slice(&54u32.to_le_bytes());
    header[14..18].copy_from_slice(&40u32.to_le_bytes());
    header[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    header[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    header[26..28].copy_from_slice(&1u16.to_le_bytes());
    header[28..30].copy_from_slice(&24u16.to_le_bytes());
    header[34..38].copy_from_slice(&(image_size as u32).to_le_bytes());

    let mut file = std::fs::File::create(path)?;
    file.write_all(&header)?;

    let mut row_buf = vec![0u8; row_stride];
    for y in 0..height {
        let src_row = &rgb[y * width * 3..(y + 1) * width * 3];
        for x in 0..width {
            let r = src_row[x * 3];
            let g = src_row[x * 3 + 1];
            let b = src_row[x * 3 + 2];
            row_buf[x * 3] = b;
            row_buf[x * 3 + 1] = g;
            row_buf[x * 3 + 2] = r;
        }
        file.write_all(&row_buf)?;
    }
    Ok(())
}

struct PpOcrEngine {
    rec_session: Session,
    keys: Vec<String>,
}

impl PpOcrEngine {
    fn new(model_dir: &Path) -> Result<Self> {
        let ocr_dir = model_dir.join("ocr");
        let rec_model_path = ocr_dir.join("rec.onnx");
        let keys_path = ocr_dir.join("keys.txt");

        if !rec_model_path.exists() || !keys_path.exists() {
            bail!("Thiếu tài nguyên mô hình OCR tại {:?}", ocr_dir);
        }

        let keys_content = fs::read_to_string(&keys_path)?;
        let mut keys: Vec<String> = keys_content.lines().map(|l| l.to_string()).collect();
        keys.insert(0, "<blank>".to_string());
        keys.push(" ".to_string());

        let threads = crate::shared::CpuOptimizer::single_task_threads();
        let rec_session = Session::builder()?
            .with_optimization_level(GraphOptimizationLevel::Level3)?
            .with_intra_threads(threads)?
            .commit_from_file(&rec_model_path)?;

        Ok(Self { rec_session, keys })
    }

    fn recognize_text(&mut self, rgb_bytes: &[u8], width: usize, height: usize) -> Result<OcrFrameRecognition> {
        if width == 0 || height == 0 || rgb_bytes.is_empty() {
            return Ok(OcrFrameRecognition::default());
        }

        let (raw_patch, clahe_patch, target_w, target_h) = 
            match self::opencv_filter::extract_and_filter_patch(rgb_bytes, width, height)? {
                Some(patch) => patch,
                None => return Ok(OcrFrameRecognition::default()),
            };

        let mut input_tensor = vec![0.0f32; 3 * target_h * target_w];
        for y in 0..target_h {
            for x in 0..target_w {
                let idx = (y * target_w + x) * 3;
                input_tensor[0 * target_h * target_w + y * target_w + x] = (clahe_patch[idx] as f32 / 255.0 - 0.5) / 0.5;
                input_tensor[1 * target_h * target_w + y * target_w + x] = (clahe_patch[idx + 1] as f32 / 255.0 - 0.5) / 0.5;
                input_tensor[2 * target_h * target_w + y * target_w + x] = (clahe_patch[idx + 2] as f32 / 255.0 - 0.5) / 0.5;
            }
        }

        let tensor_val = Tensor::from_array(([1usize, 3usize, target_h, target_w], input_tensor))?;
        let outputs = self.rec_session.run(ort::inputs![tensor_val])?;
        let (shape, logits) = outputs[0].try_extract_tensor::<f32>()?;

        let time_steps = shape[1] as usize;
        let num_classes = shape[2] as usize;

        let mut decoded_text = String::new();
        let mut last_idx = 0usize;

        for t in 0..time_steps {
            let step_offset = t * num_classes;
            let mut max_idx = 0usize;
            let mut max_val = f32::NEG_INFINITY;

            for c in 0..num_classes {
                let val = logits[step_offset + c];
                if val > max_val {
                    max_val = val;
                    max_idx = c;
                }
            }

            if max_idx != 0 && max_idx != last_idx {
                if max_idx < self.keys.len() {
                    let ch = &self.keys[max_idx];
                    if ch != "<blank>" {
                        decoded_text.push_str(ch);
                    }
                }
            }
            last_idx = max_idx;
        }

        let cleaned = sanitize_ocr_text(&decoded_text);

        Ok(OcrFrameRecognition {
            text: cleaned,
            raw_patch,
            clahe_patch,
            width: target_w,
            height: target_h,
        })
    }
}

fn sanitize_ocr_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut chars: Vec<char> = trimmed.chars().collect();
    if !chars.is_empty() && (chars[0] == '-' || chars[0] == '—' || chars[0] == '–') {
        chars.remove(0);
    }
    let s: String = chars.into_iter().collect();
    s.trim_matches(|c: char| {
        c.is_whitespace() || c == '-' || c == '—' || c == '–' || c == '_' || c == '<' || c == '>'
            || c == '《' || c == '》' || c == '【' || c == '】' || c == '“' || c == '”' || c == '"'
            || c == '\'' || c == '#' || c == '*' || c == '+' || c == '=' || c == '|' || c == '~'
            || c == ',' || c == '，' || c == '.' || c == '。' || c == '!' || c == '！' || c == '?'
            || c == '？' || c == ':' || c == '：' || c == ';' || c == '；'
    }).trim().to_string()
}

fn text_similarity(s1: &str, s2: &str) -> f32 {
    let chars1: Vec<char> = s1.chars().collect();
    let chars2: Vec<char> = s2.chars().collect();
    if chars1.is_empty() || chars2.is_empty() { return 0.0; }
    if s1 == s2 { return 1.0; }

    let min_len = chars1.len().min(chars2.len());
    let max_len = chars1.len().max(chars2.len());
    if (max_len - min_len) as f32 / max_len as f32 > 0.5 { return 0.0; }

    let mut matches = 0usize;
    let mut c2_used = vec![false; chars2.len()];
    for c1 in &chars1 {
        for (j, c2) in chars2.iter().enumerate() {
            if !c2_used[j] && c1 == c2 {
                c2_used[j] = true;
                matches += 1;
                break;
            }
        }
    }
    (2.0 * matches as f32) / (chars1.len() + chars2.len()) as f32
}

pub fn extract_hardcoded_subtitles(
    video_path: &Path,
    output_vtt_path: &Path,
    model_dir: &Path,
    config: &OcrExtractorConfig,
) -> Result<OcrExtractorOutput> {
    if !video_path.exists() {
        bail!("Video không tồn tại: {:?}", video_path);
    }

    crate::shared::tui_events::emit_step(">>> [OCR] Trích xuất Hardcoded Subtitles (VSE Optimized Engine)...");
    let mut ocr_engine = PpOcrEngine::new(model_dir)?;

    let probe_out = Command::new("ffprobe")
        .args(&[
            "-v", "error", "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0", &video_path.to_string_lossy(),
        ])
        .output()?;

    let dims: Vec<usize> = String::from_utf8_lossy(&probe_out.stdout)
        .trim().split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let (src_w, src_h) = if dims.len() == 2 { (dims[0], dims[1]) } else { (1280, 720) };

    let crop_w = (((src_w as f32 * (config.roi.x_max - config.roi.x_min)).round() as usize) / 2) * 2;
    let crop_h = (((src_h as f32 * (config.roi.y_max - config.roi.y_min)).round() as usize) / 2) * 2;
    let crop_x = (src_w as f32 * config.roi.x_min).round() as usize;
    let crop_y = (src_h as f32 * config.roi.y_min).round() as usize;

    // Sử dụng bộ lọc video FFmpeg kết hợp trích xuất PTS chính xác theo thời gian thực (tránh lệch time)
    let vf_filter = format!("fps={},crop={}:{}:{}:{}", config.fps, crop_w, crop_h, crop_x, crop_y);

    let mut ffmpeg_child = Command::new("ffmpeg")
        .args(&[
            "-i", &video_path.to_string_lossy(),
            "-vf", &vf_filter,
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-v", "error",
            "-",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stdout = ffmpeg_child.stdout.take().context("Không thể mở FFmpeg pipe")?;
    let frame_bytes = crop_w * crop_h * 3;
    let mut current_frame = vec![0u8; frame_bytes];
    let dt = 1.0 / (config.fps as f64);
    let mut frame_idx = 0usize;

    let mut segments: Vec<SubtitleSegment> = Vec::new();
    let frames_debug_dir = output_vtt_path.parent().unwrap().join("ocr_frames");
    let _ = fs::create_dir_all(&frames_debug_dir);

    struct ActiveCue {
        text: String,
        start_sec: f64,
        end_sec: f64,
        raw_patch: Vec<u8>,
        clahe_patch: Vec<u8>,
        w: usize,
        h: usize,
    }

    let mut active_cue: Option<ActiveCue> = None;

    while stdout.read_exact(&mut current_frame).is_ok() {
        let timestamp = frame_idx as f64 * dt;
        frame_idx += 1;

        let rec = ocr_engine.recognize_text(&current_frame, crop_w, crop_h).unwrap_or_default();
        let recognized = rec.text;

        if recognized.is_empty() {
            if let Some(cue) = active_cue.take() {
                if cue.end_sec - cue.start_sec >= 0.4 {
                    let idx = segments.len() + 1;
                    let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_raw.bmp", idx)), &cue.raw_patch, cue.w, cue.h);
                    let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_clahe.bmp", idx)), &cue.clahe_patch, cue.w, cue.h);
                    segments.push(SubtitleSegment {
                        start_sec: cue.start_sec,
                        end_sec: cue.end_sec,
                        text: cue.text,
                    });
                }
            }
        } else {
            match active_cue.as_mut() {
                Some(cue) => {
                    let sim = text_similarity(&cue.text, &recognized);
                    if sim >= 0.55 {
                        cue.end_sec = timestamp;
                        if recognized.chars().count() > cue.text.chars().count() {
                            cue.text = recognized;
                            if !rec.raw_patch.is_empty() {
                                cue.raw_patch = rec.raw_patch;
                                cue.clahe_patch = rec.clahe_patch;
                                cue.w = rec.width;
                                cue.h = rec.height;
                            }
                        }
                    } else {
                        // Lưu cue cũ, mở cue mới
                        let finished_cue = active_cue.take().unwrap();
                        if finished_cue.end_sec - finished_cue.start_sec >= 0.4 {
                            let idx = segments.len() + 1;
                            let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_raw.bmp", idx)), &finished_cue.raw_patch, finished_cue.w, finished_cue.h);
                            let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_clahe.bmp", idx)), &finished_cue.clahe_patch, finished_cue.w, finished_cue.h);
                            segments.push(SubtitleSegment {
                                start_sec: finished_cue.start_sec,
                                end_sec: finished_cue.end_sec,
                                text: finished_cue.text,
                            });
                        }
                        active_cue = Some(ActiveCue {
                            text: recognized,
                            start_sec: timestamp,
                            end_sec: timestamp,
                            raw_patch: rec.raw_patch,
                            clahe_patch: rec.clahe_patch,
                            w: rec.width,
                            h: rec.height,
                        });
                    }
                }
                None => {
                    active_cue = Some(ActiveCue {
                        text: recognized,
                        start_sec: timestamp,
                        end_sec: timestamp,
                        raw_patch: rec.raw_patch,
                        clahe_patch: rec.clahe_patch,
                        w: rec.width,
                        h: rec.height,
                    });
                }
            }
        }
    }

    let _ = ffmpeg_child.wait();

    if let Some(cue) = active_cue.take() {
        if cue.end_sec - cue.start_sec >= 0.4 {
            let idx = segments.len() + 1;
            let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_raw.bmp", idx)), &cue.raw_patch, cue.w, cue.h);
            let _ = save_rgb_as_bmp(&frames_debug_dir.join(format!("cue_{:03}_clahe.bmp", idx)), &cue.clahe_patch, cue.w, cue.h);
            segments.push(SubtitleSegment {
                start_sec: cue.start_sec,
                end_sec: cue.end_sec,
                text: cue.text,
            });
        }
    }

    // Ghi file WebVTT
    if let Some(parent) = output_vtt_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut vtt_content = String::from("WEBVTT\n\n");
    for seg in &segments {
        let start_str = SubtitleSegment::format_vtt_timestamp(seg.start_sec);
        let end_str = SubtitleSegment::format_vtt_timestamp(seg.end_sec);
        vtt_content.push_str(&format!("{} --> {}\n{}\n\n", start_str, end_str, seg.text));
    }

    fs::write(output_vtt_path, vtt_content)?;

    Ok(OcrExtractorOutput {
        vtt_path: output_vtt_path.to_path_buf(),
        segments,
    })
}