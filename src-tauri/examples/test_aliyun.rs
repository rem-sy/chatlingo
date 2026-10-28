use chatlingo_lib::engines::aliyun::AliyunEngine;
use chatlingo_lib::engines::{Direction, TranslationEngine, TranslateParams};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let keys = std::fs::read_to_string(r"C:\temp\chatlingo\ak.txt").unwrap();
    let mut lines = keys.lines();
    let id = lines.next().unwrap().trim().to_string();
    let secret = lines.next().unwrap().trim().to_string();
    let engine = AliyunEngine::new(id, secret);
    match engine.quick_check().await {
        Ok(()) => println!("[check] OK"),
        Err(e) => println!("[check] ERR: {e}"),
    }
    let mut stream = engine.translate_stream(TranslateParams {
        text: "The retry mechanism uses exponential backoff with jitter.".into(),
        target_lang: "zh-CN".into(),
        direction: Direction::Output,
        glossary: vec![],
        glossary_id: None,
        source_lang: None,
    });
    while let Some(item) = stream.next().await {
        match item {
            Ok(c) => println!("[translate] chunk: {c}"),
            Err(e) => { println!("[translate] ERR: {e}"); return; }
        }
    }
    println!("[translate] done");
}
