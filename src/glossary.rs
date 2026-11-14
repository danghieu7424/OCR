//! # Entity Glossary (Từ điển Danh từ riêng & Thực thể)
//!
//! Tự động trích xuất các thực thể (Tên nhân vật, Địa danh, Công pháp, Chức vị)
//! từ kết quả Caption (`《...》`) và áp dụng để chuẩn hóa phụ đề, loại bỏ lỗi OCR
//! do nét mờ hoặc chữ hiếm.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Bảng danh từ riêng tự động tạo từ Caption
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EntityGlossary {
    /// Danh sách các thực thể chuẩn (canonical terms)
    pub terms: Vec<String>,
}

impl EntityGlossary {
    pub fn new() -> Self {
        Self { terms: Vec::new() }
    }

    /// Thêm một thực thể vào từ điển (bỏ ngoặc 《》 và tự tách các cột con nếu có dấu `-`)
    pub fn add_term(&mut self, raw: &str) {
        let cleaned = raw
            .trim()
            .trim_start_matches('《')
            .trim_end_matches('》')
            .trim_end_matches("...")
            .trim_end_matches('…')
            .trim();

        if cleaned.is_empty() {
            return;
        }

        // Thêm chuỗi đầy đủ
        if !self.terms.contains(&cleaned.to_string()) {
            self.terms.push(cleaned.to_string());
        }

        // Nếu có dấu gạch ngang (vd: 《大女儿-陆紫萱》, 《徐清莹弟弟-徐轩宇》)
        // thì tách thêm từng vế độc lập, nhưng lọc bỏ các từ danh xưng/quan hệ chung
        if cleaned.contains('-') || cleaned.contains('—') {
            let sep = if cleaned.contains('-') { '-' } else { '—' };
            for part in cleaned.split(sep) {
                let p = part.trim();
                // Bỏ qua các danh từ chung/quan hệ nhân thân để tránh fuzzy match sai
                // (vd: "小女儿", "弟弟", "妹妹", "城主", "强者", "院落")
                let is_generic_title = matches!(
                    p,
                    "大女儿" | "小女儿" | "老二" | "弟弟" | "妹妹" | "城主" | "强者" | "院落" | "交界处"
                );
                if !is_generic_title && p.chars().count() >= 2 && !self.terms.contains(&p.to_string()) {
                    self.terms.push(p.to_string());
                }
            }
        }
    }

    /// Trích xuất tự động từ danh sách file WebVTT chú thích (`.captions.zh.vtt`)
    pub fn from_caption_files<P: AsRef<Path>>(files: &[P]) -> Result<Self> {
        let mut glossary = Self::new();
        for file in files {
            let path = file.as_ref();
            if !path.exists() {
                continue;
            }
            let content = fs::read_to_string(path)
                .with_context(|| format!("Không thể đọc file caption: {:?}", path))?;

            for line in content.lines() {
                let l = line.trim();
                if l.starts_with('《') && l.ends_with('》') {
                    glossary.add_term(l);
                } else if let (Some(s), Some(e)) = (l.find('《'), l.find('》')) {
                    if e > s {
                        glossary.add_term(&l[s..=e]);
                    }
                }
            }
        }
        // Sắp xếp các từ dài lên trước để ưu tiên so khớp chuỗi dài
        glossary.terms.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
        glossary.terms.dedup();
        Ok(glossary)
    }

    /// Trích xuất từ chuỗi nội dung WebVTT
    pub fn from_vtt_content(vtt_content: &str) -> Self {
        let mut glossary = Self::new();
        for line in vtt_content.lines() {
            let l = line.trim();
            if l.starts_with('《') && l.ends_with('》') {
                glossary.add_term(l);
            } else if let (Some(s), Some(e)) = (l.find('《'), l.find('》')) {
                if e > s {
                    glossary.add_term(&l[s..=e]);
                }
            }
        }
        glossary.terms.sort_by(|a, b| b.chars().count().cmp(&a.chars().count()));
        glossary.terms.dedup();
        glossary
    }

    /// Danh sách thực thể hiện có
    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Chuẩn hóa câu phụ đề dựa trên bảng thực thể:
    /// Tìm kiếm các đoạn trong câu có khoảng cách sai khác nhỏ (dist <= 1) với thực thể chuẩn
    /// và thay thế bằng dạng chuẩn.
    pub fn normalize_sentence(&self, sentence: &str) -> String {
        if sentence.trim().is_empty() || self.terms.is_empty() {
            return sentence.to_string();
        }

        let mut result = sentence.to_string();
        for canonical in &self.terms {
            let c_len = canonical.chars().count();
            // Chỉ fuzzy match cho thực thể từ 3 ký tự trở lên để tránh nhầm từ phổ thông
            // (vd: không để "老巫" bị nắn thành "老二", hoặc "何其" bị nắn thành "何訇")
            if c_len < 3 {
                continue;
            }

            // Nếu câu đã chứa đúng thực thể canonical, bỏ qua
            if result.contains(canonical) {
                continue;
            }

            // Quét qua các cửa sổ con trong câu có độ dài c_len
            let chars: Vec<char> = result.chars().collect();
            if chars.len() < c_len {
                continue;
            }

            let canonical_chars: Vec<char> = canonical.chars().collect();
            let mut best_match: Option<(usize, usize, usize)> = None; // (start, end, dist)

            for i in 0..=(chars.len() - c_len) {
                // Với tên riêng 3 chữ, chữ đầu (họ như 陆, 徐, 林) phải khớp để tránh sửa nhầm
                if c_len == 3 && chars[i] != canonical_chars[0] {
                    continue;
                }

                let sub: String = chars[i..i + c_len].iter().collect();

                // Tránh thay thế nhầm giữa các cảnh giới tu tiên (sơ kỳ, trung kỳ, hậu kỳ, đỉnh phong)
                if sub.contains("初期") || sub.contains("后期") || sub.contains("巅峰") || sub.contains("中期") {
                    if canonical.contains("初期") || canonical.contains("后期") || canonical.contains("巅峰") || canonical.contains("中期") {
                        continue;
                    }
                }

                let dist = levenshtein_chars(&sub, canonical);
                // Nếu chỉ sai 1 ký tự (vd: 陆请安 -> 陆清安, 徐轩宇 -> 徐轩宇)
                if dist == 1 {
                    match best_match {
                        Some((_, _, old_dist)) if dist < old_dist => {
                            best_match = Some((i, i + c_len, dist));
                        }
                        None => {
                            best_match = Some((i, i + c_len, dist));
                        }
                        _ => {}
                    }
                }
            }

            if let Some((start, end, _)) = best_match {
                let mut new_chars = Vec::new();
                new_chars.extend_from_slice(&chars[..start]);
                new_chars.extend(canonical.chars());
                new_chars.extend_from_slice(&chars[end..]);
                result = new_chars.into_iter().collect();
            }
        }

        result
    }

    /// Lưu từ điển ra file JSON
    pub fn save_json<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Nạp từ điển từ file JSON
    pub fn load_json<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let glossary: Self = serde_json::from_str(&content)?;
        Ok(glossary)
    }
}

fn levenshtein_chars(s1: &str, s2: &str) -> usize {
    let c1: Vec<char> = s1.chars().collect();
    let c2: Vec<char> = s2.chars().collect();
    let (l1, l2) = (c1.len(), c2.len());
    if l1 == 0 { return l2; }
    if l2 == 0 { return l1; }
    let mut prev: Vec<usize> = (0..=l2).collect();
    let mut curr: Vec<usize> = vec![0; l2 + 1];

    for i in 1..=l1 {
        curr[0] = i;
        for j in 1..=l2 {
            let cost = if c1[i - 1] == c2[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    curr[l2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_vtt() {
        let vtt = r#"WEBVTT

00:00:03.200 --> 00:00:05.100 line:20%
《陆清安》

00:01:11.200 --> 00:01:12.500 line:20%
《大女儿-陆紫萱》
"#;
        let glossary = EntityGlossary::from_vtt_content(vtt);
        assert!(glossary.terms.contains(&"陆清安".to_string()));
        assert!(glossary.terms.contains(&"大女儿-陆紫萱".to_string()));
        assert!(glossary.terms.contains(&"陆紫萱".to_string()));
        // "大女儿" là danh xưng quan hệ nên được lọc bỏ khỏi từ điển fuzzy match
        assert!(!glossary.terms.contains(&"大女儿".to_string()));
    }

    #[test]
    fn test_normalize_sentence() {
        let mut glossary = EntityGlossary::new();
        glossary.add_term("陆清安");
        glossary.add_term("九狱弑仙剑");

        // "陆请安" sai 1 chữ -> nắn về "陆清安"
        let sentence = "这是陆请安的剑";
        let normalized = glossary.normalize_sentence(sentence);
        assert_eq!(normalized, "这是陆清安的剑");
    }
}
