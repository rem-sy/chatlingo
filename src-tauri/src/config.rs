use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    /// 是否启用（关闭后不参与翻译与降级链）
    #[serde(default = "yes")]
    pub enabled: bool,
    /// 兼容历史配置里的 "baseURL" 写法
    #[serde(default = "default_base_url", alias = "baseURL")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_base_url() -> String {
    "https://api.deepseek.com/v1".into()
}
fn default_model() -> String {
    "deepseek-chat".into()
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig { enabled: true, base_url: default_base_url(), api_key: String::new(), model: default_model() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickbarConfig {
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_target_langs")]
    pub target_langs: Vec<String>,
    #[serde(default = "default_target")]
    pub default_target: String,
    #[serde(default = "yes")]
    pub auto_copy: bool,
    #[serde(default = "yes")]
    pub auto_paste: bool,
}

fn default_hotkey() -> String {
    "alt+t".into()
}
fn default_target_langs() -> Vec<String> {
    vec!["en".into(), "ja".into()]
}
fn default_target() -> String {
    "en".into()
}
fn yes() -> bool {
    true
}

impl Default for QuickbarConfig {
    fn default() -> Self {
        QuickbarConfig {
            hotkey: default_hotkey(),
            target_langs: default_target_langs(),
            default_target: default_target(),
            auto_copy: true,
            auto_paste: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardConfig {
    #[serde(default = "yes")]
    pub monitor: bool,
    #[serde(default = "default_min_len")]
    pub min_length: usize,
    /// 视觉模型（截图 OCR 用），留空则复用主力模型
    #[serde(default)]
    pub vision_model: String,
}

fn default_min_len() -> usize {
    20
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        ClipboardConfig {
            monitor: true,
            min_length: default_min_len(),
            vision_model: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeeplConfig {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
}

impl Default for DeeplConfig {
    fn default() -> Self {
        DeeplConfig { enabled: true, api_key: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliyunConfig {
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub access_key_id: String,
    #[serde(default)]
    pub access_key_secret: String,
}

impl Default for AliyunConfig {
    fn default() -> Self {
        AliyunConfig { enabled: true, access_key_id: String::new(), access_key_secret: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryConfig {
    /// 术语库总开关
    #[serde(default = "yes")]
    pub enabled: bool,
    /// 单次翻译最大注入条数（超出按 source 长度降序截断）
    #[serde(default = "default_max_terms")]
    pub max_terms: usize,
}

fn default_max_terms() -> usize {
    30
}

impl Default for GlossaryConfig {
    fn default() -> Self {
        GlossaryConfig { enabled: true, max_terms: default_max_terms() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub engine: EngineConfig,
    #[serde(default)]
    pub deepl: DeeplConfig,
    #[serde(default)]
    pub aliyun: AliyunConfig,
    /// 输出方向：大模型与 DeepL 同时翻译、面板并列对比
    #[serde(default)]
    pub compare_mode: bool,
    #[serde(default)]
    pub quickbar: QuickbarConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default = "default_output_target")]
    pub output_target: String,
    #[serde(default)]
    pub glossary: GlossaryConfig,
}

fn default_output_target() -> String {
    "zh-CN".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            engine: EngineConfig {
                enabled: true,
                base_url: default_base_url(),
                api_key: String::new(),
                model: default_model(),
            },
            deepl: DeeplConfig::default(),
            aliyun: AliyunConfig::default(),
            compare_mode: false,
            quickbar: QuickbarConfig {
                hotkey: default_hotkey(),
                target_langs: default_target_langs(),
                default_target: default_target(),
                auto_copy: true,
                auto_paste: true,
            },
            clipboard: ClipboardConfig::default(),
            output_target: default_output_target(),
            glossary: GlossaryConfig::default(),
        }
    }
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("config.json")
}

/// 默认（跟随用户配置文件）目录
pub fn legacy_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// 数据目录解析优先级：
/// 1. 便携模式：exe 同目录存在 `chatlingo.portable` 标记 → 用 exe 旁的 data/ 子目录
/// 2. 自定义：默认目录下 `data-location` 指针文件指向的目录
/// 3. 默认：%APPDATA%\com.remsy.chatlingo
pub fn data_dir(app: &AppHandle) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("chatlingo.portable").exists() {
                let d = dir.join("data");
                let _ = fs::create_dir_all(&d);
                return d;
            }
        }
    }
    let legacy = legacy_dir(app);
    if let Ok(s) = fs::read_to_string(legacy.join("data-location")) {
        let t = s.trim();
        if !t.is_empty() {
            let p = PathBuf::from(t);
            if fs::create_dir_all(&p).is_ok() {
                return p;
            }
        }
    }
    fs::create_dir_all(&legacy).ok();
    legacy
}

pub fn is_portable() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("chatlingo.portable").exists()))
        .unwrap_or(false)
}

/// 写入/清除自定义数据目录指针；便携模式下指针无效
pub fn set_data_location(app: &AppHandle, path: Option<&str>) -> Result<(), String> {
    let legacy = legacy_dir(app);
    fs::create_dir_all(&legacy).map_err(|e| e.to_string())?;
    let pointer = legacy.join("data-location");
    match path {
        Some(p) if !p.trim().is_empty() => {
            fs::write(&pointer, p.trim()).map_err(|e| e.to_string())?;
        }
        _ => {
            let _ = fs::remove_file(&pointer);
        }
    }
    Ok(())
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = config_path(app);
    let raw = fs::read_to_string(&path);
    eprintln!("[chatlingo] config path: {}", path.display());
    eprintln!("[chatlingo] config raw: {:?}", raw.as_deref().map(|s| &s[..s.len().min(300)]));
    match raw {
        Ok(content) => {
            let cfg: AppConfig = serde_json::from_str(&content).unwrap_or_default();
            eprintln!("[chatlingo] config parsed engine.base_url={}", cfg.engine.base_url);
            cfg
        }
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config_to_disk(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let path = config_path(app);
    let content = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
