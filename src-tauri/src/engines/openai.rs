use super::{glossary_prompt_block, system_prompt, ChunkStream, TranslateParams};
use crate::config::ApiFormat;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

pub struct OpenAiCompatEngine {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_format: ApiFormat,
    client: Client,
}

impl OpenAiCompatEngine {
    pub fn new(base_url: String, api_key: String, model: String, api_format: ApiFormat) -> Self {
        Self {
            base_url,
            api_key,
            model,
            api_format,
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
        }
    }

    fn clone_shallow(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
            api_format: self.api_format,
            client: self.client.clone(),
        }
    }

    pub async fn quick_check(&self) -> Result<(), String> {
        match self.api_format {
            // Anthropic 无 OpenAI 兼容 /models；用 GET {base}/v1/models（官方支持）验连通与 Key
            ApiFormat::AnthropicMessages => {
                let url = format!("{}/v1/models", self.base_url.trim_end_matches('/'));
                let resp = self
                    .client
                    .get(&url)
                    .header("x-api-key", &self.api_key)
                    .header("anthropic-version", "2023-06-01")
                    .send()
                    .await
                    .map_err(|e| format!("网络错误: {e}"))?;
                let status = resp.status();
                if !status.is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(format!("HTTP {status}: {}", truncate(&body, 200)));
                }
                Ok(())
            }
            // completions / responses 格式均有 GET /models
            _ => {
                let url = format!("{}/models", self.base_url.trim_end_matches('/'));
                let resp = self
                    .client
                    .get(&url)
                    .bearer_auth(&self.api_key)
                    .send()
                    .await
                    .map_err(|e| format!("网络错误: {e}"))?;
                let status = resp.status();
                if !status.is_success() {
                    let body = resp.text().await.unwrap_or_default();
                    return Err(format!("HTTP {status}: {}", truncate(&body, 200)));
                }
                Ok(())
            }
        }
    }

    /// 拉取模型 ID 列表（按字母序）；anthropic 格式不支持（UI 已禁用入口）
    pub async fn list_model_ids(&self) -> Result<Vec<String>, String> {
        if matches!(self.api_format, ApiFormat::AnthropicMessages) {
            return Err("anthropic-messages 格式不支持获取模型列表，请参考渠道文档手动填写".into());
        }
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

    /// 按格式构造（url, 请求体, 是否走 anthropic 头）
    fn build_request(&self, prompt: &str, text: &str) -> (String, Value, bool) {
        match self.api_format {
            ApiFormat::OpenaiCompletions => {
                let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
                let body = json!({
                    "model": self.model,
                    "stream": true,
                    "temperature": 0.2,
                    "messages": [
                        { "role": "system", "content": prompt },
                        { "role": "user", "content": text }
                    ]
                });
                (url, body, false)
            }
            ApiFormat::OpenaiResponses => {
                let url = format!("{}/responses", self.base_url.trim_end_matches('/'));
                let body = json!({
                    "model": self.model,
                    "stream": true,
                    "temperature": 0.2,
                    "instructions": prompt,
                    "input": [ { "role": "user", "content": text } ]
                });
                (url, body, false)
            }
            ApiFormat::AnthropicMessages => {
                // system prompt（含术语注入）走顶层 system 字段
                let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
                let body = json!({
                    "model": self.model,
                    "stream": true,
                    "max_tokens": 8192,
                    "temperature": 0.2,
                    "system": prompt,
                    "messages": [ { "role": "user", "content": text } ]
                });
                (url, body, true)
            }
        }
    }

    /// 按格式解析一条 SSE data JSON，返回 (delta, done, error)
    fn parse_delta(&self, v: &Value) -> (String, bool, Option<String>) {
        match self.api_format {
            ApiFormat::OpenaiCompletions => {
                let delta = v
                    .pointer("/choices/0/delta/content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                (delta.to_string(), false, None)
            }
            ApiFormat::OpenaiResponses => {
                match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "response.output_text.delta" => {
                        (v.get("delta").and_then(|d| d.as_str()).unwrap_or("").to_string(), false, None)
                    }
                    "response.completed" => (String::new(), true, None),
                    "response.failed" | "error" => {
                        let msg = v
                            .pointer("/response/error/message")
                            .or_else(|| v.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("渠道返回错误");
                        (String::new(), true, Some(msg.to_string()))
                    }
                    _ => (String::new(), false, None),
                }
            }
            ApiFormat::AnthropicMessages => {
                match v.get("type").and_then(|t| t.as_str()).unwrap_or("") {
                    "content_block_delta" => {
                        let delta = v
                            .pointer("/delta/text")
                            .and_then(|t| t.as_str())
                            .unwrap_or("");
                        (delta.to_string(), false, None)
                    }
                    "message_stop" => (String::new(), true, None),
                    "error" => {
                        let msg = v.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or("渠道返回错误");
                        (String::new(), true, Some(msg.to_string()))
                    }
                    _ => (String::new(), false, None),
                }
            }
        }
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
        let engine = self.clone_shallow();
        let prompt = system_prompt(params.direction, &params.target_lang);
        let prompt = format!("{prompt}{}", glossary_prompt_block(&params.glossary));
        Box::pin(async_stream::stream! {
            let (url, body, is_anthropic) = engine.build_request(&prompt, &params.text);
            let mut attempt = 0;
            let resp = loop {
                attempt += 1;
                let mut req = engine.client.post(&url);
                req = if is_anthropic {
                    req.header("x-api-key", &engine.api_key)
                        .header("anthropic-version", "2023-06-01")
                } else {
                    req.bearer_auth(&engine.api_key)
                };
                match req
                    .header("User-Agent", "ChatLingo/1.1")
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
                            let (delta, done, err) = engine.parse_delta(&v);
                            if let Some(e) = err {
                                yield Err(e);
                                return;
                            }
                            if !delta.is_empty() {
                                yield Ok(delta);
                            }
                            if done {
                                return;
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
