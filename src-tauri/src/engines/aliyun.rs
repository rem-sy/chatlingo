use super::{ChunkStream, TranslateParams};
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub struct AliyunEngine {
    pub access_key_id: String,
    pub access_key_secret: String,
    client: reqwest::Client,
}

/// 内部语言码 → 阿里云机器翻译目标语言码
/// 不支持的语种返回 None（印尼语/印地语）
pub fn to_aliyun_lang(lang: &str) -> Option<&'static str> {
    match lang.to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-tw" | "chinese" => Some("zh"),
        "en" | "en-us" | "english" => Some("en"),
        "ja" | "jp" | "japanese" => Some("ja"),
        "ko" | "korean" => Some("ko"),
        "fr" | "french" => Some("fr"),
        "de" | "german" => Some("de"),
        "es" | "spanish" => Some("es"),
        "ru" | "russian" => Some("ru"),
        "pt" | "portuguese" => Some("pt"),
        "it" | "italian" => Some("it"),
        "th" | "thai" => Some("th"),
        "vi" | "vietnamese" => Some("vi"),
        "ar" | "arabic" => Some("ar"),
        _ => None,
    }
}

/// RFC3986 百分号编码（阿里云 POP 签名要求）
fn pct_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn utc_datetime(ts: u64) -> (String, String) {
    // 返回 (YYYY-MM-DD, YYYY-MM-DDTHH:MM:SSZ)
    let secs_of_day = (ts % 86400) as i64;
    let days = (ts / 86400) as i64;
    let (date, _doy) = civil_from_days(days);
    let h = secs_of_day / 3600;
    let m = (secs_of_day % 3600) / 60;
    let s = secs_of_day % 60;
    (
        date.clone(),
        format!("{date}T{h:02}:{m:02}:{s:02}Z"),
    )
}

/// 天数（自 1970-01-01）→ 民用日期
fn civil_from_days(days: i64) -> (String, i64) {
    let mut year = 1970i64;
    let mut remaining = days;
    loop {
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let yd = if leap { 366 } else { 365 };
        if remaining < yd {
            break;
        }
        remaining -= yd;
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
    (format!("{year:04}-{month:02}-{:02}", remaining + 1), remaining)
}

fn nonce() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64)
        .unwrap_or(0);
    let c = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("{}{:06}", nanos, c)
}

impl AliyunEngine {
    pub fn new(access_key_id: String, access_key_secret: String) -> Self {
        Self {
            access_key_id,
            access_key_secret,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    /// 连通性检测：翻译一个词，成功即密钥与服务均可用
    pub async fn quick_check(&self) -> Result<(), String> {
        if self.access_key_id.is_empty() || self.access_key_secret.is_empty() {
            return Err("AccessKeyId/Secret 未填写".into());
        }
        self.translate_once("hello", "zh").await.map(|_| ())
    }

    async fn translate_once(&self, text: &str, target: &str) -> Result<String, String> {
        let Some(target) = to_aliyun_lang(target) else {
            return Err(format!("阿里云暂不支持目标语言：{target}"));
        };
        let (_, iso_time) = utc_datetime(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .map_err(|e| e.to_string())?,
        );

        // 按参数名字典序排列
        let mut params: Vec<(String, String)> = vec![
            ("AccessKeyId".into(), self.access_key_id.clone()),
            ("Action".into(), "Translate".into()),
            ("FormatType".into(), "text".into()),
            ("SignatureMethod".into(), "HMAC-SHA1".into()),
            ("SignatureNonce".into(), nonce()),
            ("SignatureVersion".into(), "1.0".into()),
            ("SourceLanguage".into(), "auto".into()),
            ("SourceText".into(), text.to_string()),
            ("TargetLanguage".into(), target.to_string()),
            ("Timestamp".into(), iso_time),
            ("Version".into(), "2018-01-02".into()),
        ];
        params.sort_by(|a, b| a.0.cmp(&b.0));
        let query = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, pct_encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // POP RPC 签名：GET&%2F&urlencode(查询串)，密钥 = Secret + "&"
        let string_to_sign = format!("GET&{}&{}", pct_encode("/"), pct_encode(&query));
        let mut mac = HmacSha1::new_from_slice(format!("{}&", self.access_key_secret).as_bytes())
            .map_err(|e| e.to_string())?;
        mac.update(string_to_sign.as_bytes());
        let signature = base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes());

        let url = format!("https://mt.aliyuncs.com/?{query}&Signature={}", pct_encode(&signature));
        let resp = self.client.get(&url).send().await.map_err(|e| format!("网络错误: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("HTTP {status}: {}", &body[..body.len().min(300)]));
        }
        let v: Value = serde_json::from_str(&body).map_err(|e| format!("响应解析失败: {e}"))?;
        if let Some(code) = v.get("Code").and_then(|c| c.as_str()) {
            let msg = v.get("Message").and_then(|m| m.as_str()).unwrap_or("");
            return Err(format!("{code}: {msg}"));
        }
        v.pointer("/Data/Translated")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "响应中没有译文".to_string())
    }
}

impl super::TranslationEngine for AliyunEngine {
    fn translate_stream(&self, params: TranslateParams) -> ChunkStream {
        let engine = AliyunEngine {
            access_key_id: self.access_key_id.clone(),
            access_key_secret: self.access_key_secret.clone(),
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
                        // Throttling 类限流退避重试
                        if attempt < 3 && e.contains("Throttl") {
                            tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
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
