//! Phát hiện khoảng có tiếng nói (VAD) thuần CPU và lập kế hoạch chia chunk audio.
//!
//! Tầng này KHÔNG phân vai người nói; nó chỉ trả lời "khoảng nào có tiếng người"
//! để (1) `ocr_subtitles::finalize_tts_safe_segments` tách đúng nhịp nghỉ cho TTS
//! và (2) bộ chia chunk 5 phút có mốc checkpoint ổn định.
//!
//! Bộ dò dùng năng lượng RMS (dBFS) kết hợp Zero Crossing Rate, có trễ hysteresis:
//! vào trạng thái nói ở `enter_db`, chỉ thoát ở `exit_db` thấp hơn, và phải im lặng
//! liên tục `min_silence_sec` mới đóng khoảng. `min_silence_sec` mặc định 0.18s khớp
//! hằng số trong `has_vad_silence_between`.
//!
//! Khi có `models/vad/silero_vad.onnx`, backend ONNX có thể được cắm thêm sau này;
//! API công khai ở đây cố tình chỉ phụ thuộc PCM f32 để không kéo `ort` vào
//! `ocr_shared`.

/// Tần số lấy mẫu chuẩn cho VAD (ffmpeg `-ar 16000`).
pub const VAD_SAMPLE_RATE: u32 = 16_000;

/// Silero VAD nhận frame 512 mẫu (32ms @16kHz).
pub const VAD_FRAME_SAMPLES: usize = 512;

/// Độ dài chunk mặc định của pipeline (5 phút).
pub const DEFAULT_CHUNK_SEC: f64 = 300.0;

/// Bước nhảy khoá cache khi một tập được chia thành nhiều chunk.
pub const CHUNK_ID_STRIDE: i64 = 100;

/****
 * SpeechSpan
 * Một khoảng có tiếng nói, đơn vị giây, đã bao gồm phần đệm hai đầu.
 ****/
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechSpan {
    pub start_sec: f64,
    pub end_sec: f64,
}

impl SpeechSpan {
    pub fn duration(self) -> f64 {
        (self.end_sec - self.start_sec).max(0.0)
    }
}

/****
 * VadConfig
 * Ngưỡng và thời lượng tối thiểu của bộ dò năng lượng.
 ****/
#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    /// Số mẫu mỗi frame phân tích.
    pub frame_samples: usize,
    /// Ngưỡng dBFS để bắt đầu một khoảng nói.
    pub enter_db: f32,
    /// Ngưỡng dBFS để duy trì khoảng nói đang mở.
    pub exit_db: f32,
    /// Zero Crossing Rate tối đa để một frame nhiều năng lượng được coi là tiếng người.
    pub max_zcr: f32,
    /// Độ dài tối thiểu của một khoảng nói (ngắn hơn bị coi là nhiễu).
    pub min_speech_sec: f64,
    /// Thời gian im lặng liên tục tối thiểu để đóng khoảng nói.
    pub min_silence_sec: f64,
    /// Đệm thêm hai đầu mỗi khoảng nói (tránh cắt cụt phụ âm đầu/cuối).
    pub pad_sec: f64,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            frame_samples: VAD_FRAME_SAMPLES,
            enter_db: -35.0,
            exit_db: -45.0,
            max_zcr: 0.35,
            min_speech_sec: 0.10,
            min_silence_sec: 0.18,
            pad_sec: 0.05,
        }
    }
}

/****
 * AudioChunk
 * Một lát cắt thời gian của track audio, dùng cho checkpointing theo chunk.
 ****/
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioChunk {
    pub index: usize,
    pub start_sec: f64,
    pub end_sec: f64,
}

impl AudioChunk {
    pub fn duration(self) -> f64 {
        (self.end_sec - self.start_sec).max(0.0)
    }
}

/// Chuyển PCM 16-bit có dấu sang mẫu f32 trong khoảng [-1, 1).
pub fn pcm_i16_to_f32(samples: &[i16]) -> Vec<f32> {
    samples.iter().map(|s| *s as f32 / 32_768.0).collect()
}

fn frame_level_db(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return f32::NEG_INFINITY;
    }
    let sum_sq: f32 = frame.iter().map(|s| s * s).sum();
    let rms = (sum_sq / frame.len() as f32).sqrt();
    20.0 * rms.max(1e-7).log10()
}

fn frame_zero_crossing_rate(frame: &[f32]) -> f32 {
    if frame.len() < 2 {
        return 0.0;
    }
    let crossings = frame
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    crossings as f32 / (frame.len() - 1) as f32
}

/****
 * detect_speech_spans
 * Dò các khoảng có tiếng nói trên PCM mono f32. Kết quả đã lọc nhiễu ngắn, đã
 * đệm hai đầu và đã gộp các khoảng chồng lấn, sắp xếp tăng dần theo thời gian.
 ****/
pub fn detect_speech_spans(
    samples: &[f32],
    sample_rate: u32,
    config: &VadConfig,
) -> Vec<SpeechSpan> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }

    let frame_samples = config.frame_samples.max(1);
    let frame_sec = frame_samples as f64 / sample_rate as f64;
    let total_sec = samples.len() as f64 / sample_rate as f64;

    let mut spans: Vec<SpeechSpan> = Vec::new();
    let mut speech_start: Option<f64> = None;
    let mut last_voiced_end = 0.0_f64;
    let mut silence_run = 0.0_f64;

    for (index, frame) in samples.chunks(frame_samples).enumerate() {
        let start = index as f64 * frame_sec;
        let end = (start + frame_sec).min(total_sec);
        let level_db = frame_level_db(frame);

        // Hysteresis: mở khoảng ở enter_db, giữ khoảng xuống tới exit_db.
        let voiced = match speech_start {
            None => {
                level_db >= config.enter_db && frame_zero_crossing_rate(frame) <= config.max_zcr
            }
            Some(_) => level_db >= config.exit_db,
        };

        if voiced {
            if speech_start.is_none() {
                speech_start = Some(start);
            }
            last_voiced_end = end;
            silence_run = 0.0;
        } else if let Some(begin) = speech_start {
            silence_run += end - start;
            if silence_run >= config.min_silence_sec {
                spans.push(SpeechSpan {
                    start_sec: begin,
                    end_sec: last_voiced_end,
                });
                speech_start = None;
                silence_run = 0.0;
            }
        }
    }

    if let Some(begin) = speech_start {
        spans.push(SpeechSpan {
            start_sec: begin,
            end_sec: last_voiced_end,
        });
    }

    // Bỏ nhiễu ngắn (tiếng click, tiếng gõ) trước khi đệm.
    spans.retain(|span| span.duration() >= config.min_speech_sec);

    let mut padded: Vec<SpeechSpan> = Vec::with_capacity(spans.len());
    for span in spans {
        let start = (span.start_sec - config.pad_sec).max(0.0);
        let end = (span.end_sec + config.pad_sec).min(total_sec);
        if let Some(last) = padded.last_mut()
            && start <= last.end_sec + frame_sec * 0.5
        {
            last.end_sec = last.end_sec.max(end);
            continue;
        }
        padded.push(SpeechSpan {
            start_sec: start,
            end_sec: end,
        });
    }

    padded
}

/// [`detect_speech_spans`] với cấu hình mặc định.
pub fn detect_speech_spans_default(samples: &[f32], sample_rate: u32) -> Vec<SpeechSpan> {
    detect_speech_spans(samples, sample_rate, &VadConfig::default())
}

/****
 * plan_audio_chunks
 * Chia track audio thành các chunk đều nhau (mặc định 300s). Chunk cuối ngắn hơn
 * vẫn được giữ để không mất phần đuôi phim.
 ****/
pub fn plan_audio_chunks(total_duration_sec: f64, chunk_sec: f64) -> Vec<AudioChunk> {
    if !total_duration_sec.is_finite() || total_duration_sec <= 0.0 {
        return Vec::new();
    }
    let chunk_len = if chunk_sec.is_finite() && chunk_sec > 0.0 {
        chunk_sec
    } else {
        total_duration_sec
    };

    let mut chunks = Vec::new();
    let mut start = 0.0_f64;
    let mut index = 0_usize;
    while start < total_duration_sec - 1e-6 {
        let end = (start + chunk_len).min(total_duration_sec);
        chunks.push(AudioChunk {
            index,
            start_sec: start,
            end_sec: end,
        });
        index += 1;
        start = end;
    }
    chunks
}

/****
 * chunk_cache_id
 * Khoá cache cho từng chunk. Không dùng lại `chunk_id` mức tập (2001) vì
 * `has_chunk_raw_cues` sẽ bỏ qua toàn bộ tập sau khi chunk đầu được ghi.
 * Ví dụ: tập 2001, chunk 2 -> 200102.
 ****/
pub fn chunk_cache_id(episode_id: i64, chunk_index: usize) -> i64 {
    episode_id
        .saturating_mul(CHUNK_ID_STRIDE)
        .saturating_add(chunk_index as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f64, amplitude: f32) -> Vec<f32> {
        let count = (seconds * VAD_SAMPLE_RATE as f64) as usize;
        (0..count)
            .map(|i| {
                amplitude
                    * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / VAD_SAMPLE_RATE as f32).sin()
            })
            .collect()
    }

    fn silence(seconds: f64) -> Vec<f32> {
        vec![0.0; (seconds * VAD_SAMPLE_RATE as f64) as usize]
    }

    fn concat(parts: &[Vec<f32>]) -> Vec<f32> {
        parts.iter().flat_map(|p| p.iter().copied()).collect()
    }

    #[test]
    fn silent_track_has_no_speech_span() {
        let samples = silence(3.0);
        assert!(detect_speech_spans_default(&samples, VAD_SAMPLE_RATE).is_empty());
    }

    #[test]
    fn splits_two_utterances_separated_by_long_silence() {
        let samples = concat(&[
            silence(0.5),
            tone(0.4, 0.3),
            silence(0.5),
            tone(0.4, 0.3),
            silence(0.5),
        ]);
        let spans = detect_speech_spans_default(&samples, VAD_SAMPLE_RATE);
        assert_eq!(spans.len(), 2, "phải là 2 nhịp nói tách biệt: {spans:?}");
        assert!((spans[0].start_sec - 0.45).abs() < 0.06, "{:?}", spans[0]);
        assert!((spans[0].end_sec - 0.95).abs() < 0.06, "{:?}", spans[0]);
        assert!((spans[1].start_sec - 1.35).abs() < 0.06, "{:?}", spans[1]);
    }

    #[test]
    fn short_dip_inside_utterance_keeps_single_span() {
        let samples = concat(&[tone(0.4, 0.3), silence(0.05), tone(0.4, 0.3)]);
        let spans = detect_speech_spans_default(&samples, VAD_SAMPLE_RATE);
        assert_eq!(
            spans.len(),
            1,
            "khoảng lặng 50ms chưa đủ đóng câu: {spans:?}"
        );
    }

    #[test]
    fn drops_blip_shorter_than_min_speech() {
        let samples = concat(&[silence(0.5), tone(0.04, 0.3), silence(0.5)]);
        assert!(detect_speech_spans_default(&samples, VAD_SAMPLE_RATE).is_empty());
    }

    #[test]
    fn ignores_faint_tone_below_threshold() {
        let samples = concat(&[silence(0.2), tone(1.0, 0.002), silence(0.2)]);
        let spans = detect_speech_spans_default(&samples, VAD_SAMPLE_RATE);
        assert!(spans.is_empty(), "âm lượng -54dBFS không phải tiếng nói");
    }

    #[test]
    fn zero_crossing_rate_rejects_noise_burst() {
        // Nhiễu trắng biên độ lớn: nhiều năng lượng nhưng ZCR cao.
        let noise: Vec<f32> = (0..(VAD_SAMPLE_RATE as usize))
            .map(|i| if i % 2 == 0 { 0.6 } else { -0.6 })
            .collect();
        let samples = concat(&[silence(0.2), noise, silence(0.2)]);
        assert!(detect_speech_spans_default(&samples, VAD_SAMPLE_RATE).is_empty());
    }

    #[test]
    fn plans_five_minute_chunks_including_short_tail() {
        let chunks = plan_audio_chunks(641.0, DEFAULT_CHUNK_SEC);
        assert_eq!(chunks.len(), 3);
        assert_eq!((chunks[0].start_sec, chunks[0].end_sec), (0.0, 300.0));
        assert_eq!((chunks[2].start_sec, chunks[2].end_sec), (600.0, 641.0));
        assert_eq!(chunks[2].index, 2);
    }

    #[test]
    fn plan_audio_chunks_handles_empty_and_invalid_input() {
        assert!(plan_audio_chunks(0.0, DEFAULT_CHUNK_SEC).is_empty());
        assert!(plan_audio_chunks(-5.0, DEFAULT_CHUNK_SEC).is_empty());
        let single = plan_audio_chunks(12.0, 0.0);
        assert_eq!(single.len(), 1);
        assert_eq!(single[0].end_sec, 12.0);
    }

    #[test]
    fn chunk_cache_ids_do_not_collide_with_episode_keys() {
        assert_eq!(chunk_cache_id(2001, 0), 200_100);
        assert_eq!(chunk_cache_id(2001, 2), 200_102);
        assert_ne!(chunk_cache_id(2001, 0), 2001);
    }

    #[test]
    fn converts_pcm_i16_to_unit_scale() {
        let pcm = [0_i16, 16_384, -32_768];
        let floats = pcm_i16_to_f32(&pcm);
        assert_eq!(floats[0], 0.0);
        assert!((floats[1] - 0.5).abs() < 1e-6);
        assert!((floats[2] + 1.0).abs() < 1e-6);
    }
}
