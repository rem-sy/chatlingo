//! M5：DeepL Glossary API 实测（en→zh 建资源 + 带 glossary_id 翻译 + 删资源）
use chatlingo_lib::engines::deepl::DeeplEngine;
use chatlingo_lib::config::AppConfig;
use chatlingo_lib::engines::{Direction, TranslationEngine, TranslateParams};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let raw = std::fs::read_to_string(r"D:\hobby\产物\chatlingo\v0.2.5\data\config.json").unwrap();
    let cfg: AppConfig = serde_json::from_str(&raw).unwrap();
    let engine = DeeplEngine::new(cfg.deepl.api_key.clone());

    let terms = vec![("RAG".to_string(), "检索增强生成".to_string())];
    let tsv = terms.iter().map(|(s, t)| format!("{s}\t{t}")).collect::<Vec<_>>().join("\n");

    let gid = match engine.create_glossary("EN", "ZH", &tsv).await {
        Ok(id) => {
            println!("[create] OK id={id}");
            id
        }
        Err(e) => {
            println!("[create] ERR: {e}");
            return;
        }
    };

    let mut stream = engine.translate_stream(TranslateParams {
        text: "Help me implement a RAG system with vector search.".into(),
        target_lang: "zh-CN".into(),
        direction: Direction::Input,
        glossary: terms,
        glossary_id: Some(gid.clone()),
        source_lang: Some("EN".into()),
    });
    let mut full = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(c) => full.push_str(&c),
            Err(e) => println!("[translate] ERR: {e}"),
        }
    }
    println!("[translate] {full}");
    engine.delete_glossary(&gid).await;
    println!("[delete] done");
    println!(
        "{}",
        if full.contains("检索增强生成") {
            "DEEPL GLOSSARY TEST PASSED"
        } else {
            "DEEPL GLOSSARY TEST FAILED (译法未生效)"
        }
    );
}
