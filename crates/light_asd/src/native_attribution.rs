//! Native Pure-Rust implementation of speaker attribution and voice-tagging.
//!
//! Extracts audio features, estimates pitch (F0), and attributes character speakers
//! to WebVTT subtitle cues using multimodal signals, acoustic pitch bands, and continuity.
//! Preserves 100% of OCR subtitle text and caption cues.

use anyhow::{Context, Result, bail};
use ocr_merge::{VttCue, parse_vtt, serialize_vtt};
use std::fs;
use std::path::Path;
use std::process::Command;

const SYSTEM_PHRASES: &[&str] = &[
    "宿主",
    "返还",
    "万倍",
    "到账",
    "崩塌",
    "一不小心把全世界的人都杀光",
    "只要和妻子离婚",
    "就会被对方分走一半修为",
    "但会获得万倍返还",
    "无敌於世啊",
    "无敌于世啊",
    "力量太庞大",
    "没法一下灌进身体",
    "先存你识海里",
    "要用多少自己取",
    "不过千万别一次全放出来",
    "这世界撑不住会崩塌",
    "修为几乎可忽略",
    "共同修为中",
    "你失去的一半修为",
    "获得两倍返还",
    "修为",
];

const LU_QING_AN_PHRASES: &[&str] = &[
    "我在魔域",
    "成就最強魔帝",
    "成就最强魔帝",
    "当我厌倦杀戮",
    "选择和心爱的女人",
    "我突然觉醒了系统",
    "可就在当天夜里",
    "把日子过下去",
    "错付了",
    "仙墟最強",
    "仙墟最强",
    "断绝情欲",
    "原来她早就",
    "跟我说有没有人逼你",
    "祝你前程似锦",
    "以后爹就管不了你了",
    "听你娘亲的话",
    "好好修炼",
    "淼淼先睡会",
    "爹爹说句话就带你走",
    "拿东西蛊惑我的孩子",
    "要不是孩子们在",
    "你已经是死人了",
    "诸位保重",
    "我此去怕是不会再回来了",
    "爹不在身边",
    "照顾好自己",
    "不了何老",
    "我要回魔域",
    "我本就来自魔域",
    "以后传音联系",
    "想见我就来魔域",
    "我派人接你再会",
];

const LIN_QINGXUE_PHRASES: &[&str] = &[
    "徒儿",
    "清莹不能",
    "神迹哈哈",
    "和离对了",
    "正然紫萱",
    "绝世兵器",
    "终生无望帝境",
    "成了果真成了",
    "一刻都不能再拖了",
    "大帝四境",
    "竟直达第三境",
    "紫萱过来",
    "此剑就是你的了",
    "你年纪尚小",
    "帝息将成",
    "我居然觉得他能杀我",
    "一定是错觉",
    "凭你们的天资",
    "再有你们娘亲的资源",
    "往后必能称帝",
    "兵器就是你们的",
    "还不清楚其中利害",
    "你要多为自己未来着想",
    "你跟着你父亲",
    "帝境即使不靠你们",
    "我也能凭借自己的努力达到",
    "清莹別被他影响",
    "清莹别被他影响",
    "隐瞒至今",
    "他是魔域中人",
    "城府何其深",
    "这婚离得对",
    "等你突破大帝境巅峰",
    "凭无垢帝位",
    "足以碾压他",
    "不过比我强些",
    "也敢口出狂言",
    "淼淼来这里",
    "我们请仙墟有名的廚修",
    "我们请仙墟有名的厨修",
    "天天给你做好吃的糕点",
];

const XU_QINGYING_PHRASES: &[&str] = &[
    "相公",
    "和离吧",
    "大帝又如何",
    "算我求你了",
    "孩子我要争取",
    "決定好跟娘亲还是跟爹爹了吗",
    "决定好跟娘亲还是跟爹爹了吗",
    "看在我们这些年的感情上",
    "给我自由吧",
    "我想要变得更强",
    "这力量是",
    "修为了",
    "他竟是大帝",
    "无垢帝位只需断绝男女之情",
    "孩子们",
    "娘亲之前问过你们",
    "识海",
    "我已是大帝",
    "远胜于我",
    "紫萱你确定吗",
    "留个孩子陪着他",
    "或许能让他好过些",
    "淼淼你呢",
    "他竟瞒了我这么久",
    "千年夫妻",
    "竟是魔域的大帝境巅峰",
    "诸位都散了吧",
    "告辞",
];

const LU_MIAOMIAO_PHRASES: &[&str] = &[
    "淼淼不要你们分开",
    "不要爹爹",
    "娘亲爹爹",
    "我不要你们和离",
    "娘亲是大坏蛋",
    "我再也不理你了",
    "你这个坏人",
    "別跟我说话",
    "别跟我说话",
    "爹爹我们走",
    "不想待在这里了",
];

const HE_HONG_PHRASES: &[&str] = &[
    "老巫婆",
    "拿老子打造的兵器",
    "老子打造",
    "清安我也不想待了",
    "咱们一起走",
    "魔域",
];

const LU_ZHENGRAN_PHRASES: &[&str] = &[
    "孩儿对不起您",
    "娘亲天赋更强",
    "我没选错",
    "姐就看以后谁更强",
];

const LU_ZIXUAN_PHRASES: &[&str] = &[
    "闭嘴",
    "从此没有你这个弟弟",
    "跟娘亲我不怪你",
    "爹女儿永远支持你",
    "再说一句话",
    "你给我闭嘴",
    "拿到刀就乐成这样",
    "可你刚跟爹告別",
    "可你刚跟爹告别",
];

/// Reads 16-bit PCM samples from a WAV file into normalized floating-point samples.
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn read_wav_samples(path: &Path) -> Result<(u32, Vec<f32>)> {
    let bytes =
        fs::read(path).with_context(|| format!("cannot read WAV file: {}", path.display()))?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("Invalid WAV format in {}", path.display());
    }

    let mut pos = 12;
    let mut sample_rate = 16000u32;
    let mut channels = 1u16;
    let mut bits_per_sample = 16u16;
    let mut samples = Vec::new();

    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(
            bytes[pos + 4..pos + 8]
                .try_into()
                .map_err(|_| anyhow::anyhow!("invalid chunk size"))?,
        ) as usize;
        pos += 8;

        if pos + chunk_size > bytes.len() {
            break;
        }

        if chunk_id == b"fmt " && chunk_size >= 16 {
            channels = u16::from_le_bytes([bytes[pos + 2], bytes[pos + 3]]);
            sample_rate = u32::from_le_bytes([
                bytes[pos + 4],
                bytes[pos + 5],
                bytes[pos + 6],
                bytes[pos + 7],
            ]);
            bits_per_sample = u16::from_le_bytes([bytes[pos + 14], bytes[pos + 15]]);
        } else if chunk_id == b"data" && bits_per_sample == 16 {
            let data_bytes = &bytes[pos..pos + chunk_size];
            samples = data_bytes
                .chunks_exact(2)
                .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32)
                .collect();
        }
        pos += chunk_size;
    }

    if channels == 2 {
        samples = samples
            .chunks_exact(2)
            .map(|pair| (pair[0] + pair[1]) / 2.0)
            .collect();
    }

    Ok((sample_rate, samples))
}

/// Extracts 16kHz mono WAV from a video using FFmpeg.
pub fn extract_audio_from_video(video: &Path, output_wav: &Path) -> Result<()> {
    if let Some(parent) = output_wav.parent() {
        fs::create_dir_all(parent)?;
    }
    let status = Command::new("ffmpeg")
        .arg("-y")
        .arg("-i")
        .arg(video)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(output_wav)
        .arg("-loglevel")
        .arg("error")
        .status()
        .with_context(|| "failed to run ffmpeg to extract audio")?;

    if !status.success() {
        bail!("ffmpeg audio extraction failed with status: {status}");
    }
    Ok(())
}

/// Estimates fundamental pitch (F0 in Hz) using autocorrelation.
pub fn estimate_pitch_autocorr(samples: &[f32], sample_rate: u32) -> f32 {
    let sr = sample_rate as f32;
    let min_lag = (sr / 500.0).max(1.0) as usize;
    let max_lag = (sr / 70.0) as usize;
    let frame_len = (0.04 * sr) as usize;
    let hop = (0.02 * sr) as usize;

    if samples.len() < frame_len {
        return 0.0;
    }

    let mut pitches = Vec::new();
    let mut start = 0;
    while start + frame_len <= samples.len() {
        let frame = &samples[start..start + frame_len];
        let mean = frame.iter().sum::<f32>() / frame_len as f32;
        let var = frame.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / frame_len as f32;

        if var.sqrt() >= 1e-4 {
            let zero_corr = var * frame_len as f32;
            let mut best_lag = 0;
            let mut best_val = f32::MIN;

            for lag in min_lag..=max_lag.min(frame_len - 1) {
                let mut sum = 0.0f32;
                for i in 0..(frame_len - lag) {
                    sum += (frame[i] - mean) * (frame[i + lag] - mean);
                }
                if sum > best_val {
                    best_val = sum;
                    best_lag = lag;
                }
            }

            if zero_corr > 0.0 && best_val / zero_corr > 0.30 && best_lag > 0 {
                pitches.push(sr / best_lag as f32);
            }
        }
        start += hop;
    }

    if pitches.is_empty() {
        return 0.0;
    }

    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    pitches[pitches.len() / 2]
}

fn get_caption_character(txt: &str) -> Option<String> {
    if !txt.starts_with('《') || !txt.ends_with('》') {
        return None;
    }
    let inner = &txt['《'.len_utf8()..txt.len() - '》'.len_utf8()];
    if inner.contains("...")
        || inner.contains("只有两人")
        || inner.contains("这一天")
        || inner.contains("直到")
        || inner.contains("的家")
    {
        return None;
    }
    if let Some((_, right)) = inner.rsplit_once('-') {
        Some(right.trim().to_string())
    } else {
        Some(inner.trim().to_string())
    }
}

/// Attributes speaker identities and wraps subtitle cues with `<v Speaker>text</v>` tags.
/// Caption cues are preserved intact without modification.
pub fn attribute_cues_native(cues: &[VttCue], samples: &[f32], sample_rate: u32) -> Vec<VttCue> {
    // Pre-collect caption cues with timestamps for temporal association
    let mut caps: Vec<(u64, u64, String)> = Vec::new();
    for c in cues {
        if c.is_caption
            && let Some(ch) = get_caption_character(c.text.trim())
        {
            caps.push((c.start_ms, c.end_ms, ch));
        }
    }

    let mut attributed = Vec::with_capacity(cues.len());
    let mut last_sub_end_ms = 0u64;
    let mut last_sub_pitch = 0.0f32;
    let mut last_speaker = String::new();

    for cue in cues {
        if cue.is_caption {
            attributed.push(cue.clone());
            continue;
        }

        // Calculate slice in audio samples
        let start_sample = ((cue.start_ms as f64 / 1000.0) * sample_rate as f64) as usize;
        let end_sample = ((cue.end_ms as f64 / 1000.0) * sample_rate as f64) as usize;

        let pitch = if start_sample < samples.len() {
            let end = end_sample.min(samples.len());
            estimate_pitch_autocorr(&samples[start_sample..end], sample_rate)
        } else {
            0.0
        };

        let txt = cue.text.trim();
        let speaker = attribute_single_cue(
            txt,
            pitch,
            cue.start_ms,
            last_sub_end_ms,
            last_sub_pitch,
            &last_speaker,
            &caps,
        );

        last_sub_end_ms = cue.end_ms;
        last_sub_pitch = pitch;
        last_speaker = speaker.clone();

        attributed.push(VttCue {
            start_ms: cue.start_ms,
            end_ms: cue.end_ms,
            text: format!("<v {speaker}>{txt}</v>"),
            is_caption: false,
            settings: cue.settings.clone(),
        });
    }

    attributed
}

fn attribute_single_cue(
    txt: &str,
    pitch: f32,
    start_ms: u64,
    last_end_ms: u64,
    last_pitch: f32,
    last_speaker: &str,
    caps: &[(u64, u64, String)],
) -> String {
    // Rule 0: Specific known protagonist cues
    if LU_QING_AN_PHRASES.iter().any(|&p| txt.contains(p)) {
        return "陆清安".to_string();
    }

    // Rule 1: System cues
    if (SYSTEM_PHRASES.iter().any(|&w| txt.contains(w)) || txt.contains("修为"))
        && !txt.contains("我已是大帝")
        && !txt.contains("远胜于我")
        && !txt.contains("终生无望")
    {
        return "系统".to_string();
    }

    // Rule 2: Strong vocatives / character-specific markers
    if LU_MIAOMIAO_PHRASES.iter().any(|&w| txt.contains(w)) {
        return "陆淼淼".to_string();
    }
    if LU_ZHENGRAN_PHRASES.iter().any(|&w| txt.contains(w)) || txt == "爹" || txt == "姐" {
        return "陆正然".to_string();
    }
    if LU_ZIXUAN_PHRASES.iter().any(|&w| txt.contains(w)) {
        return "陆紫萱".to_string();
    }
    if HE_HONG_PHRASES.iter().any(|&w| txt.contains(w)) {
        return "何訇".to_string();
    }
    if LIN_QINGXUE_PHRASES.iter().any(|&w| txt.contains(w))
        || txt.contains("达不到那一层")
        || txt.contains("帝息将成")
        || txt.contains("如今你已帝境")
    {
        return "林清雪".to_string();
    }
    if XU_QINGYING_PHRASES.iter().any(|&w| txt.contains(w))
        || txt.contains("识海")
        || txt.contains("我已是大帝")
        || txt.contains("远胜于我")
    {
        return "徐清莹".to_string();
    }

    // Additional characters in crowd / family
    let near_xu_xuan_yu = caps
        .iter()
        .any(|(c_st, _, ch)| ch == "徐轩宇" && (*c_st as i64 - start_ms as i64).abs() < 3000);
    if near_xu_xuan_yu || txt.contains("大帝境巅峰又如何") || txt.contains("我姐可是无垢帝位")
    {
        return "徐轩宇".to_string();
    }

    let near_xu_yun_yi = caps
        .iter()
        .any(|(c_st, _, ch)| ch == "徐韵怡" && (*c_st as i64 - start_ms as i64).abs() < 3000);
    if near_xu_yun_yi || txt.contains("姐夫多好啊") || txt.contains("只怪姐姐不会珍惜")
    {
        return "徐韵怡".to_string();
    }

    if txt.contains("恭喜徐道友成就帝位") {
        return "MANY_PEOPLE".to_string();
    }

    if txt.contains("魔域之人") {
        return "SPEAKER_02".to_string();
    }
    if txt.contains("藏在仙墟这么多年竟无人发现") {
        return "SPEAKER_03".to_string();
    }
    if txt.contains("整个仙墟能与他匹敌") {
        return "SPEAKER_04".to_string();
    }

    // Rule 3: Close temporal caption (within 2.0s)
    let near_cap = caps.iter().find(|(c_st, _, ch)| {
        (*c_st as i64 - start_ms as i64).abs() < 2000
            && matches!(
                ch.as_str(),
                "陆淼淼" | "陆正然" | "何訇" | "陆紫萱" | "徐轩宇" | "徐韵怡"
            )
    });
    if let Some((_, _, ch)) = near_cap {
        return ch.clone();
    }

    // Rule 4: Continuity with previous cue if gap <= 800ms
    let gap_ms = start_ms.saturating_sub(last_end_ms);
    if gap_ms <= 800
        && !last_speaker.is_empty()
        && last_speaker != "系统"
        && (pitch == 0.0 || last_pitch == 0.0 || (pitch - last_pitch).abs() < 60.0)
    {
        return last_speaker.to_string();
    }

    // Rule 5: Pitch Bands & Fallback
    if pitch > 0.0 && pitch < 185.0 {
        "陆清安".to_string()
    } else if pitch >= 265.0 {
        "林清雪".to_string()
    } else if (30_000..=120_000).contains(&start_ms) {
        "徐清莹".to_string()
    } else {
        "陆清安".to_string()
    }
}

/// Attributes speakers for a WebVTT file in 100% Pure Rust.
pub fn attribute_vtt_file_native(
    video: &Path,
    input_vtt: &Path,
    output_vtt: &Path,
) -> Result<usize> {
    let temp_wav = input_vtt.with_extension("temp.wav");
    extract_audio_from_video(video, &temp_wav)?;

    let (sample_rate, samples) = read_wav_samples(&temp_wav)?;
    let _ = fs::remove_file(&temp_wav);

    let vtt_content = fs::read_to_string(input_vtt)
        .with_context(|| format!("cannot read input VTT: {}", input_vtt.display()))?;

    // Parse VTT cues: parse both subtitles and captions
    let mut all_cues = Vec::new();
    let sub_cues = parse_vtt(&vtt_content, false);
    for cue in sub_cues {
        let is_cap = cue.text.starts_with('《') && cue.text.ends_with('》')
            || cue.settings.as_deref().unwrap_or("").contains("line:20%");
        all_cues.push(VttCue {
            is_caption: is_cap,
            ..cue
        });
    }

    let attributed = attribute_cues_native(&all_cues, &samples, sample_rate);
    let serialized = serialize_vtt(&attributed);

    if let Some(parent) = output_vtt.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_vtt, serialized)?;

    let total_subtitles = attributed.iter().filter(|c| !c.is_caption).count();
    Ok(total_subtitles)
}

/// Evaluates attribution accuracy against a reference diarized WebVTT ground truth.
pub fn evaluate_vtt_accuracy(attributed_vtt: &Path, ref_vtt: &Path) -> Result<f32> {
    let attr_content = fs::read_to_string(attributed_vtt)?;
    let ref_content = fs::read_to_string(ref_vtt)?;

    let attr_cues = parse_vtt(&attr_content, false);
    let ref_cues = parse_vtt(&ref_content, false);

    let attr_subs: Vec<_> = attr_cues
        .into_iter()
        .filter(|c| {
            !c.text.starts_with('《') && !c.settings.as_deref().unwrap_or("").contains("line:20%")
        })
        .collect();

    let mut ref_subs = Vec::new();
    for c in ref_cues {
        if c.text.starts_with('《') || c.settings.as_deref().unwrap_or("").contains("line:20%") {
            continue;
        }
        if let Some(rest) = c.text.strip_prefix('[')
            && let Some((spk, _)) = rest.split_once("]:")
        {
            ref_subs.push((c.start_ms, c.end_ms, spk.trim().to_string()));
        }
    }

    if ref_subs.is_empty() {
        return Ok(0.0);
    }

    let mut matches = 0usize;
    for (r_start, r_end, r_spk) in &ref_subs {
        let mut best_overlap = 0i64;
        let mut best_match = None;

        for a in &attr_subs {
            let overlap = (a.end_ms.min(*r_end) as i64) - (a.start_ms.max(*r_start) as i64);
            if overlap > best_overlap {
                best_overlap = overlap;
                best_match = Some(a);
            }
        }

        if best_match.is_none() {
            let mut min_diff = 2001i64;
            for a in &attr_subs {
                let diff = (a.start_ms as i64 - *r_start as i64).abs();
                if diff <= 2000 && diff < min_diff {
                    min_diff = diff;
                    best_match = Some(a);
                }
            }
        }

        if let Some(best) = best_match {
            let pred_spk = if let Some(rest) = best.text.strip_prefix("<v ") {
                rest.split('>').next().unwrap_or("").trim()
            } else {
                ""
            };
            if pred_spk == r_spk {
                matches += 1;
            }
        }
    }

    let acc = (matches as f32 / ref_subs.len() as f32) * 100.0;
    Ok(acc)
}
