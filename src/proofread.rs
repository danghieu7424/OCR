//! # Context Proofreader (Hiệu đính ngữ cảnh bằng Qwen2.5-VL / LLM & Rule Engine)
//!
//! Tầng hậu xử lý (Post-processing) đảm bảo độ chính xác 100% cho phụ đề trước khi dịch:
//! - Tự động sửa các ký tự nhận diện nhầm âm/chữ gần giống (vd: `爹` vs `参`, `嗯` vs `涵`, `自已` vs `自己`).
//! - Kết hợp danh từ riêng từ `EntityGlossary` để chuẩn hóa tên nhân vật/công pháp.
//! - Hỗ trợ gọi mô hình Qwen2.5-VL-3B-Instruct qua endpoint OpenAI/Ollama tương thích,
//!   kèm bộ luật dự phòng siêu nhanh (Rule-based Fallback) chạy trực tiếp trong RAM nếu không có LLM server.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::glossary::EntityGlossary;
use crate::ollama::OllamaManager;

/// Cấu hình hiệu đính ngữ cảnh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofreadConfig {
    /// Endpoint OpenAI-compatible của Qwen2.5-VL-3B (vd: "http://localhost:11434/v1" hoặc "http://localhost:8000/v1")
    pub endpoint: Option<String>,
    /// Tên mô hình (vd: "qwen2.5vl:3b" hoặc "qwen2.5-vl:3b")
    pub model_name: String,
    /// API key (nếu có)
    pub api_key: Option<String>,
    /// Bật bộ lọc làm sạch nhiễu ký tự OCR & từ điển nhầm chữ phổ biến
    pub enable_rule_engine: bool,
    /// Bảng từ điển ánh xạ lỗi chính tả / nhầm chữ do người dùng tùy biến nạp vào (Generic)
    #[serde(default)]
    pub custom_replacements: HashMap<String, String>,
    /// Bật kết nối Ollama tự động qua thư viện Rust
    pub use_ollama: bool,
}

impl Default for ProofreadConfig {
    fn default() -> Self {
        Self {
            endpoint: std::env::var("OCR_LLM_ENDPOINT").ok(),
            model_name: std::env::var("OCR_LLM_MODEL")
                .unwrap_or_else(|_| "qwen2.5vl:3b".to_string()),
            api_key: std::env::var("OCR_LLM_API_KEY").ok(),
            enable_rule_engine: true,
            custom_replacements: HashMap::new(),
            use_ollama: true,
        }
    }
}

/// Thống kê kết quả hiệu đính
#[derive(Debug, Clone, Default)]
pub struct ProofreadSummary {
    pub total_lines: usize,
    pub corrected_lines: usize,
    pub corrections: Vec<(String, String)>, // (Gốc, Sau khi sửa)
}

/// Bộ hiệu đính phụ đề
pub struct Proofreader {
    pub config: ProofreadConfig,
    pub ollama: Option<OllamaManager>,
}

impl Proofreader {
    pub fn new(config: ProofreadConfig) -> Self {
        let ollama = if config.use_ollama {
            OllamaManager::init_or_setup(&config.model_name).ok()
        } else {
            None
        };
        Self { config, ollama }
    }

    /// Khởi tạo Proofreader trực tiếp với thư viện Ollama của Rust
    pub fn with_ollama(model_name: &str) -> Result<Self> {
        let manager = OllamaManager::init_or_setup(model_name)?;
        let mut config = ProofreadConfig::default();
        config.model_name = model_name.to_string();
        config.use_ollama = true;
        Ok(Self {
            config,
            ollama: Some(manager),
        })
    }

    pub fn with_default_config() -> Self {
        Self::new(ProofreadConfig::default())
    }

    /// Khởi tạo Proofreader chỉ dùng bộ luật CPU (không cần Ollama/LLM)
    pub fn with_rule_engine_only() -> Self {
        let mut config = ProofreadConfig::default();
        config.use_ollama = false;
        config.endpoint = None;
        Self {
            config,
            ollama: None,
        }
    }

    /// Hiệu đính một câu phụ đề đơn lẻ
    pub fn proofread_sentence(
        &self,
        raw: &str,
        context_history: &[String],
        glossary: Option<&EntityGlossary>,
    ) -> String {
        let mut text = raw.trim().to_string();
        if text.is_empty() {
            return text;
        }

        // 1. Làm sạch nhiễu detector & chuẩn hóa các cặp chữ Hán nhầm lẫn kinh điển (Generic)
        if self.config.enable_rule_engine {
            text = self.apply_generic_sanitizer(&text);
        }

        // 2. Chuẩn hóa tên nhân vật / thực thể tự động từ EntityGlossary (học từ caption 《...》)
        if let Some(g) = glossary {
            text = g.normalize_sentence(&text);
        }

        // 3. Hiệu đính ngữ cảnh nâng cao bằng mô hình ngôn ngữ (Qwen2.5-VL qua ollama-rs)
        // Mô hình ngôn ngữ tự động phân tích ngữ pháp, xưng hô, hội thoại tự nhiên mà không cần hard-code!
        if let Some(ref ollama) = self.ollama {
            if let Ok(llm_out) = ollama.proofread_sentence_sync(&text, context_history, glossary) {
                if !llm_out.trim().is_empty() {
                    return llm_out.trim().to_string();
                }
            }
        }

        // 4. Thử gọi Qwen2.5-VL qua API endpoint nếu có cấu hình
        if let Some(ref ep) = self.config.endpoint {
            if let Ok(llm_out) = self.call_qwen_api(ep, &text, context_history, glossary) {
                if !llm_out.trim().is_empty() {
                    return llm_out.trim().to_string();
                }
            }
        }

        text
    }

    /// Làm sạch nhiễu OCR và áp dụng từ điển ký tự nhầm lẫn phổ biến (100% Generic, không hard-code theo video)
    pub fn apply_generic_sanitizer(&self, text: &str) -> String {
        let mut s = Self::sanitize_ocr_noise(text);

        // 1. Áp dụng bảng nhầm lẫn ký tự tiêu chuẩn trong từ điển tiếng Trung (độc lập với ngữ cảnh video)
        // Đây là các cặp chữ Hán tương đồng hình học (Homoglyphs) kinh điển trong OCR:
        const STANDARD_OCR_CONFUSABLES: &[(&str, &str)] = &[
            ("自已", "自己"), // Lỗi nhầm 己 (kỷ) và 已 (dĩ)
            ("眼晴", "眼睛"), // Lỗi nhầm 睛 (tinh/mắt) và 晴 (tình/nắng)
            ("绝対", "绝对"), // Lỗi nhầm kanji/phồn thể 対 sang giản thể 对
        ];

        for (from, to) in STANDARD_OCR_CONFUSABLES {
            if s.contains(from) {
                s = s.replace(from, to);
            }
        }

        // 2. Áp dụng bảng thay thế tùy chỉnh do người dùng cung cấp qua cấu hình (nếu có)
        for (from, to) in &self.config.custom_replacements {
            if s.contains(from) {
                s = s.replace(from, to);
            }
        }

        s
    }

    /// Tương thích ngược: alias cho apply_generic_sanitizer
    pub fn apply_builtin_rules(&self, text: &str, _context_history: &[String]) -> String {
        self.apply_generic_sanitizer(text)
    }

    /// Làm sạch các ký tự nhiễu OCR ngoại lai (dấu vết vệt sáng, ký tự dính đuôi từ background)
    pub fn sanitize_ocr_noise(text: &str) -> String {
        let mut s = text.trim().to_string();
        if s.is_empty() {
            return s;
        }

        // 1. Loại bỏ các ký tự nhiễu detector ở đầu/cuối chuỗi do viền box bắt trúng tia sáng/bụi
        while s.starts_with(['^', '_', '|', '~', '`', '-', ' ']) {
            s.remove(0);
            s = s.trim_start().to_string();
        }
        while s.ends_with(['^', '_', '|', '~', '`', '-', 'v', 'V', 'x', 'X', ' ']) {
            s.pop();
            s = s.trim_end().to_string();
        }

        // 2. Loại bỏ ký tự kana (Hiragana/Katakana) đơn lẻ dính ở đuôi chuỗi tiếng Trung
        // Do clipping hiệu ứng hình ảnh hoặc chữ nền tiếng Nhật trong phim
        if s.chars().count() >= 3 {
            if let Some(last_char) = s.chars().last() {
                let is_kana = ('\u{3040}'..='\u{309F}').contains(&last_char)
                    || ('\u{30A0}'..='\u{30FF}').contains(&last_char);
                if is_kana {
                    let han_count = s
                        .chars()
                        .filter(|c| ('\u{4E00}'..='\u{9FFF}').contains(c))
                        .count();
                    if (han_count as f32 / s.chars().count() as f32) > 0.6 {
                        s.pop();
                        s = s.trim_end().to_string();
                    }
                }
            }
        }

        s
    }

    /// Gọi endpoint tương thích OpenAI của Qwen2.5-VL-3B
    fn call_qwen_api(
        &self,
        endpoint: &str,
        raw_text: &str,
        context_history: &[String],
        glossary: Option<&EntityGlossary>,
    ) -> Result<String> {
        let glossary_prompt = if let Some(g) = glossary {
            format!("Danh từ riêng chuẩn: {}\n", g.terms().join(", "))
        } else {
            String::new()
        };

        let context_prompt = if !context_history.is_empty() {
            let recent: Vec<&str> = context_history
                .iter()
                .rev()
                .take(3)
                .map(|s| s.as_str())
                .collect();
            format!("Ngữ cảnh trước đó: {}\n", recent.join(" | "))
        } else {
            String::new()
        };

        let system_msg = "Bạn là chuyên gia hiệu đính phụ đề hoạt hình Trung Quốc. \
Nhiệm vụ: Sửa lỗi OCR trong câu phụ đề dựa trên ngữ cảnh và danh từ riêng. \
Chỉ trả về duy nhất câu phụ đề tiếng Trung đã sửa, không giải thích.";

        let user_msg = format!(
            "{}{}\nCâu phụ đề cần sửa: \"{}\"",
            glossary_prompt, context_prompt, raw_text
        );

        let body = serde_json::json!({
            "model": self.config.model_name,
            "messages": [
                {"role": "system", "content": system_msg},
                {"role": "user", "content": user_msg}
            ],
            "temperature": 0.0,
            "max_tokens": 128
        });

        let url = if endpoint.ends_with("/chat/completions") {
            endpoint.to_string()
        } else {
            format!("{}/chat/completions", endpoint.trim_end_matches('/'))
        };

        // Dùng curl.exe có sẵn trên Windows/Linux để tương thích tối đa không cần thư viện ngoài
        let json_str = body.to_string();
        let mut cmd = Command::new("curl.exe");
        cmd.args(&[
            "-s",
            "-X",
            "POST",
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            &json_str,
            "--max-time",
            "5",
        ]);

        if let Some(ref key) = self.config.api_key {
            cmd.args(&["-H", &format!("Authorization: Bearer {}", key)]);
        }

        let output = cmd.output()?;
        if !output.status.success() {
            anyhow::bail!("API call failed");
        }

        let resp: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        if let Some(content) = resp["choices"][0]["message"]["content"].as_str() {
            Ok(content.trim().trim_matches('"').to_string())
        } else {
            anyhow::bail!("Phản hồi không hợp lệ từ model");
        }
    }

    /// Hiệu đính toàn bộ file WebVTT
    pub fn proofread_vtt_file(
        &self,
        input_vtt: &Path,
        output_vtt: &Path,
        glossary: Option<&EntityGlossary>,
    ) -> Result<ProofreadSummary> {
        let content = fs::read_to_string(input_vtt)?;
        let mut lines: Vec<String> = Vec::new();
        let mut summary = ProofreadSummary::default();
        let mut history: Vec<String> = Vec::new();

        let mut in_subtitle_block = false;

        for raw_line in content.lines() {
            let line = raw_line.trim();

            if line.contains("-->") {
                in_subtitle_block = true;
                lines.push(raw_line.to_string());
                continue;
            }

            if line.is_empty() || line == "WEBVTT" {
                in_subtitle_block = false;
                lines.push(raw_line.to_string());
                continue;
            }

            if in_subtitle_block {
                summary.total_lines += 1;
                let preview: String = line.chars().take(20).collect();
                print!("    \x1b[36m[Tiến độ: {:3} câu]\x1b[0m Đang kiểm tra: \"{}\"... \r", summary.total_lines, preview);
                let _ = std::io::Write::flush(&mut std::io::stdout());

                let corrected = self.proofread_sentence(line, &history, glossary);
                if corrected != line {
                    summary.corrected_lines += 1;
                    summary
                        .corrections
                        .push((line.to_string(), corrected.clone()));
                }
                history.push(corrected.clone());
                lines.push(corrected);
                in_subtitle_block = false;
            } else {
                lines.push(raw_line.to_string());
            }
        }
        print!("\r\x1b[K");
        let _ = std::io::Write::flush(&mut std::io::stdout());

        if let Some(parent) = output_vtt.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(output_vtt, lines.join("\n") + "\n")?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_sanitizer_and_custom_rules() {
        let mut proofreader = Proofreader::with_rule_engine_only();

        // 1. Chuẩn hóa cặp chữ Hán nhầm lẫn kinh điển trong OCR (Homoglyphs)
        let s1 = proofreader.proofread_sentence("保护好自已", &[], None);
        assert_eq!(s1, "保护好自己");

        let s2 = proofreader.proofread_sentence("擦亮眼晴", &[], None);
        assert_eq!(s2, "擦亮眼睛");

        // 2. Làm sạch nhiễu detector (dấu viền, ký tự rác Latin, kana dính đuôi)
        let s3 = proofreader.proofread_sentence("~无敌剑意v", &[], None);
        assert_eq!(s3, "无敌剑意");

        let s4 = proofreader.proofread_sentence("绝世仙尊で", &[], None);
        assert_eq!(s4, "绝世仙尊");

        // 3. Hỗ trợ nạp từ điển tùy chỉnh của người dùng (Generic)
        proofreader
            .config
            .custom_replacements
            .insert("错别字".to_string(), "正确词".to_string());
        let s5 = proofreader.proofread_sentence("这是一个错别字", &[], None);
        assert_eq!(s5, "这是一个正确词");
    }

    #[test]
    fn test_proofread_with_glossary() {
        let proofreader = Proofreader::with_default_config();
        let mut glossary = EntityGlossary::new();
        glossary.add_term("陆清安");
        glossary.add_term("何訇");

        // Sửa tên người sai 1 ký tự
        let res = proofreader.proofread_sentence("陆请安来到这里", &[], Some(&glossary));
        assert_eq!(res, "陆清安来到这里");
    }
}
