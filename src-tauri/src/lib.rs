pub mod config;
pub mod detect;
pub mod engines;

use config::AppConfig;
use engines::openai::OpenAiCompatEngine;
use engines::{Direction, TranslationEngine, TranslateParams};
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

struct AppState {
    config: Mutex<AppConfig>,
    /// 我们自己写入剪贴板的内容 hash，防止监听死循环
    written_hashes: Mutex<HashSet<u64>>,
}

const QUICKBAR: &str = "quickbar";
const PANEL: &str = "panel";
const SETTINGS: &str = "settings";

fn default_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn state(app: &AppHandle) -> tauri::State<'_, AppState> {
    app.state()
}

fn parse_shortcut(s: &str) -> Option<Shortcut> {
    s.parse::<Shortcut>().ok()
}

// ---------- 窗口管理 ----------

fn create_windows(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    macro_rules! try_step {
        ($label:expr, $expr:expr) => {
            match $expr {
                Ok(v) => { eprintln!("[chatlingo] window ok: {}", $label); v }
                Err(e) => {
                    eprintln!("[chatlingo] window FAIL: {} -> {:?}", $label, e);
                    return Err(e.into());
                }
            }
        };
    }
    try_step!(
        "settings",
        WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
            .title("ChatLingo 设置")
            .inner_size(680.0, 720.0)
            .resizable(true)
            .visible(false)
            .shadow(true)
            .build()
    );
    try_step!(
        "quickbar",
        WebviewWindowBuilder::new(app, QUICKBAR, WebviewUrl::App("quickbar.html".into()))
            .title("ChatLingo QuickBar")
            .inner_size(640.0, 280.0)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .shadow(true)
            .build()
    );
    try_step!(
        "panel",
        WebviewWindowBuilder::new(app, PANEL, WebviewUrl::App("panel.html".into()))
            .title("ChatLingo Panel")
            .inner_size(440.0, 480.0)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .visible(false)
            .shadow(true)
            .transparent(true)
            .build()
    );
    Ok(())
}

fn show_quickbar(app: &AppHandle) {
    let Some(win) = app.get_webview_window(QUICKBAR) else { return };
    if let Ok(cursor) = app.cursor_position() {
        let pos = tauri::PhysicalPosition::new(cursor.x as i32, cursor.y as i32);
        let _ = win.set_position(tauri::Position::Physical(pos));
    }
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit("quickbar-reset", ());
}

fn hide_quickbar_win(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(QUICKBAR) {
        let _ = win.hide();
    }
}

fn show_panel_bottom_right(app: &AppHandle) {
    let Some(win) = app.get_webview_window(PANEL) else { return };
    if let Ok(Some(monitor)) = app.primary_monitor() {
        if let Ok(size) = win.outer_size() {
            let mon_size = monitor.size();
            let mon_pos = monitor.position();
            let scale = monitor.scale_factor();
            let margin = (24.0 * scale) as i32;
            let x = mon_pos.x + mon_size.width as i32 - size.width as i32 - margin;
            let y = mon_pos.y + mon_size.height as i32 - size.height as i32 - margin - 64; // 留出任务栏余量
            let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(
                x, y,
            )));
        }
    }
    let _ = win.show();
    let _ = win.set_focus();
}

fn hide_panel_win(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(PANEL) {
        let _ = win.hide();
    }
}

// ---------- 翻译执行 ----------

async fn run_translation(
    app: &AppHandle,
    window: &str,
    text: String,
    target: String,
    direction: Direction,
) -> Result<String, String> {
    let cfg = state(app).config.lock().unwrap().clone();
    eprintln!(
        "[chatlingo] engine: url={} model={} key_len={} key_tail={:?}",
        cfg.engine.base_url,
        cfg.engine.model,
        cfg.engine.api_key.len(),
        cfg.engine.api_key.chars().rev().take(6).collect::<String>()
    );
    if cfg.engine.api_key.is_empty() {
        return Err("未配置 API Key，请先在设置中填写".into());
    }
    let engine = OpenAiCompatEngine::new(cfg.engine.base_url, cfg.engine.api_key, cfg.engine.model);
    let mut stream = engine.translate_stream(TranslateParams {
        text,
        target_lang: target,
        direction,
    });
    let win = app.get_webview_window(window).ok_or("窗口不存在")?.clone();

    use futures_util::StreamExt;
    let mut full = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                full.push_str(&chunk);
                if window == QUICKBAR {
                    let id = ACTIVE_REQ.load(std::sync::atomic::Ordering::SeqCst);
                    let _ = win.emit(
                        "quickbar-chunk",
                        serde_json::json!({ "id": id, "text": chunk }),
                    );
                } else {
                    let _ = win.emit("panel-chunk", serde_json::json!({ "text": chunk }));
                }
            }
            Err(e) => {
                eprintln!("[chatlingo] translate error: {e}");
                return Err(e);
            }
        }
    }
    if full.trim().is_empty() {
        return Err("引擎返回了空结果".into());
    }
    eprintln!(
        "[chatlingo] translate ok: {} chars, direction={:?}",
        full.chars().count(),
        direction
    );
    Ok(full)
}

static ACTIVE_REQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

// ---------- 命令 ----------

#[tauri::command]
fn get_config(app: AppHandle) -> AppConfig {
    state(&app).config.lock().unwrap().clone()
}

#[tauri::command]
fn save_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    let st = state(&app);
    {
        let mut cfg = st.config.lock().unwrap();
        *cfg = config.clone();
    }
    config::save_config_to_disk(&app, &config)?;
    register_hotkeys(&app);
    Ok(())
}

#[tauri::command]
async fn test_engine(app: AppHandle) -> Result<Value, String> {
    let cfg = state(&app).config.lock().unwrap().clone();
    if cfg.engine.api_key.is_empty() {
        return Ok(serde_json::json!({ "ok": false, "error": "API Key 为空" }));
    }
    let engine = OpenAiCompatEngine::new(cfg.engine.base_url, cfg.engine.api_key, cfg.engine.model);
    match engine.quick_check().await {
        Ok(()) => Ok(serde_json::json!({ "ok": true })),
        Err(e) => Ok(serde_json::json!({ "ok": false, "error": e })),
    }
}

#[tauri::command]
fn hide_quickbar(app: AppHandle) {
    hide_quickbar_win(&app);
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    hide_panel_win(&app);
}

#[tauri::command]
fn open_settings(app: AppHandle) {
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn panel_ready() {}

#[tauri::command]
fn minimize_panel(app: AppHandle) {
    if let Some(win) = app.get_webview_window(PANEL) {
        let _ = win.minimize();
    }
}

#[tauri::command]
fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    write_clipboard(&app, &text)
}

#[tauri::command]
async fn paste_text(app: AppHandle, text: String) -> Result<(), String> {
    write_clipboard(&app, &text)?;
    hide_quickbar_win(&app);
    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
    simulate_paste();
    Ok(())
}

/// Quick Bar 提交：流式翻译 → 完成后按配置自动复制/回填。
#[tauri::command]
async fn submit_input(app: AppHandle, text: String, target: String, id: u64) -> Result<(), ()> {
    ACTIVE_REQ.store(id, std::sync::atomic::Ordering::SeqCst);
    let result = run_translation(&app, QUICKBAR, text, target, Direction::Input).await;
    let win = app.get_webview_window(QUICKBAR);
    let cfg = state(&app).config.lock().unwrap().clone();
    match result {
        Ok(translated) => {
            let mut pasted = false;
            if cfg.quickbar.auto_paste {
                if write_clipboard(&app, &translated).is_ok() {
                    hide_quickbar_win(&app);
                    tokio::time::sleep(std::time::Duration::from_millis(160)).await;
                    simulate_paste();
                    pasted = true;
                }
            } else if cfg.quickbar.auto_copy {
                let _ = write_clipboard(&app, &translated);
            }
            if let Some(win) = win {
                let _ = win.emit(
                    "quickbar-done",
                    serde_json::json!({ "id": id, "ok": true, "pasted": pasted }),
                );
            }
        }
        Err(e) => {
            if let Some(win) = win {
                let _ = win.emit(
                    "quickbar-done",
                    serde_json::json!({ "id": id, "ok": false, "error": e }),
                );
            }
        }
    }
    Ok(())
}

/// 翻译当前剪贴板内容（输出方向）：热键触发或手动调用。
#[tauri::command]
async fn translate_clipboard(app: AppHandle, manual: bool) -> Result<(), ()> {    let cfg = state(&app).config.lock().unwrap().clone();
    if !manual && !cfg.clipboard.monitor {
        return Ok(());
    }
    let text = match read_clipboard(&app) {
        Some(t) => t,
        None => return Ok(()),
    };
    translate_output(app, text, cfg).await;
    Ok(())
}

pub async fn translate_output(app: AppHandle, text: String, cfg: AppConfig) {
    let trimmed = text.trim().to_string();
    if trimmed.chars().count() < cfg.clipboard.min_length {
        return;
    }
    let h = default_hash(&trimmed);
    if state(&app).written_hashes.lock().unwrap().contains(&h) {
        return;
    }
    if detect::is_already_target(&trimmed, &cfg.output_target) {
        return;
    }
    if detect::code_dominant(&trimmed) {
        return;
    }
    let panel = app.get_webview_window(PANEL);
    if let Some(p) = &panel {
        let _ = p.emit(
            "panel-start",
            serde_json::json!({ "original": trimmed, "outputTarget": cfg.output_target }),
        );
    }
    show_panel_bottom_right(&app);
    let result = run_translation(&app, PANEL, trimmed, cfg.output_target.clone(), Direction::Output).await;
    if let Some(p) = &panel {
        let _ = p.emit(
            "panel-done",
            match result {
                Ok(_) => serde_json::json!({ "ok": true }),
                Err(e) => serde_json::json!({ "ok": false, "error": e }),
            },
        );
    }
}

// ---------- 剪贴板与粘贴 ----------

fn read_clipboard(app: &AppHandle) -> Option<String> {
    let _ = app;
    arboard::Clipboard::new()
        .and_then(|mut c| c.get_text())
        .ok()
        .filter(|t| !t.trim().is_empty())
}

fn write_clipboard(app: &AppHandle, text: &str) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(text.to_string()).map_err(|e| e.to_string())?;
    let h = default_hash(text.trim());
    state(app).written_hashes.lock().unwrap().insert(h);
    Ok(())
}

fn simulate_paste() {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        return;
    };
    let _ = enigo.key(Key::Control, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(Key::Control, Direction::Release);
}

// ---------- 剪贴板监听 ----------

fn spawn_clipboard_monitor(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last: u64 = 0;
        let mut first = true;
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(900)).await;
            let cfg = state(&app).config.lock().unwrap().clone();
            if !cfg.clipboard.monitor {
                continue;
            }
            let text = {
                let app = app.clone();
                tauri::async_runtime::spawn_blocking(move || read_clipboard(&app))
                    .await
                    .ok()
                    .flatten()
            };
            let Some(text) = text else { continue };
            let trimmed = text.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            let h = default_hash(&trimmed);
            if h == last {
                continue;
            }
            last = h;
            if first {
                // 启动时剪贴板里的旧内容不翻译
                first = false;
                continue;
            }
            if state(&app).written_hashes.lock().unwrap().contains(&h) {
                continue;
            }
            translate_output(app.clone(), text, cfg).await;
        }
    });
}

// ---------- 快捷键 ----------

fn register_hotkeys(app: &AppHandle) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let cfg = state(app).config.lock().unwrap().clone();
    if let Some(s) = parse_shortcut(&cfg.quickbar.hotkey) {
        let _ = gs.register(s);
    }
    if let Some(s) = parse_shortcut(&cfg.clipboard.translate_hotkey) {
        let _ = gs.register(s);
    }
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    let cfg = state(app).config.lock().unwrap().clone();
    let quick = parse_shortcut(&cfg.quickbar.hotkey);
    let clip = parse_shortcut(&cfg.clipboard.translate_hotkey);
    if quick.as_ref() == Some(shortcut) {
        if app.get_webview_window(QUICKBAR).map(|w| w.is_visible().unwrap_or(false)) == Some(true) {
            hide_quickbar_win(app);
        } else {
            show_quickbar(app);
        }
    } else if clip.as_ref() == Some(shortcut) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            translate_clipboard(app, true).await.ok();
        });
    }
}

// ---------- 托盘 ----------

fn build_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let toggle_quick = MenuItem::with_id(app, "quick", "呼出输入翻译条", true, None::<&str>)?;
    let translate_clip =
        MenuItem::with_id(app, "clip", "翻译剪贴板（Agent 输出）", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle_quick, &translate_clip, &settings_item, &quit])?;

    let icon = app
        .default_window_icon()
        .cloned()
        .expect("default window icon missing");
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("ChatLingo · AI 对话翻译")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quick" => show_quickbar(app),
            "clip" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    translate_clipboard(app, true).await.ok();
                });
            }
            "settings" => open_settings(app.clone()),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

// ---------- 入口 ----------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 二次启动：唤起设置窗口提示已有实例在运行
            if let Some(win) = app.get_webview_window(SETTINGS) {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        on_shortcut(app, shortcut);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let cfg = config::load_config(&app.handle());
            app.manage(AppState {
                config: Mutex::new(cfg),
                written_hashes: Mutex::new(HashSet::new()),
            });
            let steps: Vec<(&str, Box<dyn FnOnce(&AppHandle) -> Result<(), Box<dyn std::error::Error>>>)> =
                vec![];
            let _ = steps;
            eprintln!("[chatlingo] setup: creating windows");
            create_windows(&app.handle())?;
            eprintln!("[chatlingo] setup: building tray");
            build_tray(&app.handle())?;
            eprintln!("[chatlingo] setup: registering hotkeys");
            register_hotkeys(&app.handle());
            eprintln!("[chatlingo] setup: spawning monitor");
            spawn_clipboard_monitor(app.handle().clone());
            eprintln!("[chatlingo] setup: done");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 设置窗口点关闭只隐藏，不销毁，便于托盘再次唤起
                if window.label() == SETTINGS {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            test_engine,
            hide_quickbar,
            hide_panel,
            minimize_panel,
            open_settings,
            panel_ready,
            copy_text,
            paste_text,
            submit_input,
            translate_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatLingo");
}
