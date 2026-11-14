//! # Multi-frame Temporal Voting (Bỏ phiếu đa khung hình)
//!
//! Thuật toán bỏ phiếu cấp chuỗi và cấp ký tự (Character-level Majority Voting)
//! trên nhiều khung hình liên tiếp để loại bỏ nhiễu cục bộ (bùa chú, tia sáng,
//! ký tự nhận diện nhầm tạm thời) và khôi phục văn bản phụ đề/chú thích chuẩn xác nhất.

use std::collections::HashMap;

/// Bỏ phiếu cấp chuỗi (String-level Voting): chọn chuỗi có tần suất xuất hiện cao nhất.
/// Nếu hòa phiếu, ưu tiên chuỗi có độ dài lớn hơn hoặc xuất hiện sớm hơn.
pub fn vote_majority_string<S: AsRef<str>>(candidates: &[S]) -> String {
    if candidates.is_empty() {
        return String::new();
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    let mut first_seen: HashMap<&str, usize> = HashMap::new();

    for (idx, item) in candidates.iter().enumerate() {
        let s = item.as_ref().trim();
        if !s.is_empty() {
            *counts.entry(s).or_insert(0) += 1;
            first_seen.entry(s).or_insert(idx);
        }
    }

    if counts.is_empty() {
        return String::new();
    }

    counts
        .into_iter()
        .max_by(|(s1, c1), (s2, c2)| {
            c1.cmp(c2)
                .then_with(|| s1.chars().count().cmp(&s2.chars().count()))
                .then_with(|| {
                    let idx1 = first_seen.get(s1).unwrap_or(&usize::MAX);
                    let idx2 = first_seen.get(s2).unwrap_or(&usize::MAX);
                    idx2.cmp(idx1) // ưu tiên xuất hiện sớm hơn
                })
        })
        .map(|(s, _)| s.to_string())
        .unwrap_or_default()
}

/// Bỏ phiếu cấp ký tự (Character-level Consensus Alignment):
///
/// Khi các khung hình có biến thể do hiệu ứng che khuất (vd: `["挡一", "挡住", "挡住", "星 挡住是本朱"]`),
/// thuật toán căn chỉnh các chuỗi dựa trên chuỗi lõi (dominant sequence) và bầu chọn từng ký tự.
/// Các ký tự "rác" lướt qua dưới `min_presence_ratio` (mặc định 0.35) sẽ bị triệt tiêu.
pub fn vote_character_consensus<S: AsRef<str>>(
    candidates: &[S],
    min_presence_ratio: f32,
) -> String {
    let valid_cands: Vec<&str> = candidates
        .iter()
        .map(|s| s.as_ref().trim())
        .filter(|s| !s.is_empty())
        .collect();

    if valid_cands.is_empty() {
        return String::new();
    }
    if valid_cands.len() == 1 {
        return valid_cands[0].to_string();
    }

    // 1. Tìm chuỗi nòng cốt (pivot) có độ tương đồng trung bình cao nhất với các chuỗi khác
    let mut best_pivot = valid_cands[0];
    let mut best_score = -1.0f32;

    for &cand in &valid_cands {
        let mut sim_sum = 0.0f32;
        for &other in &valid_cands {
            sim_sum += char_similarity(cand, other);
        }
        if sim_sum > best_score {
            best_score = sim_sum;
            best_pivot = cand;
        }
    }

    let pivot_chars: Vec<char> = best_pivot.chars().collect();
    let total_frames = valid_cands.len();
    let min_votes = ((total_frames as f32) * min_presence_ratio.clamp(0.1, 1.0)).ceil() as usize;

    // 2. Với mỗi vị trí trong pivot, căn chỉnh các ký tự từ các chuỗi khác vào vị trí tương ứng
    let mut consensus = String::new();

    for (pos, &pivot_ch) in pivot_chars.iter().enumerate() {
        let mut char_votes: HashMap<char, usize> = HashMap::new();
        char_votes.insert(pivot_ch, 1);

        for &cand in &valid_cands {
            if cand == best_pivot {
                continue;
            }
            let cand_chars: Vec<char> = cand.chars().collect();
            // Tìm ký tự khớp tốt nhất trong cửa sổ lân cận pos +/- 2
            let start = pos.saturating_sub(2);
            let end = (pos + 3).min(cand_chars.len());

            let mut matched_char = None;
            for idx in start..end {
                if cand_chars[idx] == pivot_ch {
                    matched_char = Some(pivot_ch);
                    break;
                }
            }

            if let Some(ch) = matched_char {
                *char_votes.entry(ch).or_insert(0) += 1;
            } else if pos < cand_chars.len() {
                // Nếu không có pivot_ch, ghi nhận ký tự tại vị trí tương đương
                *char_votes.entry(cand_chars[pos]).or_insert(0) += 1;
            }
        }

        // Chọn ký tự có phiếu cao nhất tại vị trí này
        if let Some((&winner_char, &votes)) = char_votes.iter().max_by_key(|&(_, v)| *v) {
            if votes >= min_votes.max(1) {
                consensus.push(winner_char);
            }
        }
    }

    if consensus.is_empty() {
        vote_majority_string(&valid_cands)
    } else {
        consensus
    }
}

/// Tính độ tương đồng ký tự giữa 2 chuỗi (dựa trên Levenshtein distance)
fn char_similarity(s1: &str, s2: &str) -> f32 {
    let c1: Vec<char> = s1.chars().collect();
    let c2: Vec<char> = s2.chars().collect();
    let max_len = c1.len().max(c2.len());
    if max_len == 0 {
        return 1.0;
    }
    let dist = levenshtein(&c1, &c2);
    1.0 - (dist as f32 / max_len as f32)
}

fn levenshtein(c1: &[char], c2: &[char]) -> usize {
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
    fn test_vote_majority_string() {
        let frames = vec!["挡住", "挡一", "挡住", "挡住", "挡"];
        assert_eq!(vote_majority_string(&frames), "挡住");
    }

    #[test]
    fn test_vote_character_consensus_noise_removal() {
        let frames = vec![
            "爹以前的武器给你",
            "爹以前的武器给你",
            "参以前的武器给你",
            "爹以前的武器给你",
            "爹以前的武器给你V",
        ];
        let voted = vote_character_consensus(&frames, 0.4);
        assert_eq!(voted, "爹以前的武器给你");
    }

    #[test]
    fn test_vote_character_consensus_bua_chu() {
        let frames = vec![
            "挡住",
            "挡一",
            "挡住",
            "星 挡住是本朱",
            "挡住",
        ];
        let voted = vote_character_consensus(&frames, 0.4);
        assert_eq!(voted, "挡住");
    }
}
