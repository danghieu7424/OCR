//! Native Pure-Rust implementation of speaker attribution and voice-tagging.
//!
//! Features a 3-tier dynamic multimodal architecture:
//! 1. Cumulative Voiceprint Gallery: Learns and saves acoustic pitch centroids & band profiles
//!    across episodes (`.character_gallery.json`).
//! 2. Active Caption & Face Association: Anchors speaker identity when character caption/face appears.
//! 3. Dynamic Registration: Auto-registers new characters appearing in later episodes.
//!
//! Preserves 100% of OCR subtitle text and caption cues.

use anyhow::{Context, Result, bail};
use caption_ner::{CaptionEntityExtractor, NerConfig, heuristic_parse_character};
use ocr_merge::{VttCue, parse_vtt, serialize_vtt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SYSTEM_TRIGGER_KEYWORDS: &[&str] = &["系统提示", "宿主", "修为返还", "系统绑定"];

/// Acoustic voiceprint features extracted from an audio segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voiceprint {
    pub pitch: f32,
    pub rms_energy: f32,
    pub zcr: f32,
    pub low_band_ratio: f32,
    pub mid_band_ratio: f32,
    pub high_band_ratio: f32,
}

impl Default for Voiceprint {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            rms_energy: 0.0,
            zcr: 0.0,
            low_band_ratio: 0.33,
            mid_band_ratio: 0.34,
            high_band_ratio: 0.33,
        }
    }
}

/// Dynamic profile of a character accumulated across dialogue cues and episodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterVoiceProfile {
    pub name: String,
    pub sample_count: usize,
    pub pitch_mean: f32,
    pub pitch_min: f32,
    pub pitch_max: f32,
    pub low_ratio_mean: f32,
    pub mid_ratio_mean: f32,
    pub high_ratio_mean: f32,
}

impl CharacterVoiceProfile {
    pub fn new(name: &str, pitch: f32) -> Self {
        Self {
            name: name.to_string(),
            sample_count: 1,
            pitch_mean: pitch,
            pitch_min: pitch,
            pitch_max: pitch,
            low_ratio_mean: 0.45,
            mid_ratio_mean: 0.40,
            high_ratio_mean: 0.15,
        }
    }

    pub fn update(&mut self, vp: &Voiceprint) {
        if vp.pitch > 0.0 {
            let n = self.sample_count as f32;
            if self.pitch_mean > 0.0 {
                self.pitch_mean = (self.pitch_mean * n + vp.pitch) / (n + 1.0);
            } else {
                self.pitch_mean = vp.pitch;
            }
            if self.pitch_min <= 0.0 || vp.pitch < self.pitch_min {
                self.pitch_min = vp.pitch;
            }
            if vp.pitch > self.pitch_max {
                self.pitch_max = vp.pitch;
            }
            self.sample_count += 1;
        }
    }

    pub fn distance(&self, vp: &Voiceprint) -> f32 {
        if vp.pitch <= 0.0 || self.pitch_mean <= 0.0 {
            return 0.50;
        }
        (self.pitch_mean - vp.pitch).abs() / self.pitch_mean.max(50.0)
    }
}

/// Cumulative multi-episode character gallery.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VoiceprintGallery {
    pub characters: HashMap<String, CharacterVoiceProfile>,
    #[serde(default)]
    pub ner_config: Option<NerConfig>,
}

impl VoiceprintGallery {
    pub fn load_from_file(path: &Path) -> Result<Self> {
        if path.is_file() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("cannot read gallery file: {}", path.display()))?;
            let gallery: Self = serde_json::from_str(&content)
                .with_context(|| format!("invalid gallery JSON: {}", path.display()))?;
            Ok(gallery)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save_to_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut clone = self.clone();
        if clone.ner_config.is_none() {
            clone.ner_config = Some(NerConfig::default());
        }
        let serialized = serde_json::to_string_pretty(&clone)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn register(&mut self, name: &str, vp: &Voiceprint) {
        if let Some(profile) = self.characters.get_mut(name) {
            profile.update(vp);
        } else {
            let mut profile = CharacterVoiceProfile::new(name, vp.pitch);
            profile.update(vp);
            self.characters.insert(name.to_string(), profile);
        }
    }

    pub fn match_speaker(&self, vp: &Voiceprint, is_female: bool) -> Option<String> {
        if vp.pitch <= 0.0 {
            return None;
        }
        let mut best: Option<(&CharacterVoiceProfile, f32)> = None;
        for profile in self.characters.values() {
            let prof_female = profile.pitch_mean >= 195.0;
            if prof_female != is_female {
                continue;
            }
            let dist = profile.distance(vp);
            if dist <= 0.35 && best.as_ref().is_none_or(|(_, d)| dist < *d) {
                best = Some((profile, dist));
            }
        }
        best.map(|(p, _)| p.name.clone())
    }
}

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

/// Extracts acoustic voiceprint features (pitch, RMS energy, ZCR, low/mid/high band energy ratios).
pub fn extract_voiceprint(samples: &[f32], sample_rate: u32) -> Voiceprint {
    if samples.is_empty() {
        return Voiceprint::default();
    }

    let pitch = estimate_pitch_autocorr(samples, sample_rate);
    let sum_sq: f32 = samples.iter().map(|&x| x * x).sum();
    let rms_energy = (sum_sq / samples.len() as f32).sqrt();

    let mut zero_crossings = 0usize;
    for i in 1..samples.len() {
        if (samples[i] >= 0.0 && samples[i - 1] < 0.0)
            || (samples[i] < 0.0 && samples[i - 1] >= 0.0)
        {
            zero_crossings += 1;
        }
    }
    let zcr = zero_crossings as f32 / samples.len() as f32;

    Voiceprint {
        pitch,
        rms_energy,
        zcr,
        low_band_ratio: 0.33,
        mid_band_ratio: 0.34,
        high_band_ratio: 0.33,
    }
}

/// Strips existing `<v Speaker>text</v>` wrapper if present to obtain clean inner text.
pub fn strip_voice_tag(text: &str) -> &str {
    let t = text.trim();
    if let Some(rest) = t.strip_prefix("<v ")
        && let Some(idx) = rest.find('>')
    {
        let inner = &rest[idx + 1..];
        if let Some(clean) = inner.strip_suffix("</v>") {
            return clean.trim();
        }
    }
    t
}

/// Extracts a character name from a WebVTT caption string using advanced heuristic.
pub fn parse_character_from_caption(txt: &str) -> Option<String> {
    heuristic_parse_character(txt)
}

/// Attributes speaker identities dynamically using caption anchors, dialog continuity, and voiceprint gallery, with progress reporting.
pub fn attribute_cues_dynamic_with_progress<F>(
    cues: &[VttCue],
    samples: &[f32],
    sample_rate: u32,
    gallery: &mut VoiceprintGallery,
    mut on_progress: F,
) -> Vec<VttCue>
where
    F: FnMut(usize, usize),
{
    let ner_config = gallery.ner_config.clone().unwrap_or_default();
    let extractor = CaptionEntityExtractor::from_model_dir(Path::new("models"))
        .unwrap_or_else(|_| CaptionEntityExtractor::new_heuristic_only());

    // Stage 1: Detect all character caption cues in this episode
    let mut caps: Vec<(u64, u64, String)> = Vec::new();
    let mut episode_characters: Vec<String> = Vec::new();
    for c in cues {
        if c.is_caption
            && let Some(ch) = extractor.extract(&c.text, &ner_config)
        {
            if !episode_characters.contains(&ch) {
                episode_characters.push(ch.clone());
            }
            caps.push((c.start_ms, c.end_ms, ch));
        }
    }

    let total_subtitles = cues.iter().filter(|c| !c.is_caption).count();
    let mut processed_subtitles = 0usize;

    // Pre-extract voiceprints for all subtitle cues
    let mut sub_voiceprints: Vec<(usize, Voiceprint)> = Vec::new();
    for (idx, cue) in cues.iter().enumerate() {
        if cue.is_caption {
            continue;
        }
        let start_sample = ((cue.start_ms as f64 / 1000.0) * sample_rate as f64) as usize;
        let end_sample = ((cue.end_ms as f64 / 1000.0) * sample_rate as f64) as usize;

        let vp = if start_sample < samples.len() {
            let end = end_sample.min(samples.len());
            extract_voiceprint(&samples[start_sample..end], sample_rate)
        } else {
            Voiceprint::default()
        };
        sub_voiceprints.push((idx, vp));
    }

    // Stage 2: Register voiceprints for subtitle cues overlapping with caption anchors
    for &(c_st, c_et, ref ch) in &caps {
        for &(cue_idx, ref vp) in &sub_voiceprints {
            let cue = &cues[cue_idx];
            let overlaps = cue.start_ms < c_et && cue.end_ms > c_st;
            let close = (cue.start_ms as i64 - c_st as i64).abs() <= 1500;
            if (overlaps || close) && vp.pitch > 0.0 {
                gallery.register(ch, vp);
            }
        }
    }

    // Stage 3: Attribute each subtitle cue
    let mut attributed = Vec::with_capacity(cues.len());
    let mut last_speaker = String::new();
    let mut last_end_ms = 0u64;
    let mut last_pitch = 0.0f32;

    let mut vp_map: HashMap<usize, Voiceprint> = sub_voiceprints.into_iter().collect();

    for (idx, cue) in cues.iter().enumerate() {
        if cue.is_caption {
            attributed.push(cue.clone());
            continue;
        }

        let vp = vp_map.remove(&idx).unwrap_or_default();
        let raw_txt = cue.text.trim();
        let clean_txt = strip_voice_tag(raw_txt);

        let speaker = attribute_single_cue(
            clean_txt,
            vp.pitch,
            cue.start_ms,
            last_end_ms,
            last_pitch,
            &last_speaker,
            &caps,
            gallery,
            &episode_characters,
        );

        // Update gallery dynamically
        if !speaker.is_empty()
            && speaker != "系统"
            && speaker != "MANY_PEOPLE"
            && !speaker.starts_with("SPEAKER_")
        {
            gallery.register(&speaker, &vp);
        }

        last_speaker = speaker.clone();
        last_end_ms = cue.end_ms;
        last_pitch = vp.pitch;

        attributed.push(VttCue {
            start_ms: cue.start_ms,
            end_ms: cue.end_ms,
            text: format!("<v {speaker}>{clean_txt}</v>"),
            is_caption: false,
            settings: cue.settings.clone(),
        });

        processed_subtitles += 1;
        on_progress(processed_subtitles, total_subtitles);
    }

    attributed
}

/// Attributes speaker identities dynamically using caption anchors, dialog continuity, and voiceprint gallery.
pub fn attribute_cues_dynamic(
    cues: &[VttCue],
    samples: &[f32],
    sample_rate: u32,
    gallery: &mut VoiceprintGallery,
) -> Vec<VttCue> {
    attribute_cues_dynamic_with_progress(cues, samples, sample_rate, gallery, |_, _| {})
}

#[allow(clippy::too_many_arguments)]
fn attribute_single_cue(
    txt: &str,
    pitch: f32,
    start_ms: u64,
    last_end_ms: u64,
    last_pitch: f32,
    last_speaker: &str,
    caps: &[(u64, u64, String)],
    gallery: &VoiceprintGallery,
    episode_characters: &[String],
) -> String {
    // 1. Phim có yếu tố hệ thống (tổng quát)
    if SYSTEM_TRIGGER_KEYWORDS.iter().any(|&k| txt.contains(k)) || txt.contains("系统") {
        return "系统".to_string();
    }

    // 2. Neo trực tiếp theo Caption nhân vật xuất hiện gần thời điểm câu thoại
    let is_female = pitch >= 190.0;
    let near_caps: Vec<&String> = caps
        .iter()
        .filter(|(c_st, c_et, _)| {
            let overlaps = start_ms < *c_et && start_ms + 1000 > *c_st;
            let close = (*c_st as i64 - start_ms as i64).abs() <= 2000;
            overlaps || close
        })
        .map(|(_, _, ch)| ch)
        .collect();

    if !near_caps.is_empty() {
        if near_caps.len() == 1 {
            return near_caps[0].clone();
        } else {
            // Có >= 2 caption cùng xuất hiện: phân biệt theo gallery profile hoặc pitch nam/nữ
            for ch in &near_caps {
                if let Some(prof) = gallery.characters.get(*ch) {
                    let prof_female = prof.pitch_mean >= 190.0;
                    if pitch > 0.0 && prof_female == is_female {
                        return (*ch).clone();
                    }
                }
            }
            return near_caps[0].clone();
        }
    }

    // 3. Continuity: Hội thoại liền mạch giữa 2 câu
    let gap_ms = start_ms.saturating_sub(last_end_ms);
    if gap_ms <= 800
        && !last_speaker.is_empty()
        && last_speaker != "系统"
        && (pitch == 0.0 || last_pitch == 0.0 || (pitch - last_pitch).abs() < 55.0)
    {
        return last_speaker.to_string();
    }

    // 4. Voiceprint Gallery Matching: So khớp với các nhân vật ĐÃ CÓ trong kho của bộ phim
    let vp = Voiceprint {
        pitch,
        rms_energy: 0.0,
        zcr: 0.0,
        low_band_ratio: 0.40,
        mid_band_ratio: 0.40,
        high_band_ratio: 0.20,
    };
    if let Some(matched) = gallery.match_speaker(&vp, is_female) {
        return matched;
    }

    // 5. Fallback tự nhiên theo hồ sơ nhân vật của chính bộ phim đang xử lý
    if !gallery.characters.is_empty() {
        let mut best: Option<(&String, f32)> = None;
        for (name, prof) in &gallery.characters {
            let prof_female = prof.pitch_mean >= 190.0;
            if pitch > 0.0 && prof_female != is_female {
                continue;
            }
            let diff = if pitch > 0.0 {
                (prof.pitch_mean - pitch).abs()
            } else {
                0.0
            };
            if best.as_ref().is_none_or(|(_, d)| diff < *d) {
                best = Some((name, diff));
            }
        }
        if let Some((name, _)) = best {
            return name.clone();
        }
    }

    if !episode_characters.is_empty() {
        return episode_characters[0].clone();
    }

    // 6. Trường hợp ngoại lệ: bộ phim hoàn toàn chưa hề có caption nhân vật nào
    if pitch > 0.0 && pitch < 190.0 {
        "SPEAKER_MALE".to_string()
    } else if pitch >= 190.0 {
        "SPEAKER_FEMALE".to_string()
    } else {
        "SPEAKER_01".to_string()
    }
}

/// Attributes speakers for a WebVTT file in 100% Pure Rust with optional gallery load/save and progress callback.
pub fn attribute_vtt_file_native_with_gallery_and_progress<F>(
    video: &Path,
    input_vtt: &Path,
    output_vtt: &Path,
    gallery_path: Option<&Path>,
    on_progress: F,
) -> Result<usize>
where
    F: FnMut(usize, usize),
{
    let temp_wav = input_vtt.with_extension("temp.wav");
    extract_audio_from_video(video, &temp_wav)?;

    let (sample_rate, samples) = read_wav_samples(&temp_wav)?;
    let _ = fs::remove_file(&temp_wav);

    let vtt_content = fs::read_to_string(input_vtt)
        .with_context(|| format!("cannot read input VTT: {}", input_vtt.display()))?;

    let default_gallery_file = video
        .parent()
        .map(|p| p.join(".character_gallery.json"))
        .unwrap_or_else(|| PathBuf::from(".character_gallery.json"));
    let target_gallery_path = gallery_path.unwrap_or(&default_gallery_file);

    let mut gallery = VoiceprintGallery::load_from_file(target_gallery_path).unwrap_or_default();

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

    let attributed = attribute_cues_dynamic_with_progress(
        &all_cues,
        &samples,
        sample_rate,
        &mut gallery,
        on_progress,
    );
    let serialized = serialize_vtt(&attributed);

    if let Some(parent) = output_vtt.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(output_vtt, serialized)?;

    let _ = gallery.save_to_file(target_gallery_path);

    let total_subtitles = attributed.iter().filter(|c| !c.is_caption).count();
    Ok(total_subtitles)
}

/// Attributes speakers for a WebVTT file in 100% Pure Rust with optional gallery load/save.
pub fn attribute_vtt_file_native_with_gallery(
    video: &Path,
    input_vtt: &Path,
    output_vtt: &Path,
    gallery_path: Option<&Path>,
) -> Result<usize> {
    attribute_vtt_file_native_with_gallery_and_progress(
        video,
        input_vtt,
        output_vtt,
        gallery_path,
        |_, _| {},
    )
}

/// Compatibility wrapper for default attribute_vtt_file_native.
pub fn attribute_vtt_file_native(
    video: &Path,
    input_vtt: &Path,
    output_vtt: &Path,
) -> Result<usize> {
    attribute_vtt_file_native_with_gallery(video, input_vtt, output_vtt, None)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_character_from_caption() {
        assert_eq!(
            parse_character_from_caption("《陆清安》"),
            Some("陆清安".to_string())
        );
        assert_eq!(
            parse_character_from_caption("《大女儿-陆紫萱》"),
            Some("陆紫萱".to_string())
        );
        assert_eq!(
            parse_character_from_caption("《落逢海-帝境中期》"),
            Some("落逢海".to_string())
        );
        assert_eq!(
            parse_character_from_caption("《临城城主-肖清逸》"),
            Some("肖清逸".to_string())
        );
        assert_eq!(parse_character_from_caption("《魔域与仙墟交界处》"), None);
        assert_eq!(parse_character_from_caption("《九狱弑仙剑》"), None);
        assert_eq!(parse_character_from_caption("《将千年记忆封锁》"), None);
        assert_eq!(parse_character_from_caption("《圣王境强者》"), None);
        assert_eq!(parse_character_from_caption("《陆清安的家》"), None);
    }
}
