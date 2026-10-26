pub mod config;
pub mod detect;
pub mod engines;
pub mod store;

use config::AppConfig;
use engines::openai::OpenAiCompatEngine;
use engines::{Direction, TranslationEngine, TranslateParams};
use base64::Engine as _;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use store::HistoryStore;

/// 译文 LRU 缓存（key = hash(text + target + direction)）
struct TranslationCache {
    map: std::collections::HashMap<u64, String>,
    order: std::collections::VecDeque<u64>,
    cap: usize,
}

impl TranslationCache {
    fn new() -> Self {
        TranslationCache {
            map: std::collections::HashMap::new(),
            order: std::collections::VecDeque::new(),
            cap: 200,
        }
    }
    fn get(&mut self, k: u64) -> Option<&String> {
        if self.map.contains_key(&k) {
            // 提升为最近使用
            if let Some(pos) = self.order.iter().position(|&x| x == k) {
                self.order.remove(pos);
                self.order.push_back(k);
            }
            self.map.get(&k)
        } else {
            None
        }
    }
    fn put(&mut self, k: u64, v: String) {
        if self.map.insert(k, v).is_none() {
            self.order.push_back(k);
            while self.order.len() > self.cap {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }
}

struct AppState {
    config: Mutex<AppConfig>,
    /// 我们自己写入剪贴板的内容 hash，防止监听死循环
    written_hashes: Mutex<HashSet<u64>>,
    /// 译文缓存
    cache: Mutex<TranslationCache>,
    /// 进行中的翻译请求（同 key 合并等待）
    in_flight: Mutex<HashSet<u64>>,
    /// 翻译历史
    history: HistoryStore,
}

const QUICKBAR: &str = "quickbar";
const PANEL: &str = "panel";
const SETTINGS: &str = "settings";
const SNIP: &str = "snip";

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
        "history",
        WebviewWindowBuilder::new(app, "history", WebviewUrl::App("history.html".into()))
            .title("ChatLingo 翻译历史")
            .inner_size(560.0, 640.0)
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
            .min_inner_size(420.0, 200.0)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(true)
            .maximizable(false)
            .visible(false)
            .shadow(true)
            .transparent(true)
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
            .maximizable(false)
            .visible(false)
            .shadow(true)
            .transparent(true)
            .build()
    );
    try_step!(
        "snip",
        WebviewWindowBuilder::new(app, SNIP, WebviewUrl::App("snip.html".into()))
            .title("ChatLingo Snip")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .shadow(false)
            .transparent(true)
            .build()
    );
    Ok(())
}

/// 用户手动拖动过面板后记住位置，之后 Alt+T 不再重定位到光标
static USER_QUICKBAR_POS: Mutex<Option<(i32, i32)>> = Mutex::new(None);

fn show_quickbar(app: &AppHandle) {
    let Some(win) = app.get_webview_window(QUICKBAR) else { return };
    let user_pos = *USER_QUICKBAR_POS.lock().unwrap();
    let mut pos = match user_pos {
        Some((x, y)) => (x, y),
        None => match app.cursor_position() {
            Ok(c) => (c.x as i32, c.y as i32),
            Err(_) => (200, 200),
        },
    };
    // 钳制到主屏可视范围内，防止记住的出界坐标把面板藏到屏幕外
    if let (Ok(Some(monitor)), Ok(size)) = (app.primary_monitor(), win.outer_size()) {
        let mon = monitor.size();
        let mon_pos = monitor.position();
        let max_x = mon_pos.x + mon.width as i32 - size.width as i32;
        let max_y = mon_pos.y + mon.height as i32 - size.height as i32;
        pos.0 = pos.0.clamp(mon_pos.x, max_x.max(mon_pos.x));
        pos.1 = pos.1.clamp(mon_pos.y, max_y.max(mon_pos.y));
    }
    let _ = win.set_position(tauri::PhysicalPosition::new(pos.0, pos.1));
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit("quickbar-reset", ());
}

#[tauri::command]
fn save_quickbar_pos(x: i32, y: i32) {
    *USER_QUICKBAR_POS.lock().unwrap() = Some((x, y));
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
    let llm_ready = !cfg.engine.api_key.is_empty();
    let deepl_ready = !cfg.deepl.api_key.is_empty();
    if !llm_ready && !deepl_ready {
        return Err("未配置任何翻译引擎，请先在设置中填写".into());
    }

    // ---- 译文缓存 ----
    let cache_key = default_hash(&format!("{}|{}|{:?}", text, target, direction));
    if let Some(cached) = state(app).cache.lock().unwrap().get(cache_key).cloned() {
        eprintln!("[chatlingo] cache hit ({} chars)", cached.chars().count());
        if let Some(win) = app.get_webview_window(window) {
            if window == QUICKBAR {
                let id = ACTIVE_REQ.load(std::sync::atomic::Ordering::SeqCst);
                let _ = win.emit("quickbar-chunk", serde_json::json!({ "id": id, "text": cached }));
            } else {
                let _ = win.emit("panel-chunk", serde_json::json!({ "text": cached }));
            }
        }
        return Ok(cached);
    }
    // ---- 请求合并：同内容翻译进行中 → 等它完成后走缓存 ----
    let already_in_flight = {
        let st = state(app);
        let mut infl = st.in_flight.lock().unwrap();
        if infl.contains(&cache_key) {
            true
        } else {
            infl.insert(cache_key);
            false
        }
    };
    if already_in_flight {
        for _ in 0..200 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let cached = state(app).cache.lock().unwrap().get(cache_key).cloned();
            if let Some(cached) = cached {
                if let Some(win) = app.get_webview_window(window) {
                    let _ = win.emit("panel-chunk", serde_json::json!({ "text": cached }));
                }
                return Ok(cached);
            }
            let still = state(app).in_flight.lock().unwrap().contains(&cache_key);
            if !still {
                break; // 对方失败了，自己再翻一次
            }
        }
    }

    let mut result = if llm_ready {
        eprintln!(
            "[chatlingo] engine: primary=LLM url={} model={}",
            cfg.engine.base_url, cfg.engine.model
        );
        run_engine_stream(app, window, text.clone(), target.clone(), direction, &cfg).await
    } else {
        Err("LLM 未配置".into())
    };
    let mut used_engine = "llm";

    // 降级链：LLM 失败且 DeepL 已配置 → 自动兜底
    if result.is_err() && deepl_ready {
        let err = result.clone().unwrap_err();
        eprintln!("[chatlingo] LLM failed ({err}), falling back to DeepL");
        if let Some(win) = app.get_webview_window(window) {
            let _ = win.emit(
                "engine-note",
                serde_json::json!({ "note": format!("大模型失败，已用 DeepL 兜底（{err}）") }),
            );
        }
        let engine = engines::deepl::DeeplEngine::new(cfg.deepl.api_key.clone());
        use futures_util::StreamExt;
        let mut stream = engine.translate_stream(TranslateParams {
            text: text.clone(),
            target_lang: target,
            direction,
        });
        let win = app.get_webview_window(window);
        let mut full = String::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    full.push_str(&chunk);
                    if let Some(w) = &win {
                        if window == QUICKBAR {
                            let id = ACTIVE_REQ.load(std::sync::atomic::Ordering::SeqCst);
                            let _ = w.emit(
                                "quickbar-chunk",
                                serde_json::json!({ "id": id, "text": chunk }),
                            );
                        } else {
                            let _ = w.emit("panel-chunk", serde_json::json!({ "text": chunk }));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[chatlingo] deepl translate error: {e}");
                    result = Err(format!("LLM: {err}；DeepL: {e}"));
                    break;
                }
            }
        }
        if result.is_err() && !full.trim().is_empty() {
            eprintln!(
                "[chatlingo] translate ok via DeepL: {} chars",
                full.chars().count()
            );
            result = Ok(full);
            used_engine = "deepl";
        }
    }
    // 成功 → 写入缓存与历史；无论如何清掉 in-flight 标记
    if let Ok(translated) = &result {
        state(app).cache.lock().unwrap().put(cache_key, translated.clone());
        let dir_label = match direction {
            Direction::Input => "input",
            Direction::Output => "output",
        };
        state(app).history.add(app, dir_label, &text, translated, used_engine);
    }
    state(app).in_flight.lock().unwrap().remove(&cache_key);
    result
}

async fn run_engine_stream(
    app: &AppHandle,
    window: &str,
    text: String,
    target: String,
    direction: Direction,
    cfg: &AppConfig,
) -> Result<String, String> {
    let engine = OpenAiCompatEngine::new(cfg.engine.base_url.clone(), cfg.engine.api_key.clone(), cfg.engine.model.clone());
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

/// 拉取引擎的模型列表（OpenAI 兼容 GET /models）
#[tauri::command]
async fn list_models(app: AppHandle) -> Result<Value, String> {
    let cfg = state(&app).config.lock().unwrap().clone();
    if cfg.engine.api_key.is_empty() {
        return Err("请先填写 API Key".into());
    }
    let engine = OpenAiCompatEngine::new(cfg.engine.base_url, cfg.engine.api_key, cfg.engine.model);
    match engine.list_model_ids().await {
        Ok(ids) => Ok(serde_json::json!({ "models": ids })),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn test_engine(app: AppHandle) -> Result<Value, String> {
    let cfg = state(&app).config.lock().unwrap().clone();
    // LLM 主引擎
    let llm = if cfg.engine.api_key.is_empty() {
        (false, "未填写 Key".to_string())
    } else {
        let engine = OpenAiCompatEngine::new(cfg.engine.base_url.clone(), cfg.engine.api_key.clone(), cfg.engine.model.clone());
        match engine.quick_check().await {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    // DeepL 兜底
    let deepl = if cfg.deepl.api_key.is_empty() {
        (false, "未填写 Key".to_string())
    } else {
        let engine = engines::deepl::DeeplEngine::new(cfg.deepl.api_key.clone());
        match engine.quick_check().await {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    let ok = llm.0 || deepl.0;
    let mut parts = vec![];
    if !llm.0 {
        parts.push(format!("大模型: {}", llm.1));
    } else {
        parts.push("大模型: ✓".into());
    }
    if !deepl.0 {
        parts.push(format!("DeepL: {}", deepl.1));
    } else {
        parts.push("DeepL: ✓".into());
    }
    Ok(serde_json::json!({ "ok": ok, "error": parts.join("；") }))
}

#[tauri::command]
fn debug_log(msg: String) {
    eprintln!("[chatlingo][js] {msg}");
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
fn history_list(app: AppHandle, query: String) -> Vec<store::HistoryEntry> {
    state(&app).history.list(&app, &query, 500)
}

#[tauri::command]
fn history_clear(app: AppHandle) {
    state(&app).history.clear(&app);
}

#[tauri::command]
fn open_history(app: AppHandle) {
    if let Some(win) = app.get_webview_window("history") {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = win.emit("history-refresh", ());
    }
}

/// 系统文件夹选择器（Rust 侧调用，不走前端 ACL）
#[tauri::command]
async fn pick_folder(app: AppHandle) -> Result<Value, String> {
    use tauri_plugin_dialog::DialogExt;
    let app = app.clone();
    let folder = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("选择数据存储目录")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| e.to_string())?;
    match folder {
        Some(p) => Ok(serde_json::json!({ "path": p.to_string() })),
        None => Ok(serde_json::json!({ "path": null })),
    }
}

/// 查询当前数据目录与模式
#[tauri::command]
fn get_data_info(app: AppHandle) -> Value {
    let dir = config::data_dir(&app);
    serde_json::json!({
        "dir": dir.to_string_lossy(),
        "portable": config::is_portable(),
        "custom": dir != config::legacy_dir(&app) && !config::is_portable(),
    })
}

/// 设置/清除自定义数据目录，并把现有数据迁移过去（便携模式下不可用）
#[tauri::command]
fn set_data_dir(app: AppHandle, path: String) -> Result<Value, String> {
    eprintln!("[chatlingo] set_data_dir called: path={path:?}");
    if config::is_portable() {
        return Err("便携模式下数据目录固定在 exe 旁，无法自定义".into());
    }
    let old = config::data_dir(&app);
    if path.trim().is_empty() {
        // 恢复默认
        config::set_data_location(&app, None)?;
        state(&app).history.reset();
        return Ok(serde_json::json!({ "ok": true, "restored": true }));
    }
    let new_dir = std::path::PathBuf::from(path.trim());
    // 中文路径不受影响吗？配置/DB 不经 WebView2，无影响；但为稳妥提示
    std::fs::create_dir_all(&new_dir).map_err(|e| format!("无法创建目录: {e}"))?;
    if new_dir == old {
        return Err("新目录与当前目录相同".into());
    }
    // 迁移数据
    for name in ["config.json", "history.db"] {
        let src = old.join(name);
        if src.exists() {
            let dst = new_dir.join(name);
            if dst.exists() {
                let _ = std::fs::rename(&dst, new_dir.join(format!("{name}.bak")));
            }
            std::fs::copy(&src, &dst).map_err(|e| format!("迁移 {name} 失败: {e}"))?;
        }
    }
    config::set_data_location(&app, Some(path.trim()))?;
    // 当前内存配置立即落到新位置；历史连接指向新库
    let cfg = state(&app).config.lock().unwrap().clone();
    config::save_config_to_disk(&app, &cfg)?;
    state(&app).history.reset();
    eprintln!("[chatlingo] data dir migrated: {} -> {}", old.display(), new_dir.display());
    Ok(serde_json::json!({ "ok": true, "dir": new_dir.to_string_lossy() }))
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(app: AppHandle, enable: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autostart = app.autolaunch();
    if enable {
        autostart.enable().map_err(|e| e.to_string())
    } else {
        autostart.disable().map_err(|e| e.to_string())
    }
}

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

// ---------- 截图翻译 ----------

/// 统一翻译面板点「截图」：截取主屏 → 选区 → 识别文字回填输入框
fn start_screenshot(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(win) = app.get_webview_window(SNIP) else { return };
        // 先截屏（窗口显示前），再弹选区
        let shot = tauri::async_runtime::spawn_blocking(|| -> Result<(Vec<u8>, u32, u32), String> {
            let monitor = xcap::Monitor::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|m| m.is_primary().unwrap_or(false))
                .ok_or("找不到主显示器")?;
            let img = monitor.capture_image().map_err(|e| e.to_string())?;
            let (w, h) = (img.width(), img.height());
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(img)
                .write_to(&mut png, image::ImageFormat::Png)
                .map_err(|e| e.to_string())?;
            Ok((png.into_inner(), w, h))
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r);

        match shot {
            Ok((png, w, h)) => {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
                let _ = win.emit("snip-image", serde_json::json!({
                    "image": format!("data:image/png;base64,{b64}"),
                    "width": w,
                    "height": h,
                }));
                // 窗口铺满主屏
                let _ = win.set_size(tauri::PhysicalSize::new(w, h));
                let _ = win.set_position(tauri::PhysicalPosition::new(0, 0));
                let _ = win.show();
                let _ = win.set_focus();
            }
            Err(e) => {
                eprintln!("[chatlingo] screenshot failed: {e}");
                if let Some(b) = app.get_webview_window(QUICKBAR) {
                    let _ = b.emit("quickbar-done", serde_json::json!({ "id": 0, "ok": false, "error": format!("截图失败：{e}") }));
                    let _ = b.show();
                }
            }
        }
    });
}

#[tauri::command]
fn cancel_screenshot(app: AppHandle) {
    if let Some(win) = app.get_webview_window(SNIP) {
        let _ = win.hide();
    }
}

/// 统一翻译面板点「截图」：选区文字识别后回填输入框
#[tauri::command]
fn request_screenshot(app: AppHandle) {
    start_screenshot(&app);
}

/// 选区完成：视觉模型提取文字 → 回填统一翻译面板输入框
#[tauri::command]
async fn submit_screenshot(app: AppHandle, image: String) -> Result<(), String> {
    // 隐藏选区窗口
    cancel_screenshot(app.clone());
    let cfg = state(&app).config.lock().unwrap().clone();
    if cfg.engine.api_key.is_empty() {
        return Err("未配置 API Key".into());
    }
    // dataURL → PNG bytes（前端 canvas 裁剪后重新编码）
    let b64 = image.trim_start_matches("data:image/png;base64,").to_string();
    let png = base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .map_err(|e| format!("图片解码失败: {e}"))?;

    eprintln!("[chatlingo] snip: extracting text via vision model ({}KB)", png.len() / 1024);

    let bar = app.get_webview_window(QUICKBAR);
    if let Some(b) = &bar {
        let _ = b.emit("quickbar-chunk-status", serde_json::json!({ "text": "截图识别中…" }));
        let _ = b.show();
        let _ = b.set_focus();
    }
    let extracted = engines::vision::extract_text(&cfg, &png).await;
    match extracted {
        Ok(text) => {
            eprintln!("[chatlingo] snip->quickbar: extracted {} chars", text.chars().count());
            if let Some(b) = &bar {
                let _ = b.emit("quickbar-ocr", serde_json::json!({ "text": text }));
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("[chatlingo] snip extract failed: {e}");
            if let Some(b) = &bar {
                let _ = b.emit("quickbar-ocr", serde_json::json!({ "text": "" }));
                let _ = b.emit("quickbar-done", serde_json::json!({ "id": 0, "ok": false, "error": format!("截图识别失败：{e}") }));
            }
            Err(e)
        }
    }
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
        eprintln!("[chatlingo] skip: too short ({} < {})", trimmed.chars().count(), cfg.clipboard.min_length);
        return;
    }
    let h = default_hash(&trimmed);
    if state(&app).written_hashes.lock().unwrap().contains(&h) {
        eprintln!("[chatlingo] skip: written by ourselves");
        return;
    }
    if detect::is_already_target(&trimmed, &cfg.output_target) {
        eprintln!("[chatlingo] skip: already target language");
        return;
    }
    if detect::code_dominant(&trimmed) {
        eprintln!("[chatlingo] skip: code dominant");
        return;
    }
    let panel = app.get_webview_window(PANEL);
    // 对比模式：LLM 与 DeepL 并行翻译，面板并列展示
    let compare = cfg.compare_mode && !cfg.engine.api_key.is_empty() && !cfg.deepl.api_key.is_empty();
    if let Some(p) = &panel {
        let _ = p.emit(
            "panel-start",
            serde_json::json!({ "original": trimmed, "outputTarget": cfg.output_target, "compare": compare }),
        );
    }
    show_panel_bottom_right(&app);
    if compare {
        translate_compare(&app, trimmed, cfg.output_target.clone()).await;
        return;
    }
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

/// 对比模式：LLM 与 DeepL 同时翻译，chunk 带 engine 标签推送
async fn translate_compare(app: &AppHandle, text: String, target: String) {
    use futures_util::StreamExt;
    let cfg = state(app).config.lock().unwrap().clone();
    eprintln!("[chatlingo] compare mode: LLM ∥ DeepL → {target}");

    let llm_cfg = cfg.clone();
    let llm_text = text.clone();
    let llm_target = target.clone();
    let llm_app = app.clone();
    let llm_task = tauri::async_runtime::spawn(async move {
        let engine = OpenAiCompatEngine::new(llm_cfg.engine.base_url, llm_cfg.engine.api_key, llm_cfg.engine.model);
        let mut stream = engine.translate_stream(TranslateParams {
            text: llm_text,
            target_lang: llm_target,
            direction: Direction::Output,
        });
        let mut full = String::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    full.push_str(&chunk);
                    if let Some(w) = llm_app.get_webview_window(PANEL) {
                        let _ = w.emit("panel-chunk", serde_json::json!({ "engine": "llm", "text": chunk }));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        if full.trim().is_empty() {
            Err("引擎返回了空结果".into())
        } else {
            Ok(full)
        }
    });

    let deepl_cfg = cfg.clone();
    let deepl_text = text.clone();
    let deepl_target = target.clone();
    let deepl_app = app.clone();
    let deepl_task = tauri::async_runtime::spawn(async move {
        let engine = engines::deepl::DeeplEngine::new(deepl_cfg.deepl.api_key);
        let mut stream = engine.translate_stream(TranslateParams {
            text: deepl_text,
            target_lang: deepl_target,
            direction: Direction::Output,
        });
        let mut full = String::new();
        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    full.push_str(&chunk);
                    if let Some(w) = deepl_app.get_webview_window(PANEL) {
                        let _ = w.emit("panel-chunk", serde_json::json!({ "engine": "deepl", "text": chunk }));
                    }
                }
                Err(e) => return Err(e),
            }
        }
        if full.trim().is_empty() {
            Err("引擎返回了空结果".into())
        } else {
            Ok(full)
        }
    });

    let (llm_res, deepl_res) = tokio::join!(llm_task, deepl_task);
    eprintln!(
        "[chatlingo] compare done: llm={} deepl={}",
        matches!(&llm_res, Ok(Ok(_))),
        matches!(&deepl_res, Ok(Ok(_)))
    );
    // 对比模式记录大模型译文到历史
    if let Ok(Ok(translated)) = &llm_res {
        state(app).history.add(app, "output", &text, translated, "llm-compare");
    }
    let llm_err = match llm_res {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e),
        Err(e) => Some(e.to_string()),
    };
    let deepl_err = match deepl_res {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e),
        Err(e) => Some(e.to_string()),
    };
    if let Some(p) = app.get_webview_window(PANEL) {
        let mut errs = vec![];
        if let Some(e) = llm_err {
            errs.push(format!("LLM: {e}"));
        }
        if let Some(e) = deepl_err {
            errs.push(format!("DeepL: {e}"));
        }
        let _ = p.emit(
            "panel-done",
            serde_json::json!({
                "ok": errs.len() < 2,
                "error": if errs.is_empty() { serde_json::Value::Null } else { serde_json::json!(errs.join("；")) },
            }),
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
    // 唯一快捷键：呼出统一翻译面板
    if let Some(s) = parse_shortcut(&cfg.quickbar.hotkey) {
        if let Err(e) = gs.register(s) {
            eprintln!("[chatlingo] hotkey register FAIL {}: {e}", cfg.quickbar.hotkey);
        }
    } else {
        eprintln!("[chatlingo] hotkey parse FAIL: {}", cfg.quickbar.hotkey);
    }
}

/// 快捷键录入期间临时注销，避免按到旧组合键触发面板
#[tauri::command]
fn suspend_hotkeys(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    eprintln!("[chatlingo] hotkeys suspended (recording)");
}

#[tauri::command]
fn resume_hotkeys(app: AppHandle) {
    register_hotkeys(&app);
    eprintln!("[chatlingo] hotkeys resumed");
}

fn on_shortcut(app: &AppHandle, shortcut: &Shortcut) {
    eprintln!("[chatlingo] shortcut fired: {shortcut:?}");
    if app.get_webview_window(QUICKBAR).map(|w| w.is_visible().unwrap_or(false)) == Some(true) {
        hide_quickbar_win(app);
    } else {
        show_quickbar(app);
    }
}

// ---------- 托盘 ----------

fn build_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let toggle_quick = MenuItem::with_id(app, "quick", "呼出翻译面板", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "翻译历史…", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle_quick, &history_item, &settings_item, &quit])?;

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
            "history" => open_history(app.clone()),
            "settings" => open_settings(app.clone()),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

// ---------- 预置配置（编译期嵌入 src-tauri/preset.json） ----------

const PRESET_JSON: &str = include_str!("../preset.json");

fn embedded_preset() -> Option<AppConfig> {
    // 预置文件里大模型 Key 为空 → 视为空白版
    match serde_json::from_str::<AppConfig>(PRESET_JSON) {
        Ok(cfg) if !cfg.engine.api_key.is_empty() => Some(cfg),
        _ => None,
    }
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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
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
            let path = config::config_path(&app.handle());
            // 预置版：本地无配置时写入内置预置配置
            if !path.exists() {
                if let Some(preset) = embedded_preset() {
                    eprintln!("[chatlingo] applying embedded preset config");
                    if let Err(e) = config::save_config_to_disk(&app.handle(), &preset) {
                        eprintln!("[chatlingo] preset write failed: {e}");
                    }
                }
            }
            let cfg = config::load_config(&app.handle());
            app.manage(AppState {
                config: Mutex::new(cfg),
                written_hashes: Mutex::new(HashSet::new()),
                cache: Mutex::new(TranslationCache::new()),
                in_flight: Mutex::new(HashSet::new()),
                history: HistoryStore::new(),
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
                // 设置/历史窗口点关闭只隐藏，不销毁，便于托盘再次唤起
                if window.label() == SETTINGS || window.label() == "history" {
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
            save_quickbar_pos,
            debug_log,
            hide_panel,
            minimize_panel,
            open_settings,
            open_history,
            history_list,
            history_clear,
            get_autostart,
            set_autostart,
            suspend_hotkeys,
            resume_hotkeys,
            get_data_info,
            set_data_dir,
            pick_folder,
            panel_ready,
            copy_text,
            paste_text,
            cancel_screenshot,
            request_screenshot,
            list_models,
            submit_screenshot,
            submit_input,
            translate_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatLingo");
}
