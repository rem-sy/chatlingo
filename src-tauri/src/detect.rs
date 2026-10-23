/// 基于脚本的零成本语言检测启发式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Zh,
    Ja,
    Ko,
    Latin,
    Other,
}

fn char_stats(text: &str) -> (f64, f64, f64, f64) {
    // 返回 (汉字占比, 假名占比, 谚文占比, 拉丁字母占比)，按非空白字符计
    let mut total = 0usize;
    let mut han = 0usize;
    let mut kana = 0usize;
    let mut hangul = 0usize;
    let mut latin = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        total += 1;
        let cp = ch as u32;
        if (0x4E00..=0x9FFF).contains(&cp)
            || (0x3400..=0x4DBF).contains(&cp)
            || (0xF900..=0xFAFF).contains(&cp)
        {
            han += 1;
        } else if (0x3040..=0x30FF).contains(&cp) {
            kana += 1;
        } else if (0xAC00..=0xD7AF).contains(&cp) || (0x1100..=0x11FF).contains(&cp) {
            hangul += 1;
        } else if ch.is_ascii_alphabetic() {
            latin += 1;
        }
    }
    let t = total.max(1) as f64;
    (
        han as f64 / t,
        kana as f64 / t,
        hangul as f64 / t,
        latin as f64 / t,
    )
}

pub fn detect(text: &str) -> Lang {
    let (han, kana, hangul, latin) = char_stats(text);
    if kana > 0.10 {
        return Lang::Ja;
    }
    if hangul > 0.20 {
        return Lang::Ko;
    }
    if han > 0.25 {
        return Lang::Zh;
    }
    if latin > 0.40 {
        return Lang::Latin;
    }
    Lang::Other
}

/// 该文本是否已经基本是目标语言（无需翻译）
pub fn is_already_target(text: &str, target: &str) -> bool {
    let lang = detect(text);
    match target.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => matches!(lang, Lang::Zh),
        "ja" | "jp" | "japanese" => matches!(lang, Lang::Ja),
        "ko" | "korean" => matches!(lang, Lang::Ko),
        "en" | "english" => matches!(lang, Lang::Latin),
        _ => false,
    }
}

/// 文本中代码围栏占比是否过高（视为不需要翻译）
pub fn code_dominant(text: &str) -> bool {
    let fence_count = text.matches("```").count() / 2;
    if fence_count == 0 {
        return false;
    }
    let code_len: usize = text
        .split("```")
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s.len())
        .sum();
    code_len * 2 > text.len()
}
