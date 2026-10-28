//! M5 验收：LLM 术语注入——「RAG → 检索增强生成」注入后译法一致。
//! Key 从本机 config.json 读（数据目录指针 %APPDATA%\com.remsy.chatlingo\data-location）。
use chatlingo_lib::config::AppConfig;
use chatlingo_lib::engines::openai::OpenAiCompatEngine;
use chatlingo_lib::engines::{Direction, TranslationEngine, TranslateParams};
use chatlingo_lib::store::{glossary_match_in, GlossaryEntry};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let cfg_path = r"D:\hobby\产物\chatlingo\v0.2.5\data\config.json";
    let raw = std::fs::read_to_string(cfg_path).unwrap_or_else(|e| panic!("读配置失败 {e}"));
    let cfg: AppConfig = serde_json::from_str(&raw).unwrap();
    println!("[cfg] model={} base_url={}", cfg.engine.model, cfg.engine.base_url);

    // 模拟术语库命中
    let entries = vec![
        GlossaryEntry { id: 0, source: "RAG".into(), target: "检索增强生成".into(), note: String::new(), enabled: true, created_at: 0 },
    ];
    let text = "帮我实现一个 RAG 系统";
    let hits = glossary_match_in(&entries, text, 30);
    println!("[match] {hits:?}");
    assert_eq!(hits.len(), 1);

    let engine = OpenAiCompatEngine::new(cfg.engine.base_url, cfg.engine.api_key, cfg.engine.model);
    let mut stream = engine.translate_stream(TranslateParams {
        text: text.into(),
        target_lang: "zh-CN".into(),
        direction: Direction::Input,
        glossary: hits,
        glossary_id: None,
        source_lang: None,
    });
    let mut full = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(c) => full.push_str(&c),
            Err(e) => {
                println!("[translate] ERR: {e}");
                return;
            }
        }
    }
    println!("[translate] {full}");
    if full.contains("检索增强生成") {
        println!("GLOSSARY LLM TEST PASSED");
    } else {
        println!("GLOSSARY LLM TEST FAILED (未出现指定译法)");
    }
}
