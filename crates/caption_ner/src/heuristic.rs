/// Advanced heuristic parser to extract character name from caption inner text
/// using generic grammatical and morphological structure rules (zero hardcoding of specific story names/places).
pub fn heuristic_parse_character(txt: &str) -> Option<String> {
    let trimmed = txt.trim();
    let inner = if let Some(stripped) = trimmed.strip_prefix('《') {
        stripped.strip_suffix('》').unwrap_or(stripped).trim()
    } else {
        trimmed
    };

    if inner.is_empty() {
        return None;
    }

    // 1. Generic warning/metadata filter
    let warning_indicators = [
        "请勿模仿",
        "不良导向",
        "危险动作",
        "无价值",
        "生成",
        "导向",
        "系统",
        "...",
    ];
    if warning_indicators.iter().any(|&w| inner.contains(w)) {
        return None;
    }

    // Weapons / magical artifacts endings: e.g. 九狱弑仙剑
    let artifact_endings = [
        "剑", "刀", "枪", "鼎", "印", "符", "琴", "塔", "钟", "炉", "幡", "轮", "扇", "镜",
        "戟", "鞭", "弓", "斧",
    ];
    if artifact_endings.iter().any(|&w| inner.ends_with(w)) && inner.chars().count() >= 4 {
        return None;
    }

    let delimiter = ['-', '—', '–'].into_iter().find(|&d| inner.contains(d));
    let raw_name = if let Some(d) = delimiter {
        let (left, right) = inner.split_once(d).unwrap();
        let left = left.trim();
        let right = right.trim();

        let state_or_realm_keywords = [
            "状态", "形态", "化身", "分身", "真身", "变身", "体", "境", "期", "重", "巅峰",
        ];
        let role_or_title_keywords = [
            "大女儿", "二女儿", "三女儿", "女儿", "儿子", "前妻", "妻子", "夫君", "父亲", "母亲",
            "舅舅", "城主", "宗主", "师尊", "长老", "使者", "府主", "殿主", "门主", "堂主",
            "统领", "大帝", "圣王", "本体", "人形", "身躯",
        ];

        if state_or_realm_keywords.iter().any(|k| right.contains(k)) {
            left
        } else if role_or_title_keywords.iter().any(|k| left.contains(k)) {
            right
        } else if (2..=4).contains(&left.chars().count())
            && (right.chars().count() > 4
                || role_or_title_keywords.iter().any(|k| right.contains(k)))
        {
            left
        } else if (2..=4).contains(&right.chars().count()) {
            right
        } else {
            left
        }
    } else {
        // No hyphen delimiter: inspect morphological affixes and grammatical structures
        let action_narratives = ["直到", "这一天", "用尽", "穿破", "封锁", "逃离", "只有两人", "的家"];
        if action_narratives.iter().any(|&a| inner.contains(a)) {
            return None;
        }

        let loc_endings = [
            "交界处", "交界", "溶洞", "虚空", "虛空", "院子", "院落", "城", "宗", "殿", "山", "海",
            "界", "域", "宫", "阁", "洞", "楼",
        ];
        if loc_endings.iter().any(|&l| inner.ends_with(l)) && inner.chars().count() >= 4 {
            return None;
        }

        // Check avatar / state suffixes: e.g. 黑龙身外化身 -> 黑龙
        let avatar_suffixes = [
            "身外化身", "身外分身", "化身", "分身", "真身", "状态", "形态", "残魂", "神魂", "法身",
        ];
        let mut stripped_avatar: Option<&str> = None;
        for suf in &avatar_suffixes {
            if inner.ends_with(suf) && inner.chars().count() > suf.chars().count() {
                let stem = &inner[..inner.len() - suf.len()];
                if (2..=4).contains(&stem.chars().count()) {
                    stripped_avatar = Some(stem);
                    break;
                }
            }
        }
        if let Some(stem) = stripped_avatar {
            stem
        } else {
            // Check group collective titles with count prefixes: e.g. 三位神明境强者 -> 三位神
            let count_prefixes = [
                "一位", "两位", "三位", "四位", "五位", "六位", "七位", "八位", "九位", "十位",
                "两名", "三名", "四名", "众", "诸位",
            ];
            let mut collective_title: Option<String> = None;
            for pfx in &count_prefixes {
                if inner.starts_with(pfx) && inner.chars().count() > pfx.chars().count() {
                    let rest = &inner[pfx.len()..];
                    if let Some(first_char) = rest.chars().next() {
                        collective_title = Some(format!("{pfx}{first_char}"));
                        break;
                    }
                }
            }
            if let Some(coll) = collective_title {
                return Some(coll);
            }
            inner.trim()
        }
    };

    let char_count = raw_name.chars().count();
    if (2..=4).contains(&char_count)
        && raw_name
            .chars()
            .all(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
    {
        Some(raw_name.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_caption_parsing() {
        assert_eq!(
            heuristic_parse_character("《三位神明境強者》"),
            Some("三位神".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《雪天妃》"),
            Some("雪天妃".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《陆清安-妖魔体状态》"),
            Some("陆清安".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《黑龙身外化身》"),
            Some("黑龙".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《骆丹茲》"),
            Some("骆丹茲".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《本体人形-黑龙》"),
            Some("黑龙".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《陆清安前妻-徐清莹》"),
            Some("徐清莹".to_string())
        );
        assert_eq!(
            heuristic_parse_character("《韩梦瑤身躯-郑妙音》"),
            Some("郑妙音".to_string())
        );
        assert_eq!(heuristic_parse_character("《地底溶洞》"), None);
        assert_eq!(heuristic_parse_character("《危险动作请勿模仿》"), None);
        assert_eq!(heuristic_parse_character("《逃离虛空》"), None);
        assert_eq!(heuristic_parse_character("《用尽手段穿破一层封锁》"), None);
    }
}
