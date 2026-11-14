#![allow(non_snake_case)]
// crates/OCR_Merge/src/lib.rs
//! # OCR_Merge - Ghép Subtitles và Captions thành 1 file WebVTT đồng bộ
//!
//! Module chịu trách nhiệm:
//! - Đọc và phân tích cấu trúc cú pháp WebVTT (.vtt) của phụ đề lời thoại và chú thích nhân vật.
//! - Hợp nhất (merge) các cue phụ đề và chú thích theo trục thời gian tuyến tính (chronological sorting).
//! - Xuất file `.merged.zh.vtt` hoặc WebVTT hoàn chỉnh với chỉ mục tuần tự chuẩn W3C WebVTT.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Một khối phụ đề hoặc chú thích trong file WebVTT
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VttCue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub is_caption: bool,
    /// Thiết lập vị trí hiển thị chuẩn WebVTT cue settings (ví dụ: `line:20%`, `position:50%`, `align:start`,...)
    pub settings: Option<String>,
}

/// Thống kê kết quả hợp nhất
#[derive(Debug, Clone, Default)]
pub struct MergeSummary {
    pub subtitle_cues: usize,
    pub caption_cues: usize,
    pub total_merged_cues: usize,
}

/// Chuyển đổi chuỗi mốc thời gian WebVTT (`00:01:23.456` hoặc `01:23.456`) sang mili-giây
pub fn parse_vtt_timestamp(ts: &str) -> Option<u64> {
    let ts = ts.trim();
    let parts: Vec<&str> = ts.split(':').collect();
    match parts.len() {
        3 => {
            // HH:MM:SS.mmm
            let hours: u64 = parts[0].parse().ok()?;
            let minutes: u64 = parts[1].parse().ok()?;
            let sec_parts: Vec<&str> = parts[2].split('.').collect();
            let seconds: u64 = sec_parts.get(0)?.parse().ok()?;
            let millis: u64 = sec_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            Some(hours * 3600_000 + minutes * 60_000 + seconds * 1000 + millis)
        }
        2 => {
            // MM:SS.mmm
            let minutes: u64 = parts[0].parse().ok()?;
            let sec_parts: Vec<&str> = parts[1].split('.').collect();
            let seconds: u64 = sec_parts.get(0)?.parse().ok()?;
            let millis: u64 = sec_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            Some(minutes * 60_000 + seconds * 1000 + millis)
        }
        _ => None,
    }
}

/// Định dạng mili-giây sang chuẩn WebVTT `HH:MM:SS.mmm`
pub fn format_vtt_timestamp(ms: u64) -> String {
    let hours = ms / 3600_000;
    let rem = ms % 3600_000;
    let minutes = rem / 60_000;
    let rem = rem % 60_000;
    let seconds = rem / 1000;
    let millis = rem % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, millis)
}

/// Phân tích nội dung file WebVTT thành danh sách các `VttCue`
pub fn parse_vtt(content: &str, is_caption: bool) -> Vec<VttCue> {
    let mut cues = Vec::new();
    let mut lines = content.lines().map(|l| l.trim());

    while let Some(line) = lines.next() {
        if line.contains("-->") {
            let parts: Vec<&str> = line.split("-->").collect();
            if parts.len() >= 2 {
                let start_part = parts[0].split_whitespace().next().unwrap_or("");
                let after_arrow = parts[1].trim();

                let mut ws = after_arrow.split_whitespace();
                let end_part = ws.next().unwrap_or("");
                let remaining_settings: Vec<&str> = ws.collect();
                let settings = if !remaining_settings.is_empty() {
                    Some(remaining_settings.join(" "))
                } else if is_caption {
                    // Fallback mặc định cho caption nếu file nguồn chưa có thiết lập dòng
                    Some("line:20%".to_string())
                } else {
                    None
                };

                if let (Some(start_ms), Some(end_ms)) =
                    (parse_vtt_timestamp(start_part), parse_vtt_timestamp(end_part))
                {
                    let mut text_lines = Vec::new();
                    while let Some(text_line) = lines.next() {
                        if text_line.is_empty() {
                            break;
                        }
                        text_lines.push(text_line);
                    }

                    let text = text_lines.join("\n").trim().to_string();
                    if !text.is_empty() {
                        cues.push(VttCue {
                            start_ms,
                            end_ms,
                            text,
                            is_caption,
                            settings,
                        });
                    }
                }
            }
        }
    }

    cues
}

/// Hợp nhất danh sách các cue từ phụ đề và chú thích, sắp xếp theo thời gian tăng dần
pub fn merge_cues(subtitles: &[VttCue], captions: &[VttCue]) -> Vec<VttCue> {
    let mut all_cues = Vec::with_capacity(subtitles.len() + captions.len());
    all_cues.extend_from_slice(subtitles);
    all_cues.extend_from_slice(captions);

    // Sắp xếp theo mốc thời gian bắt đầu; nếu trùng thời gian thì ưu tiên caption lên trước
    all_cues.sort_by(|a, b| {
        a.start_ms
            .cmp(&b.start_ms)
            .then_with(|| b.is_caption.cmp(&a.is_caption))
            .then_with(|| a.end_ms.cmp(&b.end_ms))
    });

    all_cues
}

/// Xuất danh sách VttCue thành chuỗi định dạng chuẩn WebVTT
pub fn serialize_vtt(cues: &[VttCue]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for (idx, cue) in cues.iter().enumerate() {
        out.push_str(&format!("{}\n", idx + 1));
        if let Some(ref s) = cue.settings {
            if !s.is_empty() {
                out.push_str(&format!(
                    "{} --> {} {}\n",
                    format_vtt_timestamp(cue.start_ms),
                    format_vtt_timestamp(cue.end_ms),
                    s
                ));
            } else {
                out.push_str(&format!(
                    "{} --> {}\n",
                    format_vtt_timestamp(cue.start_ms),
                    format_vtt_timestamp(cue.end_ms)
                ));
            }
        } else {
            out.push_str(&format!(
                "{} --> {}\n",
                format_vtt_timestamp(cue.start_ms),
                format_vtt_timestamp(cue.end_ms)
            ));
        }
        out.push_str(&format!("{}\n\n", cue.text));
    }
    out
}

/// Ghép 2 file WebVTT (phụ đề + chú thích) thành 1 file WebVTT duy nhất
pub fn merge_vtt_files(
    subtitle_path: &Path,
    caption_path: &Path,
    output_path: &Path,
) -> Result<MergeSummary> {
    let sub_content = if subtitle_path.exists() {
        fs::read_to_string(subtitle_path)
            .with_context(|| format!("Không thể đọc file phụ đề {:?}", subtitle_path))?
    } else {
        String::new()
    };

    let cap_content = if caption_path.exists() {
        fs::read_to_string(caption_path)
            .with_context(|| format!("Không thể đọc file chú thích {:?}", caption_path))?
    } else {
        String::new()
    };

    let sub_cues = parse_vtt(&sub_content, false);
    let cap_cues = parse_vtt(&cap_content, true);

    let summary = MergeSummary {
        subtitle_cues: sub_cues.len(),
        caption_cues: cap_cues.len(),
        total_merged_cues: sub_cues.len() + cap_cues.len(),
    };

    let merged = merge_cues(&sub_cues, &cap_cues);
    let serialized = serialize_vtt(&merged);

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_path, serialized)?;

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_parsing_and_formatting() {
        let ms = parse_vtt_timestamp("01:23:45.678").unwrap();
        assert_eq!(ms, 3600_000 + 23 * 60_000 + 45 * 1000 + 678);
        assert_eq!(format_vtt_timestamp(ms), "01:23:45.678");

        let ms2 = parse_vtt_timestamp("05:12.340").unwrap();
        assert_eq!(ms2, 5 * 60_000 + 12 * 1000 + 340);
        assert_eq!(format_vtt_timestamp(ms2), "00:05:12.340");
    }

    #[test]
    fn test_merge_cues_ordering() {
        let sub = vec![
            VttCue {
                start_ms: 5000,
                end_ms: 8000,
                text: "Lời thoại 1".into(),
                is_caption: false,
                settings: None,
            },
            VttCue {
                start_ms: 12000,
                end_ms: 15000,
                text: "Lời thoại 2".into(),
                is_caption: false,
                settings: None,
            },
        ];

        let cap = vec![VttCue {
            start_ms: 3000,
            end_ms: 6000,
            text: "《Nhân vật A》".into(),
            is_caption: true,
            settings: Some("line:20%".into()),
        }];

        let merged = merge_cues(&sub, &cap);
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].text, "《Nhân vật A》");
        assert_eq!(merged[1].text, "Lời thoại 1");
        assert_eq!(merged[2].text, "Lời thoại 2");

        let serialized = serialize_vtt(&merged);
        assert!(serialized.contains("WEBVTT"));
        assert!(serialized.contains("00:00:03.000 --> 00:00:06.000 line:20%"));
    }

    #[test]
    fn test_preserve_vtt_cue_settings_parsing_and_serialization() {
        let sample_vtt = r#"WEBVTT

00:00:25.800 --> 00:00:27.900 line:20% position:50% align:center
《小女儿-陆淼淼》

00:00:38.400 --> 00:00:39.700 line:20%
《老二-陆正然》
"#;
        let cues = parse_vtt(sample_vtt, true);
        assert_eq!(cues.len(), 2);
        assert_eq!(
            cues[0].settings.as_deref(),
            Some("line:20% position:50% align:center")
        );
        assert_eq!(cues[1].settings.as_deref(), Some("line:20%"));

        let serialized = serialize_vtt(&cues);
        assert!(serialized.contains("00:00:25.800 --> 00:00:27.900 line:20% position:50% align:center"));
        assert!(serialized.contains("00:00:38.400 --> 00:00:39.700 line:20%"));
    }
}
