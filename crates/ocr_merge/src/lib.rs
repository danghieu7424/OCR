// crates/ocr_merge/src/lib.rs
//! # ocr_merge - Ghép Subtitles và Captions thành 1 file WebVTT đồng bộ
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
            let seconds: u64 = sec_parts.first()?.parse().ok()?;
            let millis: u64 = sec_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + millis)
        }
        2 => {
            // MM:SS.mmm
            let minutes: u64 = parts[0].parse().ok()?;
            let sec_parts: Vec<&str> = parts[1].split('.').collect();
            let seconds: u64 = sec_parts.first()?.parse().ok()?;
            let millis: u64 = sec_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
            Some(minutes * 60_000 + seconds * 1_000 + millis)
        }
        _ => None,
    }
}

/// Định dạng mili-giây sang chuẩn WebVTT `HH:MM:SS.mmm`
pub fn format_vtt_timestamp(ms: u64) -> String {
    let hours = ms / 3_600_000;
    let rem = ms % 3_600_000;
    let minutes = rem / 60_000;
    let rem = rem % 60_000;
    let seconds = rem / 1_000;
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

                if let (Some(start_ms), Some(end_ms)) = (
                    parse_vtt_timestamp(start_part),
                    parse_vtt_timestamp(end_part),
                ) {
                    let mut text_lines = Vec::new();
                    for text_line in lines.by_ref() {
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
    merge_vtt_files_with_line_overrides(subtitle_path, caption_path, output_path, None, None)
}

/// Merge raw subtitle/caption VTTs and optionally replace only the `line` setting.
pub fn merge_vtt_files_with_line_overrides(
    subtitle_path: &Path,
    caption_path: &Path,
    output_path: &Path,
    subtitle_line: Option<&str>,
    caption_line: Option<&str>,
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

    let mut sub_cues = parse_vtt(&sub_content, false);
    let mut cap_cues = parse_vtt(&cap_content, true);
    apply_line_override(&mut sub_cues, subtitle_line)?;
    apply_line_override(&mut cap_cues, caption_line)?;

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

fn apply_line_override(cues: &mut [VttCue], line: Option<&str>) -> Result<()> {
    let Some(line) = line else {
        return Ok(());
    };
    let line = line.strip_prefix("line:").unwrap_or(line);
    if line.is_empty() || line.chars().any(char::is_whitespace) {
        anyhow::bail!("Giá trị line phải là một token WebVTT không rỗng");
    }
    let line_setting = format!("line:{line}");

    for cue in cues {
        let mut settings: Vec<&str> = cue
            .settings
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .filter(|setting| !setting.starts_with("line:"))
            .collect();
        settings.insert(0, &line_setting);
        cue.settings = Some(settings.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_parsing_and_formatting() {
        let ms = parse_vtt_timestamp("01:23:45.678").unwrap();
        assert_eq!(ms, 3_600_000 + 23 * 60_000 + 45 * 1_000 + 678);
        assert_eq!(format_vtt_timestamp(ms), "01:23:45.678");

        let ms2 = parse_vtt_timestamp("05:12.340").unwrap();
        assert_eq!(ms2, 5 * 60_000 + 12 * 1000 + 340);
        assert_eq!(format_vtt_timestamp(ms2), "00:05:12.340");
    }

    #[cfg(test)]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        const MAX_TIME_MS: u64 = 1_000_000_000_000_000;

        fn raw_cues() -> impl Strategy<Value = Vec<(u64, u64, u16)>> {
            prop::collection::vec((0u64..MAX_TIME_MS, 0u64..10_000u64, any::<u16>()), 0..20)
        }

        fn subtitles() -> impl Strategy<Value = Vec<VttCue>> {
            raw_cues().prop_map(|items| {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (start_ms, duration_ms, id))| VttCue {
                        start_ms,
                        end_ms: start_ms + duration_ms,
                        text: format!("subtitle-{index}-{id}"),
                        is_caption: false,
                        settings: None,
                    })
                    .collect()
            })
        }

        fn caption_settings() -> impl Strategy<Value = String> {
            prop::sample::select(vec![
                "line:20%".to_string(),
                "position:50%".to_string(),
                "align:center".to_string(),
                "line:20% position:45%".to_string(),
                "line:20% position:50% align:center".to_string(),
                "line:20% position:50% align:start".to_string(),
            ])
        }

        fn captions() -> impl Strategy<Value = Vec<VttCue>> {
            prop::collection::vec(
                (
                    0u64..MAX_TIME_MS,
                    0u64..10_000u64,
                    any::<u16>(),
                    caption_settings(),
                ),
                0..20,
            )
            .prop_map(|items| {
                items
                    .into_iter()
                    .enumerate()
                    .map(|(index, (start_ms, duration_ms, id, settings))| VttCue {
                        start_ms,
                        end_ms: start_ms + duration_ms,
                        text: format!("caption-{index}-{id}"),
                        is_caption: true,
                        settings: Some(settings),
                    })
                    .collect()
            })
        }

        proptest! {
            #![proptest_config(ProptestConfig {
                cases: 2_048,
                ..ProptestConfig::default()
            })]

            #[test]
            fn timestamp_format_parse_round_trip(milliseconds in 0u64..MAX_TIME_MS) {
                let formatted = format_vtt_timestamp(milliseconds);
                let parts: Vec<_> = formatted.split(':').collect();
                prop_assert_eq!(parts.len(), 3);
                prop_assert!(parts[0].len() >= 2);
                prop_assert_eq!(parts[1].len(), 2);

                let seconds_parts: Vec<_> = parts[2].split('.').collect();
                prop_assert_eq!(seconds_parts.len(), 2);
                prop_assert_eq!(seconds_parts[0].len(), 2);
                prop_assert_eq!(seconds_parts[1].len(), 3);
                prop_assert_eq!(parse_vtt_timestamp(&formatted), Some(milliseconds));
            }

            #[test]
            fn timestamp_parser_matches_hour_components(
                hours in 0u64..1_000_000,
                minutes in 0u64..60,
                seconds in 0u64..60,
                milliseconds in 0u64..1_000,
            ) {
                let timestamp = format!("{hours:02}:{minutes:02}:{seconds:02}.{milliseconds:03}");
                let expected = hours * 3_600_000 + minutes * 60_000
                    + seconds * 1_000 + milliseconds;
                prop_assert_eq!(parse_vtt_timestamp(&timestamp), Some(expected));
            }

            #[test]
            fn subtitle_caption_merge_render_round_trip(
                subtitles in subtitles(),
                captions in captions(),
            ) {
                let parsed_subtitles = parse_vtt(&serialize_vtt(&subtitles), false);
                let parsed_captions = parse_vtt(&serialize_vtt(&captions), true);
                let merged = merge_cues(&parsed_subtitles, &parsed_captions);
                let rendered = serialize_vtt(&merged);
                let round_trip = parse_vtt(&rendered, false);

                prop_assert_eq!(round_trip.len(), merged.len());
                prop_assert!(
                    round_trip
                        .windows(2)
                        .all(|pair| pair[0].start_ms <= pair[1].start_ms)
                );

                for (expected, actual) in merged.iter().zip(&round_trip) {
                    prop_assert_eq!(actual.start_ms, expected.start_ms);
                    prop_assert_eq!(actual.end_ms, expected.end_ms);
                    prop_assert_eq!(&actual.text, &expected.text);
                    prop_assert_eq!(&actual.settings, &expected.settings);
                }
            }

            #[test]
            fn caption_settings_survive_parse_merge_render(
                start_ms in 0u64..MAX_TIME_MS,
                duration_ms in 0u64..10_000u64,
                settings in caption_settings(),
            ) {
                let cue = VttCue {
                    start_ms,
                    end_ms: start_ms + duration_ms,
                    text: "caption".to_string(),
                    is_caption: true,
                    settings: Some(settings.clone()),
                };
                let parsed = parse_vtt(&serialize_vtt(&[cue]), true);
                let merged = merge_cues(&[], &parsed);
                let rendered = serialize_vtt(&merged);
                let round_trip = parse_vtt(&rendered, false);

                prop_assert_eq!(round_trip.len(), 1);
                prop_assert_eq!(round_trip[0].settings.as_deref(), Some(settings.as_str()));
                prop_assert_eq!(&round_trip[0].text, "caption");
            }
        }
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
        assert!(
            serialized.contains("00:00:25.800 --> 00:00:27.900 line:20% position:50% align:center")
        );
        assert!(serialized.contains("00:00:38.400 --> 00:00:39.700 line:20%"));
    }

    #[test]
    fn line_overrides_preserve_other_cue_settings_and_leave_raw_files_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let subtitle = temp.path().join("subtitles.vtt");
        let caption = temp.path().join("captions.vtt");
        let output = temp.path().join("merged.vtt");
        let subtitle_content = "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nsubtitle\n\n";
        let caption_content = "WEBVTT\n\n00:00:01.500 --> 00:00:02.500 line:20% position:45% align:start\ncaption\n\n";
        fs::write(&subtitle, subtitle_content).unwrap();
        fs::write(&caption, caption_content).unwrap();

        merge_vtt_files_with_line_overrides(
            &subtitle,
            &caption,
            &output,
            Some("85%"),
            Some("line:30%"),
        )
        .unwrap();

        let merged = fs::read_to_string(output).unwrap();
        assert!(merged.contains("00:00:01.000 --> 00:00:02.000 line:85%"));
        assert!(merged.contains("00:00:01.500 --> 00:00:02.500 line:30% position:45% align:start"));
        assert_eq!(fs::read_to_string(subtitle).unwrap(), subtitle_content);
        assert_eq!(fs::read_to_string(caption).unwrap(), caption_content);
    }

    #[test]
    fn line_override_rejects_empty_or_whitespace_values() {
        let temp = tempfile::tempdir().unwrap();
        let subtitle = temp.path().join("subtitles.vtt");
        let caption = temp.path().join("captions.vtt");
        let output = temp.path().join("merged.vtt");
        fs::write(&subtitle, "WEBVTT\n").unwrap();
        fs::write(&caption, "WEBVTT\n").unwrap();

        assert!(
            merge_vtt_files_with_line_overrides(&subtitle, &caption, &output, Some(""), None)
                .is_err()
        );
        assert!(
            merge_vtt_files_with_line_overrides(
                &subtitle,
                &caption,
                &output,
                None,
                Some("20% position:50%"),
            )
            .is_err()
        );
    }
}
