//! M5：阿里云 Context 注入实测（Key 从本机 config.json 读）
use chatlingo_lib::config::AppConfig;
use chatlingo_lib::engines::aliyun::AliyunEngine;
use chatlingo_lib::engines::{glossary_context, Direction, TranslationEngine, TranslateParams};
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    let raw = std::fs::read_to_string(r"D:\hobby\产物\chatlingo\v0.2.5\data\config.json").unwrap();
    let cfg: AppConfig = serde_json::from_str(&raw).unwrap();
    // 本机 config.json 未填阿里云密钥 → 从 ak.txt 读（与 test_aliyun 一致）
    let (id, secret) = if cfg.aliyun.access_key_id.is_empty() {
        let keys = std::fs::read_to_string(r"C:\temp\chatlingo\ak.txt").expect("ak.txt 不存在");
        let mut lines = keys.lines();
        (
            lines.next().unwrap().trim().to_string(),
            lines.next().unwrap().trim().to_string(),
        )
    } else {
        (cfg.aliyun.access_key_id.clone(), cfg.aliyun.access_key_secret.clone())
    };
    let engine = AliyunEngine::new(id, secret);

    let terms = vec![("RAG".to_string(), "检索增强生成".to_string())];
    println!("[context] {}", glossary_context(&terms));

    let mut stream = engine.translate_stream(TranslateParams {
        text: "Help me implement a RAG system with vector search.".into(),
        target_lang: "zh-CN".into(),
        direction: Direction::Input,
        glossary: terms,
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
    println!(
        "{}",
        if full.contains("检索增强生成") {
            "ALIYUN CONTEXT TEST PASSED"
        } else {
            "ALIYUN CONTEXT TEST: 指定译法未出现（Context 是软约束，运行时译后校验会兜底提示）"
        }
    );
}
