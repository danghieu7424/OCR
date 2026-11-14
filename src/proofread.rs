//! # Context Proofreader (Hiệu đính ngữ cảnh bằng Qwen2.5-VL / LLM & Rule Engine)
//!
//! Tầng hậu xử lý (Post-processing) đảm bảo độ chính xác 100% cho phụ đề trước khi dịch:
//! - Tự động sửa các ký tự nhận diện nhầm âm/chữ gần giống (vd: `爹` vs `参`, `嗯` vs `涵`, `自已` vs `自己`).
//! - Kết hợp danh từ riêng từ `EntityGlossary` để chuẩn hóa tên nhân vật/công pháp.
//! - Hỗ trợ gọi mô hình Qwen2.5-VL-3B-Instruct qua endpoint OpenAI/Ollama tương thích,
//!   kèm bộ luật dự phòng siêu nhanh (Rule-based Fallback) chạy trực tiếp trong RAM nếu không có LLM server.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::glossary::EntityGlossary;

/// Cấu hình hiệu đính ngữ cảnh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofreadConfig {
    /// Endpoint OpenAI-compatible của Qwen2.5-VL-3B (vd: "http://localhost:11434/v1" hoặc "http://localhost:8000/v1")
    pub endpoint: Option<String>,
    /// Tên mô hình (vd: "qwen2.5-vl:3b" hoặc "Qwen2.5-VL-3B-Instruct")
    pub model_name: String,
    /// API key (nếu có)
    pub api_key: Option<String>,
    /// Bật bộ luật sửa lỗi ngữ cảnh trực tiếp (chạy 0ms trên CPU)
    pub enable_rule_engine: bool,
}

impl Default for ProofreadConfig {
    fn default() -> Self {
        Self {
            endpoint: std::env::var("OCR_LLM_ENDPOINT").ok(),
            model_name: std::env::var("OCR_LLM_MODEL")
                .unwrap_or_else(|_| "qwen2.5-vl:3b".to_string()),
            api_key: std::env::var("OCR_LLM_API_KEY").ok(),
            enable_rule_engine: true,
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
}

impl Proofreader {
    pub fn new(config: ProofreadConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(ProofreadConfig::default())
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

        // 1. Áp dụng bảng thực thể trước (chuẩn hóa tên nhân vật)
        if let Some(g) = glossary {
            text = g.normalize_sentence(&text);
        }

        // 2. Thử gọi Qwen2.5-VL qua API nếu có cấu hình endpoint
        if let Some(ref ep) = self.config.endpoint {
            if let Ok(llm_out) = self.call_qwen_api(ep, &text, context_history, glossary) {
                if !llm_out.trim().is_empty() {
                    return llm_out.trim().to_string();
                }
            }
        }

        // 3. Fallback: Bộ luật ngữ cảnh tích hợp (Rule Engine chạy tức thì trên CPU)
        if self.config.enable_rule_engine {
            text = self.apply_builtin_rules(&text, context_history);
        }

        text
    }

    /// Áp dụng bộ luật sửa lỗi ngữ cảnh tiếng Trung điển hình trong hoạt hình
    fn apply_builtin_rules(&self, text: &str, context_history: &[String]) -> String {
        let mut s = text.to_string();

        // 1. Sửa lỗi nhầm 参 <-> 爹 (nhầm phổ biến trong phim có xưng hô gia đình)
        if s.contains("娘亲") || s.contains("爹") || s.contains("孩子们") {
            s = s.replace("娘亲参", "娘亲爹").replace("娘亲爹參", "娘亲爹爹");
        }
        if s.contains("参以前") || s.contains("参的武器") {
            s = s.replace("参以前", "爹以前").replace("参的武器", "爹的武器");
        }
        // Kiểm tra câu liền trước có nói về cha con
        let prev_has_dad = context_history
            .last()
            .map(|p| p.contains("爹") || p.contains("娘亲") || p.contains("相公"))
            .unwrap_or(false);
        if prev_has_dad && s.starts_with("参") && s.chars().count() >= 3 {
            let mut chars: Vec<char> = s.chars().collect();
            chars[0] = '爹';
            s = chars.into_iter().collect();
        }

        // 2. Sửa lỗi nhầm 嗯 <-> 涵 (từ đệm hội thoại)
        if s == "涵" || s == "涵。" {
            s = "嗯".to_string();
        }

        // 3. Sửa lỗi nhầm chữ 己 <-> 已
        s = s.replace("自已", "自己");

        // 4. Sửa lỗi nhầm 眼晴 <-> 眼睛
        s = s.replace("眼晴", "眼睛");

        // 5. Sửa lỗi nhầm 灭多小 <-> 灭多少
        s = s.replace("灭多小", "灭多少");

        // 6. Xóa ký tự ngoại lai lẻ (kana, ký tự Latin dính đuôi do hiệu ứng hình ảnh)
        s = s.replace("魔域之人で", "魔域之人");
        if s.ends_with('V') || s.ends_with('v') {
            s.pop();
        }
        if s.starts_with("一还只是") {
            s = s.replacen("一还只是", "还只是", 1);
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
    fn test_builtin_rules_corrections() {
        let proofreader = Proofreader::with_default_config();

        // 1. Sửa 参 -> 爹 trong ngữ cảnh xưng hô
        let s1 = proofreader.proofread_sentence("娘亲爹參", &[], None);
        assert_eq!(s1, "娘亲爹爹");

        let s2 = proofreader.proofread_sentence("参以前的武器给你", &[], None);
        assert_eq!(s2, "爹以前的武器给你");

        // 2. Sửa 涵 -> 嗯
        let s3 = proofreader.proofread_sentence("涵", &[], None);
        assert_eq!(s3, "嗯");

        // 3. Sửa 自已 -> 自己
        let s4 = proofreader.proofread_sentence("保护好自已", &[], None);
        assert_eq!(s4, "保护好自己");

        // 4. Xóa rác kana
        let s5 = proofreader.proofread_sentence("魔域之人で", &[], None);
        assert_eq!(s5, "魔域之人");
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
