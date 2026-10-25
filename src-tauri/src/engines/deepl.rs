use super::{ChunkStream, TranslateParams};
use serde_json::json;

pub struct DeeplEngine {
    pub api_key: String,
    client: reqwest::Client,
}

/// 我们内部语言码 → DeepL 目标语言码
pub fn to_deepl_lang(lang: &str) -> &'static str {
    match lang.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => "ZH",
        "ja" | "jp" | "japanese" => "JA",
        "en" | "english" => "EN-US",
        "ko" | "korean" => "KO",
        "fr" | "french" => "FR",
        "de" | "german" => "DE",
        "es" | "spanish" => "ES",
        "pt" | "portuguese" => "PT-BR",
        "ru" | "russian" => "RU",
        _ => "EN-US",
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
    ) -> Result<String, String> {
        let url = format!("{}/v2/translate", self.host());
        let body = json!({
            "text": [text],
            "target_lang": to_deepl_lang(target),
            "preserve_formatting": true,
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
        Box::pin(async_stream::stream! {
            let mut attempt = 0;
            loop {
                attempt += 1;
                match engine.translate_once(&text, &target).await {
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
