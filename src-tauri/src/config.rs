use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    #[serde(default = "default_base_url")]
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
        EngineConfig { base_url: default_base_url(), api_key: String::new(), model: default_model() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct ClipboardConfig {
    #[serde(default = "yes")]
    pub monitor: bool,
    #[serde(default = "default_min_len")]
    pub min_length: usize,
    #[serde(default = "default_translate_hotkey")]
    pub translate_hotkey: String,
}

fn default_min_len() -> usize {
    20
}
fn default_translate_hotkey() -> String {
    "alt+y".into()
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        ClipboardConfig { monitor: true, min_length: default_min_len(), translate_hotkey: default_translate_hotkey() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub engine: EngineConfig,
    #[serde(default)]
    pub quickbar: QuickbarConfig,
    #[serde(default)]
    pub clipboard: ClipboardConfig,
    #[serde(default = "default_output_target")]
    pub output_target: String,
}

fn default_output_target() -> String {
    "zh-CN".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            engine: EngineConfig {
                base_url: default_base_url(),
                api_key: String::new(),
                model: default_model(),
            },
            quickbar: QuickbarConfig {
                hotkey: default_hotkey(),
                target_langs: default_target_langs(),
                default_target: default_target(),
                auto_copy: true,
                auto_paste: true,
            },
            clipboard: ClipboardConfig {
                monitor: true,
                min_length: default_min_len(),
                translate_hotkey: default_translate_hotkey(),
            },
            output_target: default_output_target(),
        }
    }
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_config_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    std::fs::create_dir_all(&dir).ok();
    dir.join("config.json")
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = config_path(app);
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config_to_disk(app: &AppHandle, cfg: &AppConfig) -> Result<(), String> {
    let path = config_path(app);
    let content = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}
