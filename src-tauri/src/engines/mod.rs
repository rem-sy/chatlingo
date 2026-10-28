pub mod aliyun;
pub mod deepl;
pub mod openai;
pub mod vision;

use futures_util::Stream;
use std::pin::Pin;

pub struct TranslateParams {
    pub text: String,
    pub target_lang: String,
    /// input = 用户输入方向（口语化、无需保 Markdown）；output = agent 输出方向（保 Markdown/代码）
    pub direction: Direction,
    /// 命中的术语表（source → 固定译法）；各引擎自行决定注入方式
    pub glossary: Vec<(String, String)>,
    /// DeepL 远端术语表资源 id（lib.rs 懒重建后传入；其他引擎忽略）
    pub glossary_id: Option<String>,
    /// 检测出的源语言码（DeepL glossary 翻译必须显式 source_lang）
    pub source_lang: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<String, String>> + Send>>;

pub trait TranslationEngine: Send + Sync {
    /// 流式翻译：逐块产出译文文本；错误以 Err(String) 结束。
    fn translate_stream(&self, params: TranslateParams) -> ChunkStream;
}

/// 语言码 → 英文全名（写进 Prompt，模型对全名的理解远稳于裸码）
pub fn lang_name(code: &str) -> String {
    match code.to_ascii_lowercase().as_str() {
        "en" | "en-us" | "english" => "English".into(),
        "ja" | "jp" | "japanese" => "Japanese".into(),
        "ko" | "korean" => "Korean".into(),
        "zh" | "zh-cn" | "zh-tw" | "chinese" => "Simplified Chinese".into(),
        "fr" | "french" => "French".into(),
        "de" | "german" => "German".into(),
        "es" | "spanish" => "Spanish".into(),
        "ru" | "russian" => "Russian".into(),
        "pt" | "portuguese" => "Portuguese".into(),
        "it" | "italian" => "Italian".into(),
        "th" | "thai" => "Thai".into(),
        "vi" | "vietnamese" => "Vietnamese".into(),
        "ar" | "arabic" => "Arabic".into(),
        "id" | "indonesian" => "Indonesian".into(),
        "hi" | "hindi" => "Hindi".into(),
        other => {
            if other.is_empty() {
                "English".into()
            } else {
                // 未知码原样传入，模型通常也认识（如 "uk"、"ms"）
                other.to_string()
            }
        }
    }
}

pub fn system_prompt(direction: Direction, target: &str) -> String {    let target = lang_name(target);
    match direction {
        Direction::Input => format!(
            "You are a professional translator. Translate the user's message into {target} \
             for an AI assistant to read. The message is casual user input: translate naturally, \
             keep technical terms accurate, keep code snippets, URLs and file paths unchanged. \
             If the message is already mostly in {target}, return it unchanged. \
             Output ONLY the translation, nothing else. No quotes, no explanations."
        ),
        Direction::Output => format!(
            "You are a professional translator. Translate the given AI assistant's reply into {target}. \
             STRICTLY preserve the Markdown structure: headings, lists, tables, bold/italic, links. \
             Keep fenced code blocks completely UNCHANGED (do not translate code or comments inside them). \
             Keep inline code, URLs, command lines, API names and identifiers unchanged. \
             Translate the prose accurately and naturally. \
             Output ONLY the translated Markdown, nothing else."
        ),
    }
}

/// 大模型 system prompt 的术语注入段（空术语表返回空串）
pub fn glossary_prompt_block(terms: &[(String, String)]) -> String {
    if terms.is_empty() {
        return String::new();
    }
    let lines: Vec<String> = terms.iter().map(|(s, t)| format!("- {s} → {t}")).collect();
    format!(
        "\n\nGlossary (MUST be followed strictly: translate these exact terms using the given \
         renderings whenever they appear, in either direction):\n{}",
        lines.join("\n")
    )
}

/// 阿里云 Context 参数拼接：术语对照：RAG=检索增强生成；…（超长截断）
pub fn glossary_context(terms: &[(String, String)]) -> String {
    if terms.is_empty() {
        return String::new();
    }
    let pairs: Vec<String> = terms.iter().map(|(s, t)| format!("{s}={t}")).collect();
    let mut ctx = format!("术语对照：{}", pairs.join("；"));
    // 阿里云 Context 有限长，保守截 900 字符
    if ctx.chars().count() > 900 {
        ctx = ctx.chars().take(900).collect();
    }
    ctx
}
