use super::{ChunkStream, TranslateParams};
use serde_json::json;

pub struct DeeplEngine {
    pub api_key: String,
    client: reqwest::Client,
}

/// 我们内部语言码 → DeepL 目标语言码
/// 目录中 DeepL 不支持的语种返回 None（泰/越/阿/印地）
pub fn to_deepl_lang(lang: &str) -> Option<&'static str> {
    match lang.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => Some("ZH"),
        "ja" | "jp" | "japanese" => Some("JA"),
        "en" | "en-us" | "english" => Some("EN-US"),
        "ko" | "korean" => Some("KO"),
        "fr" | "french" => Some("FR"),
        "de" | "german" => Some("DE"),
        "es" | "spanish" => Some("ES"),
        "pt" | "portuguese" => Some("PT-BR"),
        "ru" | "russian" => Some("RU"),
        "it" | "italian" => Some("IT"),
        "id" | "indonesian" => Some("ID"),
        _ => None,
    }
}

impl DeeplEngine {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    /// 免费 Key 以 :fx 结尾，走 api-free 子域
    fn host(&self) -> &'static str {
        if self.api_key.ends_with(":fx") {
            "https://api-free.deepl.com"
        } else {
            "https://api.deepl.com"
        }
    }

    /// 创建术语表资源（entries 为 TSV：source\ttarget 每行一对），返回 glossary_id。
    /// 语言对不受支持等错误原样上抛（调用方降级为译后校验）。
    pub async fn create_glossary(
        &self,
        source_lang: &str,
        target_lang: &str,
        entries_tsv: &str,
    ) -> Result<String, String> {
        let url = format!("{}/v2/glossaries", self.host());
        let body = json!({
            "name": format!("chatlingo-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)),
            "source_lang": source_lang,
            "target_lang": target_lang,
            "entries": entries_tsv,
            "entries_format": "tsv",
        });
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let text_body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &text_body[..text_body.len().min(300)]));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text_body).map_err(|e| format!("响应解析失败: {e}"))?;
        v.pointer("/glossary_id")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "响应中没有 glossary_id".to_string())
    }

    /// 删除术语表资源（避免堆积；free 版上限 1000 个）
    pub async fn delete_glossary(&self, glossary_id: &str) {
        let url = format!("{}/v2/glossaries/{glossary_id}", self.host());
        if let Err(e) = self
            .client
            .delete(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .send()
            .await
        {
            eprintln!("[chatlingo] deepl glossary delete failed: {e}");
        }
    }

    pub async fn quick_check(&self) -> Result<(), String> {
        let url = format!("{}/v2/usage", self.host());
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.as_u16() == 403 {
            return Err("Key 无效或已过期 (403)".into());
        }
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &body[..body.len().min(200)]));
        }
        Ok(())
    }

    async fn translate_once(
        &self,
        text: &str,
        target: &str,
        glossary_id: Option<&str>,
        source_lang: Option<&str>,
    ) -> Result<String, String> {
        let url = format!("{}/v2/translate", self.host());
        let mut body = json!({
            "text": [text],
            "target_lang": to_deepl_lang(target),
            "preserve_formatting": true,
        });
        if let Some(gid) = glossary_id {
            body["glossary_id"] = json!(gid);
            // DeepL 要求：使用 glossary 时必须显式 source_lang
            if let Some(src) = source_lang {
                body["source_lang"] = json!(src);
            }
        }
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let text_body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &text_body[..text_body.len().min(300)]));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text_body).map_err(|e| format!("响应解析失败: {e}"))?;
        v.pointer("/translations/0/text")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "响应中没有译文".to_string())
    }
}

impl super::TranslationEngine for DeeplEngine {
    fn translate_stream(&self, params: TranslateParams) -> ChunkStream {
        let engine = DeeplEngine {
            api_key: self.api_key.clone(),
            client: self.client.clone(),
        };
        let text = params.text;
        let target = params.target_lang;
        let glossary_id = params.glossary_id;
        let source_lang = params.source_lang;
        Box::pin(async_stream::stream! {
            let mut attempt = 0;
            loop {
                attempt += 1;
                match engine.translate_once(&text, &target, glossary_id.as_deref(), source_lang.as_deref()).await {
                    Ok(translated) => {
                        yield Ok(translated);
                        return;
                    }
                    Err(e) => {
                        // 429/5xx：指数退避重试，最多 3 次
                        if attempt < 3 && (e.contains("429") || e.contains("5")) {
                            tokio::time::sleep(std::time::Duration::from_millis(
                                1000 * attempt as u64,
                            ))
                            .await;
                            continue;
                        }
                        yield Err(e);
                        return;
                    }
                }
            }
        })
    }
}
