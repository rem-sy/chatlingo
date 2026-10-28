use super::{glossary_prompt_block, system_prompt, ChunkStream, TranslateParams};
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub struct OpenAiCompatEngine {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    client: Client,
}

impl OpenAiCompatEngine {
    pub fn new(base_url: String, api_key: String, model: String) -> Self {
        Self {
            base_url,
            api_key,
            model,
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
        }
    }

    pub async fn quick_check(&self) -> Result<(), String> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {}", truncate(&body, 200)));
        }
        Ok(())
    }

    /// 拉取模型 ID 列表（按字母序）
    pub async fn list_model_ids(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", truncate(&body, 200)));
        }
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("响应解析失败: {e}"))?;
        let mut ids: Vec<String> = v
            .as_object()
            .and_then(|o| o.get("data"))
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        Ok(ids)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() > n {
        format!("{}…", &s[..n])
    } else {
        s.to_string()
    }
}

impl super::TranslationEngine for OpenAiCompatEngine {
    fn translate_stream(&self, params: TranslateParams) -> ChunkStream {
        let engine = OpenAiCompatEngine {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            client: self.client.clone(),
        };
        let prompt = system_prompt(params.direction, &params.target_lang);
        let prompt = format!("{prompt}{}", glossary_prompt_block(&params.glossary));
        Box::pin(async_stream::stream! {
            let url = format!("{}/chat/completions", engine.base_url.trim_end_matches('/'));
            let body = json!({
                "model": engine.model,
                "stream": true,
                "temperature": 0.2,
                "messages": [
                    { "role": "system", "content": prompt },
                    { "role": "user", "content": params.text }
                ]
            });
            let mut attempt = 0;
            let resp = loop {
                attempt += 1;
                match engine
                    .client
                    .post(&url)
                    .bearer_auth(&engine.api_key)
                    .header("User-Agent", "ChatLingo/0.1")
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(r) => {
                        // 网关偶发 401/5xx：自动重试一次
                        let s = r.status();
                        if attempt < 2 && (s.as_u16() == 401 || s.is_server_error()) {
                            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                            continue;
                        }
                        break r;
                    }
                    Err(e) => {
                        if attempt < 2 {
                            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                            continue;
                        }
                        yield Err(format!("网络错误: {e}"));
                        return;
                    }
                }
            };
            let status = resp.status();
            if !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                yield Err(format!("HTTP {status}: {}", truncate(&text, 300)));
                return;
            }
            let mut stream = resp.bytes_stream();
            let mut buf = String::new();
            loop {
                match stream.next().await {
                    Some(Ok(bytes)) => {
                        buf.push_str(&String::from_utf8_lossy(&bytes));
                        loop {
                            let Some(pos) = buf.find('\n') else { break };
                            let line: String = buf.drain(..=pos).collect();
                            let line = line.trim();
                            if !line.starts_with("data:") {
                                continue;
                            }
                            let data = line["data:".len()..].trim();
                            if data == "[DONE]" {
                                return;
                            }
                            let Ok(v) = serde_json::from_str::<Value>(data) else { continue };
                            let delta = v
                                .pointer("/choices/0/delta/content")
                                .and_then(|c| c.as_str())
                                .unwrap_or("");
                            if !delta.is_empty() {
                                yield Ok(delta.to_string());
                            }
                        }
                    }
                    Some(Err(e)) => {
                        yield Err(format!("流中断: {e}"));
                        return;
                    }
                    None => return,
                }
            }
        })
    }
}
