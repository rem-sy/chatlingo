use crate::config::AppConfig;
use base64::Engine as _;

/// 用视觉模型从截图提取文字（OpenAI 兼容 image_url 协议，非流式）
pub async fn extract_text(cfg: &AppConfig, image_png: &[u8]) -> Result<String, String> {
    let model = if cfg.clipboard.vision_model.is_empty() {
        cfg.engine.model.clone()
    } else {
        cfg.clipboard.vision_model.clone()
    };
    let b64 = base64::engine::general_purpose::STANDARD.encode(image_png);
    let data_url = format!("data:image/png;base64,{b64}");
    let url = format!(
        "{}/chat/completions",
        cfg.engine.base_url.trim_end_matches('/')
    );
    let body = serde_json::json!({
        "model": model,
        "stream": false,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": "Extract all visible text from this image exactly as written. Preserve line breaks and reading order. Output ONLY the extracted text, no explanations." },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }]
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .post(&url)
        .bearer_auth(&cfg.engine.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络错误: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let head = &text[..text.len().min(300)];
        return Err(format!("HTTP {status}: {head}（若模型不支持图片输入，请在设置里指定视觉模型）"));
    }
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("响应解析失败: {e}"))?;
    v.pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "视觉模型没有返回文字".to_string())
}
