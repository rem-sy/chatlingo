use chatlingo_lib::engines::openai::OpenAiCompatEngine;
use chatlingo_lib::engines::{Direction, TranslationEngine, TranslateParams};
use futures_util::StreamExt;

/// Key 不入库：优先环境变量 CHATLINGO_TEST_KEY，否则读本机 config.json（同 test_glossary_llm）
fn load_key() -> (String, String, String) {
    if let Ok(k) = std::env::var("CHATLINGO_TEST_KEY") {
        let base = std::env::var("CHATLINGO_TEST_BASE").unwrap_or_else(|_| "https://windhub.cc/v1".into());
        let model = std::env::var("CHATLINGO_TEST_MODEL").unwrap_or_else(|_| "glm-4.5-flash".into());
        return (base, k, model);
    }
    let cfg_path = dirs_home_config();
    let raw = std::fs::read_to_string(&cfg_path).unwrap_or_else(|_| panic!("无 CHATLINGO_TEST_KEY 且读不到 {cfg_path}"));
    let v: serde_json::Value = serde_json::from_str(&raw).expect("config.json 解析失败");
    (
        v["engine"]["baseUrl"].as_str().unwrap_or_default().into(),
        v["engine"]["apiKey"].as_str().unwrap_or_default().into(),
        v["engine"]["model"].as_str().unwrap_or_default().into(),
    )
}

fn dirs_home_config() -> String {
    let p = std::path::PathBuf::from(
        std::fs::read_to_string(r"D:\hobby\产物\chatlingo\v0.2.6\data\data-location")
            .map(|s| s.trim().to_string())
            .unwrap_or_default(),
    )
    .join("config.json");
    if p.exists() {
        return p.to_string_lossy().into();
    }
    // 指针文件不在（默认安装）：%APPDATA%\com.remsy.chatlingo\config.json
    format!(r"{}\com.remsy.chatlingo\config.json", std::env::var("APPDATA").unwrap_or_default())
}

#[tokio::main]
async fn main() {
    let (base, key, model) = load_key();
    let engine = OpenAiCompatEngine::new(base, key, model);
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
