use super::{ChunkStream, TranslateParams};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

pub struct TencentEngine {
    pub secret_id: String,
    pub secret_key: String,
    client: reqwest::Client,
}

/// 内部语言码 → 腾讯云 TMT 目标语言码
/// 不支持的语种返回 None（印尼语）
pub fn to_tencent_lang(lang: &str) -> Option<&'static str> {
    match lang.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => Some("zh"),
        "en" | "en-us" | "english" => Some("en"),
        "ja" | "jp" | "japanese" => Some("jp"),
        "ko" | "korean" => Some("kr"),
        "fr" | "french" => Some("fr"),
        "de" | "german" => Some("de"),
        "es" | "spanish" => Some("es"),
        "ru" | "russian" => Some("ru"),
        "pt" | "portuguese" => Some("pt"),
        "it" | "italian" => Some("it"),
        "tr" | "turkish" => Some("tr"),
        "th" | "thai" => Some("th"),
        "vi" | "vietnamese" => Some("vi"),
        "ar" | "arabic" => Some("ar"),
        _ => None,
    }
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("hmac key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

impl TencentEngine {
    pub fn new(secret_id: String, secret_key: String) -> Self {
        Self {
            secret_id,
            secret_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    fn host(&self) -> &'static str {
        "tmt.tencentcloudapi.com"
    }

    pub async fn quick_check(&self) -> Result<(), String> {
        if self.secret_id.is_empty() || self.secret_key.is_empty() {
            return Err("SecretId/SecretKey 未填写".into());
        }
        let (payload, headers) = self.signed_request("TextTranslate", &json!({
            "Source": "en",
            "Target": "zh",
            "ProjectId": 0,
            "Text": "hello"
        }))?;
        let resp = self
            .client
            .post(format!("https://{}", self.host()))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-TC-Action", "TextTranslate")
            .header("X-TC-Version", "2018-03-21")
            .header("X-TC-Timestamp", headers.ts)
            .header("X-TC-Language", "zh-CN")
            .header("Authorization", headers.authorization)
            .body(payload)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &body[..body.len().min(200)]));
        }
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("响应解析失败: {e}"))?;
        if let Some(err) = v.pointer("/Response/Error") {
            let code = err.get("Code").and_then(|c| c.as_str()).unwrap_or("");
            if code.contains("AuthFailure") || code.contains("SecretId") {
                return Err(format!("密钥无效（{}）", code));
            }
            return Err(format!("{}: {}", code, err.get("Message").and_then(|m| m.as_str()).unwrap_or("")));
        }
        Ok(())
    }

    fn signed_request(&self, action: &str, params: &serde_json::Value) -> Result<(String, SigHeaders), String> {
        let payload = serde_json::to_string(params).map_err(|e| e.to_string())?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| e.to_string())?;
        let date = time_date(ts);

        let canonical = format!(
            "POST\n/\n\ncontent-type:application/json; charset=utf-8\nhost:{}\nx-tc-action:{}\n\ncontent-type;host;x-tc-action\n{}",
            self.host(),
            action.to_lowercase(),
            sha256_hex(payload.as_bytes())
        );
        let string_to_sign = format!(
            "TC3-HMAC-SHA256\n{}\n{}/tmt/tc3_request\n{}",
            ts,
            date,
            sha256_hex(canonical.as_bytes())
        );
        let k_date = hmac_sha256(format!("TC3{}", self.secret_key).as_bytes(), date.as_bytes());
        let k_service = hmac_sha256(&k_date, b"tmt");
        let k_signing = hmac_sha256(&k_service, b"tc3_request");
        let signature = hex::encode(hmac_sha256(&k_signing, string_to_sign.as_bytes()));
        let authorization = format!(
            "TC3-HMAC-SHA256 Credential={}/{}/tmt/tc3_request, SignedHeaders=content-type;host;x-tc-action, Signature={}",
            self.secret_id, date, signature
        );
        Ok((
            payload,
            SigHeaders { ts, authorization },
        ))
    }

    async fn translate_once(&self, text: &str, target: &str) -> Result<String, String> {
        let Some(target) = to_tencent_lang(target) else {
            return Err(format!("腾讯云暂不支持目标语言：{target}"));
        };
        let (payload, headers) = self.signed_request(
            "TextTranslate",
            &json!({ "Source": "auto", "Target": target, "ProjectId": 0, "Text": text }),
        )?;
        let resp = self
            .client
            .post(format!("https://{}", self.host()))
            .header("Content-Type", "application/json; charset=utf-8")
            .header("X-TC-Action", "TextTranslate")
            .header("X-TC-Version", "2018-03-21")
            .header("X-TC-Timestamp", headers.ts)
            .header("X-TC-Language", "zh-CN")
            .header("Authorization", headers.authorization)
            .body(payload)
            .send()
            .await
            .map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &body[..body.len().min(300)]));
        }
        let v: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("响应解析失败: {e}"))?;
        if let Some(err) = v.pointer("/Response/Error") {
            let code = err.get("Code").and_then(|c| c.as_str()).unwrap_or("");
            let msg = err.get("Message").and_then(|m| m.as_str()).unwrap_or("");
            return Err(format!("{code}: {msg}"));
        }
        v.pointer("/Response/TargetText")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "响应中没有译文".to_string())
    }
}

struct SigHeaders {
    ts: u64,
    authorization: String,
}

fn time_date(ts: u64) -> String {
    // 将 unix 时间戳转为 UTC 日期 YYYY-MM-DD
    let secs = ts;
    let days = secs / 86400;
    // 简单的闰年计算
    let mut year = 1970i64;
    let mut remaining = days as i64;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let ydays = if leap { 366 } else { 365 };
        if remaining < ydays {
            break;
        }
        remaining -= ydays;
        year += 1;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let mdays = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for &m in &mdays {
        if remaining < m {
            break;
        }
        remaining -= m;
        month += 1;
    }
    format!("{year:04}-{month:02}-{remaining:02}")
}

impl super::TranslationEngine for TencentEngine {
    fn translate_stream(&self, params: TranslateParams) -> ChunkStream {
        let engine = TencentEngine {
            secret_id: self.secret_id.clone(),
            secret_key: self.secret_key.clone(),
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
                        if attempt < 2 && (e.contains("RequestLimitExceeded") || e.contains("5")) {
                            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
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
