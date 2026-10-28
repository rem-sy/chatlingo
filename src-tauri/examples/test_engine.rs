use chatlingo_lib::engines::openai::OpenAiCompatEngine;
use chatlingo_lib::engines::{Direction, TranslationEngine, TranslateParams};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let engine = OpenAiCompatEngine::new(
        "https://windhub.cc/v1".into(),
        "REDACTED-REVOKED-KEY".into(),
        "glm-4.5-flash".into(),
    );
    // 先测 quick_check（GET /models）
    match engine.quick_check().await {
        Ok(()) => println!("[models] OK"),
        Err(e) => println!("[models] ERR: {e}"),
    }
    let mut stream = engine.translate_stream(TranslateParams {
        text: "The retry mechanism uses exponential backoff with jitter.".into(),
        target_lang: "zh-CN".into(),
        direction: Direction::Output,
        glossary: vec![],
        glossary_id: None,
        source_lang: None,
    });
    let mut got = 0;
    while let Some(item) = stream.next().await {
        match item {
            Ok(c) => got += c.chars().count(),
            Err(e) => {
                println!("[translate] ERR: {e}");
                return;
            }
        }
    }
    println!("[translate] OK, {got} chars");
}
