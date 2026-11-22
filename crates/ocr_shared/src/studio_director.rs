//! Bộ não điều phối Studio (Stage 3): hiệu đính OCR, phân định nhân vật, dịch
//! chuẩn vai vế/ngôi thứ và gắn nhịp nghỉ cho TTS.
//!
//! Model đích là LLM text thuần ONNX (Qwen2.5-7B-Instruct INT4); module này chỉ
//! dựng dữ liệu vào/ra và prompt, không tự nạp model.

use serde::{Deserialize, Serialize};

/// Cấu trúc đầu vào gửi cho LLM Coordinator theo từng gói cue.
#[derive(Debug, Serialize)]
pub struct DirectorBatchInput {
    pub chunk_id: i64,
    /// Sổ tay nhân vật, cấp bậc, địa danh đã khóa từ đầu phim tới hiện tại
    pub active_glossary: Vec<GlossaryEntry>,
    /// Các câu cuối của gói trước để giữ mạch xưng hô liên tục
    pub previous_context: Vec<TranslatedCue>,
    /// Danh sách cue cần hiệu đính & dịch trong gói hiện tại
    pub cues_to_process: Vec<RawCueInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlossaryEntry {
    pub zh_term: String,
    pub vi_term: String,
    /// "PERSON" | "TITLE_RANK" | "LOCATION" | "SKILL"
    pub category: String,
    /// Ví dụ: "Sư tôn - Đồ đệ: xưng Ta - Con; với kẻ địch: Bản tôn - Ngươi"
    pub pronoun_rule: String,
}

#[derive(Debug, Serialize)]
pub struct RawCueInput {
    pub id: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    /// "SUB" hoặc "CAP"
    pub kind: String,
    /// Từ Light-ASD / Active Speaker, rỗng nếu người nói ngoài khung hình
    pub speaker_hint: String,
    pub raw_zh: String,
}

/// Cấu trúc JSON đầu ra bắt buộc từ LLM Coordinator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectorBatchOutput {
    /// Thực thể mới phát hiện từ CAP hoặc thoại để cập nhật vào FerriteDB
    pub new_glossary_entries: Vec<GlossaryEntry>,
    pub cues: Vec<TranslatedCue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranslatedCue {
    pub id: i64,
    /// Văn bản gốc tiếng Trung đã sửa lỗi đồng âm/rụng nét của OCR (xuất .zh.vtt)
    pub clean_zh: String,
    /// Nhân vật được suy luận từ ngữ cảnh + khẩu hình + thẻ tên
    pub resolved_speaker: String,
    /// Bản dịch tiếng Việt chuẩn vai vế, giữ nguyên ngập ngừng
    pub translated_vi: String,
    /// Khoảng nghỉ sau câu (ms) cho bộ tổng hợp giọng nói TTS
    pub tts_pause_after_ms: u32,
}

/// System prompt chuẩn studio cho LLM Coordinator.
pub fn build_studio_director_prompt(batch: &DirectorBatchInput) -> String {
    let batch_json = serde_json::to_string(batch).unwrap_or_default();
    format!(
        r#"Bạn là Đạo diễn Hậu kỳ & Biên dịch Phim chuyên nghiệp (Trung - Việt).
Nhiệm vụ của bạn là xử lý gói phụ đề và thẻ chú thích (caption) sau đây để chuẩn bị cho khâu lồng tiếng TTS:

QUY TẮC BẮT BUỘC:
1. HIỆU ĐÍNH OCR (`clean_zh`): Sửa các lỗi nhận diện sai mặt chữ Hán đồng âm/gần nét dựa trên ngữ cảnh, nhưng GIỮ NGUYÊN từ đơn cảm thán (哦, 啊, 嗯, 自...) và cấu trúc lặp ngập ngừng.
2. PHÂN ĐỊNH NHÂN VẬT (`resolved_speaker`): Phim ít diễn viên lồng tiếng nên KHÔNG dựa vào chất giọng. Hãy kết hợp `speaker_hint`, thẻ chú thích `CAP` (tên, cấp bậc, địa danh), và mạch đối đáp để xác định chính xác ai đang nói với ai.
3. CHUẨN VAI VẾ & NGÔI THỨ (`translated_vi`):
   - Áp dụng nghiêm ngặt `active_glossary` đã có và bổ sung nhân vật/cấp bậc mới vào `new_glossary_entries`.
   - Dịch đúng ngôi thứ theo bối cảnh phim (Tiên hiệp/Cổ trang: Ta - Ngươi, Tại hạ - Các hạ, Sư tôn - Đồ nhi, Thuộc hạ - Điện hạ; Đô thị: Tôi - Anh/Cô/Ông/Ngài...). Tuyệt đối không dịch máy móc thành "Tôi - Bạn".
4. BẢO TOÀN NHỊP NGẬP NGỪNG & TỪ ĐƠN CHO TTS:
   - Giữ nguyên nhịp lắp bắp, ngập ngừng tự nhiên trong tiếng Việt (ví dụ: "我…我想" -> "T-tôi muốn...", "呃…那个" -> "À... thì...", "啊" -> "A!").
   - Đặt `tts_pause_after_ms` (0 đến 800ms) dựa trên dấu câu và khoảng cách thời gian tới câu kế tiếp.
5. ĐỊNH DẠNG ĐẦU RA: Chỉ trả về DUY NHẤT một khối JSON hợp lệ khớp schema `DirectorBatchOutput`, không giải thích thêm.

DỮ LIỆU ĐẦU VÀO:
{batch_json}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_batch() -> DirectorBatchInput {
        DirectorBatchInput {
            chunk_id: 3,
            active_glossary: vec![GlossaryEntry {
                zh_term: "宗主".to_string(),
                vi_term: "Tông chủ".to_string(),
                category: "TITLE_RANK".to_string(),
                pronoun_rule: "Bề trên: xưng Ta - Con".to_string(),
            }],
            previous_context: vec![],
            cues_to_process: vec![RawCueInput {
                id: 11,
                start_ms: 1_000,
                end_ms: 1_800,
                kind: "SUB".to_string(),
                speaker_hint: "Tiêu Viêm".to_string(),
                raw_zh: "呃……那个，我-我想".to_string(),
            }],
        }
    }

    #[test]
    fn prompt_embeds_hesitation_preserving_rules_and_batch_json() {
        let prompt = build_studio_director_prompt(&sample_batch());
        assert!(prompt.contains("DirectorBatchOutput"));
        assert!(prompt.contains("tts_pause_after_ms"));
        assert!(prompt.contains("\"raw_zh\":\"呃……那个，我-我想\""));
        assert!(prompt.contains("\"chunk_id\":3"));
    }

    #[test]
    fn director_output_round_trips_through_json() {
        let output = DirectorBatchOutput {
            new_glossary_entries: vec![],
            cues: vec![TranslatedCue {
                id: 11,
                clean_zh: "我…我想".to_string(),
                resolved_speaker: "Tiêu Viêm".to_string(),
                translated_vi: "T-tôi muốn...".to_string(),
                tts_pause_after_ms: 240,
            }],
        };
        let json = serde_json::to_string(&output).unwrap();
        let decoded: DirectorBatchOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, output);
    }
}
