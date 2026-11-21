/// Advanced heuristic parser to extract character name from caption inner text.
pub fn heuristic_parse_character(txt: &str) -> Option<String> {
    let trimmed = txt.trim();
    let inner = if let Some(stripped) = trimmed.strip_prefix('《') {
        stripped.strip_suffix('》').unwrap_or(stripped)
    } else {
        trimmed
    };

    // Filter out scenes, locations, weapons, realm descriptors, actions
    let non_person_indicators = [
        "...",
        "只有两人",
        "这一天",
        "直到",
        "的家",
        "院落",
        "境界",
        "神明境",
        "交界",
        "交界处",
        "记忆",
        "封锁",
        "突破",
        "剑",
        "装",
        "刀",
        "枪",
        "鼎",
        "印",
        "符",
        "秘术",
        "强者",
        "城",
        "域",
        "山",
        "殿",
        "宗",
        "门",
        "仙墟",
        "魔域",
        "交界",
        "系统",
        "溶洞",
        "虚空",
        "虛空",
        "逃离",
        "人形",
        "生成",
        "妈妈",
        "爸爸",
        "爷爷",
        "奶奶",
    ];

    let delimiter = ['-', '—', '–'].into_iter().find(|&d| inner.contains(d));
    let raw_name = if let Some(d) = delimiter {
        let (left, right) = inner.split_once(d).unwrap();
        let left = left.trim();
        let right = right.trim();

        let state_or_realm_keywords = [
            "境", "期", "巅峰", "重", "状态", "形态", "化身", "分身", "真身", "变身", "体",
        ];
        let title_keywords = [
            "大女儿",
            "二女儿",
            "三女儿",
            "女儿",
            "妻子",
            "夫君",
            "儿子",
            "父亲",
            "母亲",
            "城主",
            "宗主",
            "师尊",
            "长老",
            "使者",
            "府主",
            "殿主",
            "门主",
            "堂主",
            "统领",
        ];

        if state_or_realm_keywords.iter().any(|k| right.contains(k)) {
            left
        } else if title_keywords.iter().any(|k| left.contains(k)) {
            right
        } else if (2..=4).contains(&left.chars().count()) {
            left
        } else {
            right
        }
    } else {
        inner.trim()
    };

    for indicator in &non_person_indicators {
        if raw_name.contains(indicator) {
            return None;
        }
    }

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
