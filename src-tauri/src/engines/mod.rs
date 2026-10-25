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

pub fn system_prompt(direction: Direction, target: &str) -> String {
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
