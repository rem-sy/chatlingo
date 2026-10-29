pub mod config;
pub mod detect;
pub mod engines;
pub mod store;

use config::AppConfig;
use engines::openai::OpenAiCompatEngine;
use engines::{ChunkStream, Direction, TranslationEngine, TranslateParams};
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
    /// DeepL 远端术语表资源缓存（术语库变更时置脏，下次翻译懒重建）
    deepl_glossary: Mutex<DeeplGlossaryCache>,
}

#[derive(Default)]
struct DeeplGlossaryCache {
    dirty: bool,
    id: Option<String>,
    /// 已建资源对应的 (source_lang, target_lang)
    lang_pair: Option<(String, String)>,
    /// 已建资源对应的术语内容签名
    sig: u64,
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

/// 译文缓存 / in_flight 合并共用 key（apiFormat 必须纳入：换格式不得命中旧缓存）
fn translation_cache_key(
    text: &str,
    target: &str,
    direction: Direction,
    glossary_sig: u64,
    api_format: &str,
) -> u64 {
    default_hash(&format!("{text}|{target}|{direction:?}|{glossary_sig}|{api_format}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_isolates_api_format() {
        let base = translation_cache_key("hello", "zh-CN", Direction::Output, 42, "openai-completions");
        let responses = translation_cache_key("hello", "zh-CN", Direction::Output, 42, "openai-responses");
        let anthropic = translation_cache_key("hello", "zh-CN", Direction::Output, 42, "anthropic-messages");
        assert_ne!(base, responses);
        assert_ne!(base, anthropic);
        assert_ne!(responses, anthropic);
        // 同格式同参数 → 同 key（缓存正常命中）
        assert_eq!(base, translation_cache_key("hello", "zh-CN", Direction::Output, 42, "openai-completions"));
        // 术语签名变化 → key 变化（陷阱 15 回归）
        assert_ne!(base, translation_cache_key("hello", "zh-CN", Direction::Output, 43, "openai-completions"));
    }
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
    // M6b 懒建窗口：启动只建核心交互窗口（quickbar/panel）；
    // settings/history/glossary 首唤起时创建（open_util_window），snip 首截图时创建
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
    Ok(())
}

/// 懒建工具窗口（settings/history/glossary）：存在即显示，不存在即创建（直接可见）。
/// 全部入口（托盘/单实例回调/设置页按钮）统一走这里，保证 get-or-create。
fn open_util_window(app: &AppHandle, label: &str, title: &str, html: &str, w: f64, h: f64) {
    if let Some(win) = app.get_webview_window(label) {
        let _ = win.show();
        let _ = win.set_focus();
        let refresh = match label {
            "history" => Some("history-refresh"),
            "glossary" => Some("glossary-refresh"),
            _ => None,
        };
        if let Some(ev) = refresh {
            let _ = win.emit(ev, ());
        }
        return;
    }
    match WebviewWindowBuilder::new(app, label, WebviewUrl::App(html.into()))
        .title(title)
        .inner_size(w, h)
        .resizable(true)
        .visible(true)
        .shadow(true)
        .build()
    {
        Ok(_) => eprintln!("[chatlingo] window created (lazy): {}", label),
        Err(e) => eprintln!("[chatlingo] window create FAIL (lazy): {} -> {:?}", label, e),
    }
}

/// 截图选区窗口懒建（隐藏创建，start_screenshot 再铺屏显示）
fn ensure_snip_window(app: &AppHandle) -> bool {
    if app.get_webview_window(SNIP).is_some() {
        return true;
    }
    match WebviewWindowBuilder::new(app, SNIP, WebviewUrl::App("snip.html".into()))
        .title("ChatLingo Snip")
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .shadow(false)
        .transparent(true)
        .build()
    {
        Ok(_) => {
            eprintln!("[chatlingo] window created (lazy): snip");
            true
        }
        Err(e) => {
            eprintln!("[chatlingo] window create FAIL (lazy): snip -> {:?}", e);
            false
        }
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineKind {
    Llm,
    Deepl,
    Aliyun,
}

impl EngineKind {
    fn id(self) -> &'static str {
        match self {
            EngineKind::Llm => "llm",
            EngineKind::Deepl => "deepl",
            EngineKind::Aliyun => "aliyun",
        }
    }
    fn label(self) -> &'static str {
        match self {
            EngineKind::Llm => "大模型",
            EngineKind::Deepl => "DeepL",
            EngineKind::Aliyun => "阿里云",
        }
    }
}

/// 用户启用且密钥齐全的引擎，按设置顺序
fn enabled_engines(cfg: &AppConfig) -> Vec<EngineKind> {
    let mut v = Vec::new();
    if cfg.engine.enabled && !cfg.engine.api_key.is_empty() {
        v.push(EngineKind::Llm);
    }
    if cfg.deepl.enabled && !cfg.deepl.api_key.is_empty() {
        v.push(EngineKind::Deepl);
    }
    if cfg.aliyun.enabled && !cfg.aliyun.access_key_id.is_empty() && !cfg.aliyun.access_key_secret.is_empty() {
        v.push(EngineKind::Aliyun);
    }
    v
}

/// 各引擎支持的目标语言（与前端 SUPPORTED_LANGS 保持一致）
fn supports_target(kind: EngineKind, target: &str) -> bool {
    let t = target.to_ascii_lowercase();
    match kind {
        EngineKind::Llm => true,
        EngineKind::Deepl => matches!(
            t.as_str(),
            "en" | "en-us" | "ja" | "ko" | "zh" | "zh-cn" | "zh-tw" | "fr" | "de" | "es" | "ru" | "pt" | "it" | "id"
        ),
        EngineKind::Aliyun => matches!(
            t.as_str(),
            "en" | "ja" | "ko" | "zh" | "zh-cn" | "zh-tw" | "fr" | "de" | "es" | "ru" | "pt" | "it" | "th" | "vi" | "ar"
        ),
    }
}

fn make_stream(
    kind: EngineKind,
    cfg: &AppConfig,
    text: String,
    target: String,
    direction: Direction,
    glossary: Vec<(String, String)>,
    deepl_glossary_id: Option<String>,
    source_lang: Option<String>,
) -> ChunkStream {
    let params = TranslateParams {
        text,
        target_lang: target,
        direction,
        glossary,
        glossary_id: deepl_glossary_id,
        source_lang,
    };
    match kind {
        EngineKind::Llm => {
            let engine = OpenAiCompatEngine::new(
                cfg.engine.base_url.clone(),
                cfg.engine.api_key.clone(),
                cfg.engine.model.clone(),
                cfg.engine.api_format,
            );
            engine.translate_stream(params)
        }
        EngineKind::Deepl => {
            let engine = engines::deepl::DeeplEngine::new(cfg.deepl.api_key.clone());
            engine.translate_stream(params)
        }
        EngineKind::Aliyun => {
            let engine = engines::aliyun::AliyunEngine::new(
                cfg.aliyun.access_key_id.clone(),
                cfg.aliyun.access_key_secret.clone(),
            );
            engine.translate_stream(params)
        }
    }
}

/// 单引擎流式翻译：chunk 推送到窗口；成功返回全文
async fn run_single_stream(
    app: &AppHandle,
    window: &str,
    kind: EngineKind,
    text: String,
    target: String,
    direction: Direction,
    cfg: &AppConfig,
    tag_chunks: bool,
    glossary: Vec<(String, String)>,
    deepl_glossary_id: Option<String>,
    source_lang: Option<String>,
) -> Result<String, String> {
    let win = app.get_webview_window(window).ok_or("窗口不存在")?.clone();
    let mut stream = make_stream(
        kind, cfg, text, target, direction, glossary.clone(), deepl_glossary_id, source_lang,
    );
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
                } else if tag_chunks {
                    let _ = win.emit(
                        "panel-chunk",
                        serde_json::json!({ "engine": kind.id(), "text": chunk }),
                    );
                } else {
                    let _ = win.emit("panel-chunk", serde_json::json!({ "text": chunk }));
                }
            }
            Err(e) => return Err(e),
        }
    }
    if full.trim().is_empty() {
        return Err("引擎返回了空结果".into());
    }
    check_and_warn_glossary(app, &win, window, kind, &glossary, &full);
    Ok(full)
}

/// 译后校验（传统引擎兜底）：命中术语的译法未出现在译文中 → 橙色提示行。
/// 不强行替换，避免破坏语法。仅 deepl/aliyun；llm 已走 Prompt 注入。
fn check_and_warn_glossary(
    app: &AppHandle,
    win: &tauri::WebviewWindow,
    window: &str,
    kind: EngineKind,
    glossary: &[(String, String)],
    translated: &str,
) {
    let _ = app;
    if glossary.is_empty() || kind == EngineKind::Llm {
        return;
    }
    let norm = store::glossary_norm(translated);
    let missing: Vec<String> = glossary
        .iter()
        .filter(|(_s, t)| {
            // 目标译法未出现，且原文术语仍原样留在译文里（真未生效）
            !norm.contains(&store::glossary_norm(t))
        })
        .map(|(s, t)| format!("{s} 应为 {t}"))
        .collect();
    if missing.is_empty() {
        return;
    }
    let msg = format!("术语未生效：{}", missing.join("；"));
    eprintln!("[chatlingo] glossary warn ({}): {}", kind.id(), msg);
    let payload = if window == QUICKBAR {
        serde_json::json!({ "engine": kind.id(), "text": msg })
    } else {
        serde_json::json!({ "engine": kind.id(), "text": msg })
    };
    let _ = win.emit("glossary-warn", payload);
}

/// 主入口：按启用的引擎执行翻译。
/// 单引擎 → 该引擎流式直出；多引擎 → 全部并行翻译，全部成功(或部分成功)时并列展示。
async fn run_translation(
    app: &AppHandle,
    window: &str,
    text: String,
    target: String,
    direction: Direction,
) -> Result<String, String> {
    let cfg = state(app).config.lock().unwrap().clone();
    let engines_on = enabled_engines(&cfg);
    if engines_on.is_empty() {
        return Err("未启用任何翻译引擎，请先在设置中开启".into());
    }
    // 只保留支持当前目标语言的引擎；不支持的直接跳过，不去尝试
    let engines_on: Vec<EngineKind> =
        engines_on.into_iter().filter(|k| supports_target(*k, &target)).collect();
    if engines_on.is_empty() {
        let names: Vec<&str> = enabled_engines(&cfg).iter().map(|k| k.label()).collect();
        return Err(format!("启用的引擎（{}）均不支持目标语言 {target}", names.join("、")));
    }

    // ---- 术语匹配（总开关/最大条数走配置；缓存 key 纳入术语签名 → 增删即生效） ----
    let glossary_terms: Vec<(String, String)> = if cfg.glossary.enabled {
        let t = state(app).history.glossary_match(app, &text, cfg.glossary.max_terms);
        if !t.is_empty() {
            eprintln!("[chatlingo] glossary matched {} terms", t.len());
        }
        t
    } else {
        Vec::new()
    };
    let glossary_sig = glossary_terms.iter().map(|(s, t)| format!("{s}>{t}")).collect::<Vec<_>>().join("|");
    let glossary_sig = default_hash(&glossary_sig);

    // ---- 译文缓存（key 纳入 apiFormat：换格式不得命中旧缓存，陷阱 15 同款） ----
    let cache_key = translation_cache_key(&text, &target, direction, glossary_sig, cfg.engine.api_format.as_str());
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

    // DeepL：命中术语时懒重建远端术语表资源（语言对不支持 → 降级译后校验）
    let deepl_src = detect_deepl_source(&text);
    let deepl_gid: Option<String> = if engines_on.contains(&EngineKind::Deepl) && !glossary_terms.is_empty() {
        ensure_deepl_glossary(app, &cfg, deepl_src, &target, &glossary_terms).await
    } else {
        None
    };

    let result = if engines_on.len() == 1 {
        let kind = engines_on[0];
        eprintln!("[chatlingo] engine: {}", kind.label());
        if window == PANEL {
            if let Some(p) = app.get_webview_window(PANEL) {
                let _ = p.emit(
                    "panel-start",
                    serde_json::json!({
                        "original": text,
                        "outputTarget": target,
                        "compare": false,
                    }),
                );
            }
            show_panel_bottom_right(app);
        }
        let r = run_single_stream(
            app, window, kind, text.clone(), target.clone(), direction, &cfg, false,
            glossary_terms.clone(),
            if kind == EngineKind::Deepl { deepl_gid.clone() } else { None },
            Some(deepl_src.to_string()),
        )
        .await;        if window == PANEL {
            if let Some(p) = app.get_webview_window(PANEL) {
                let _ = p.emit(
                    "panel-done",
                    match &r {
                        Ok(_) => serde_json::json!({ "ok": true }),
                        Err(e) => serde_json::json!({ "ok": false, "error": e }),
                    },
                );
            }
        }
        if let Ok(translated) = &r {
            state(app).history.add(
                app,
                if direction == Direction::Input { "input" } else { "output" },
                &text,
                translated,
                kind.id(),
            );
        }
        r
    } else {
        // 多引擎：全部并行，结果在调用窗口内堆叠/并列展示（不弹另一个面板）
        let kinds: Vec<&str> = engines_on.iter().map(|k| k.id()).collect();
        eprintln!("[chatlingo] engines: multi {:?}", kinds);
        let win = app.get_webview_window(window);
        if window == PANEL {
            if let Some(p) = &win {
                let _ = p.emit(
                    "panel-start",
                    serde_json::json!({
                        "original": text,
                        "outputTarget": target,
                        "compare": true,
                        "engines": kinds,
                    }),
                );
            }
            show_panel_bottom_right(app);
        } else if let Some(b) = &win {
            let id = ACTIVE_REQ.load(std::sync::atomic::Ordering::SeqCst);
            let _ = b.emit("quickbar-multi-start", serde_json::json!({ "id": id, "engines": kinds }));
        }
        let mut tasks = Vec::new();
        for kind in engines_on {
            let cfg2 = cfg.clone();
            let text2 = text.clone();
            let target2 = target.clone();
            let win2 = win.clone();
            let is_quickbar = window == QUICKBAR;
            let id = ACTIVE_REQ.load(std::sync::atomic::Ordering::SeqCst);
            let glossary2 = glossary_terms.clone();
            let gid2 = if kind == EngineKind::Deepl { deepl_gid.clone() } else { None };
            let src2 = Some(deepl_src.to_string());
            tasks.push(tauri::async_runtime::spawn(async move {
                let mut stream = make_stream(kind, &cfg2, text2, target2, direction, glossary2.clone(), gid2, src2);
                use futures_util::StreamExt;
                let mut full = String::new();
                while let Some(item) = stream.next().await {
                    match item {
                        Ok(chunk) => {
                            full.push_str(&chunk);
                            if let Some(w) = &win2 {
                                if is_quickbar {
                                    let _ = w.emit(
                                        "quickbar-chunk",
                                        serde_json::json!({ "id": id, "engine": kind.id(), "text": chunk }),
                                    );
                                } else {
                                    let _ = w.emit(
                                        "panel-chunk",
                                        serde_json::json!({ "engine": kind.id(), "text": chunk }),
                                    );
                                }
                            }
                        }
                        Err(e) => return (kind, Err(e)),
                    }
                }
                (kind, if full.trim().is_empty() { Err("空结果".into()) } else { Ok(full) })
            }));
        }
        let mut errs = vec![];
        let mut any_ok = false;
        let mut primary: Option<String> = None;
        for t in tasks {
            let (kind, r) = t.await.unwrap_or((EngineKind::Llm, Err("任务崩溃".into())));
            match r {
                Ok(translated) => {
                    any_ok = true;
                    if let Some(w) = &win {
                        check_and_warn_glossary(app, w, window, kind, &glossary_terms, &translated);
                    }
                    if primary.is_none() {
                        primary = Some(translated.clone());
                    }
                    state(app).history.add(
                        app,
                        if direction == Direction::Input { "input" } else { "output" },
                        &text,
                        &translated,
                        kind.id(),
                    );
                }
                Err(e) => errs.push(format!("{}: {e}", kind.label())),
            }
        }
        if window == PANEL {
            if let Some(p) = &win {
                let _ = p.emit(
                    "panel-done",
                    serde_json::json!({
                        "ok": any_ok,
                        "error": if errs.is_empty() { serde_json::Value::Null } else { serde_json::json!(errs.join("；")) },
                    }),
                );
            }
        }
        match primary {
            Some(p) => Ok(p),
            None => Err(errs.join("；")),
        }
    };

    if result.is_ok() {
        state(app).cache.lock().unwrap().put(cache_key, result.as_ref().unwrap().clone());
    }
    state(app).in_flight.lock().unwrap().remove(&cache_key);
    result
}

static ACTIVE_REQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// DeepL 源语言码：脚本检测（glossary 创建与翻译均需显式 source_lang）
fn detect_deepl_source(text: &str) -> &'static str {
    match detect::detect(text) {
        detect::Lang::Zh => "ZH",
        detect::Lang::Ja => "JA",
        detect::Lang::Ko => "KO",
        detect::Lang::Latin | detect::Lang::Other => "EN",
    }
}

/// DeepL 远端术语表懒重建：
/// 脏（术语变更/首次）或语言对/内容签名变化 → 删旧建新；
/// 语言对不受支持（HTTP 400）→ 记住失败状态不再反复重试，返回 None 走译后校验。
async fn ensure_deepl_glossary(
    app: &AppHandle,
    cfg: &AppConfig,
    src: &str,
    target: &str,
    terms: &[(String, String)],
) -> Option<String> {
    let Some(tgt) = engines::deepl::to_deepl_lang(target) else { return None };
    let sig = default_hash(&terms.iter().map(|(s, t)| format!("{s}>{t}")).collect::<Vec<_>>().join("|"));
    {
        let st = state(app);
        let st = st.deepl_glossary.lock().unwrap();
        if !st.dirty && st.id.is_some() && st.lang_pair == Some((src.into(), tgt.into())) && st.sig == sig {
            return st.id.clone();
        }
    }
    let engine = engines::deepl::DeeplEngine::new(cfg.deepl.api_key.clone());
    // 删旧资源防堆积
    let old = { state(app).deepl_glossary.lock().unwrap().id.clone() };
    if let Some(old_id) = old {
        engine.delete_glossary(&old_id).await;
    }
    let tsv = terms.iter().map(|(s, t)| format!("{s}\t{t}")).collect::<Vec<_>>().join("\n");
    eprintln!("[chatlingo] deepl glossary create: {src}->{tgt}, {} entries", terms.len());
    match engine.create_glossary(src, tgt, &tsv).await {
        Ok(id) => {
            *state(app).deepl_glossary.lock().unwrap() = DeeplGlossaryCache {
                dirty: false,
                id: Some(id.clone()),
                lang_pair: Some((src.into(), tgt.into())),
                sig,
            };
            Some(id)
        }
        Err(e) => {
            eprintln!("[chatlingo] deepl glossary create FAILED (fallback to post-check): {e}");
            // 记住该语言对失败，避免每次翻译都撞 400
            *state(app).deepl_glossary.lock().unwrap() = DeeplGlossaryCache {
                dirty: false,
                id: None,
                lang_pair: Some((src.into(), tgt.into())),
                sig,
            };
            None
        }
    }
}

// ---------- 自动更新 ----------

/// 启动延迟 8s 静默检查：失败静默（仅 eprintln）；发现新版本 → 系统通知 + 通知设置页
fn spawn_updater_silent_check(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(8)).await;
        eprintln!("[chatlingo] updater: silent check start");
        use tauri_plugin_updater::UpdaterExt;
        let checked = match app.updater() {
            Ok(u) => u.check().await.map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        match checked {
            Ok(Some(update)) => {
                eprintln!("[chatlingo] updater: new version {} available", update.version);
                // 系统通知（尽力而为，失败不影响）
                use tauri_plugin_notification::NotificationExt;
                let _ = app
                    .notification()
                    .builder()
                    .title("ChatLingo 发现新版本")
                    .body(format!("v{} 可用，点击托盘「设置…」→「检查更新」安装", update.version))
                    .show();
                // 设置页在打开时监听该事件展示提示条
                if let Some(win) = app.get_webview_window(SETTINGS) {
                    let _ = win.emit(
                        "updater-available",
                        serde_json::json!({ "version": update.version, "notes": update.body.clone().unwrap_or_default() }),
                    );
                }
            }
            Ok(None) => eprintln!("[chatlingo] updater: up to date"),
            Err(e) => eprintln!("[chatlingo] updater: silent check failed (ignore): {e}"),
        }
    });
}

/// 托盘「检查更新…」：打开设置页并触发前端手动检查流程
/// （窗口可能刚懒建，事件延迟发送等前端 listener 就绪）
#[tauri::command]
fn check_update(app: AppHandle) {
    open_util_window(&app, SETTINGS, "ChatLingo 设置", "settings.html", 680.0, 720.0);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        if let Some(win) = app.get_webview_window(SETTINGS) {
            let _ = win.emit("updater-check", ());
        }
    });
}

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
    let engine = OpenAiCompatEngine::new(cfg.engine.base_url, cfg.engine.api_key, cfg.engine.model, cfg.engine.api_format);
    match engine.list_model_ids().await {
        Ok(ids) => Ok(serde_json::json!({ "models": ids })),
        Err(e) => Err(e),
    }
}

#[tauri::command]
async fn test_engine(app: AppHandle) -> Result<Value, String> {
    let cfg = state(&app).config.lock().unwrap().clone();
    // 未启用的引擎直接标「未启用」，不算失败
    let llm = if !cfg.engine.enabled {
        (false, "未启用".to_string())
    } else if cfg.engine.api_key.is_empty() {
        (false, "未填写 Key".to_string())
    } else {
        let engine = OpenAiCompatEngine::new(cfg.engine.base_url.clone(), cfg.engine.api_key.clone(), cfg.engine.model.clone(), cfg.engine.api_format);
        match engine.quick_check().await {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    let deepl = if !cfg.deepl.enabled {
        (false, "未启用".to_string())
    } else if cfg.deepl.api_key.is_empty() {
        (false, "未填写 Key".to_string())
    } else {
        let engine = engines::deepl::DeeplEngine::new(cfg.deepl.api_key.clone());
        match engine.quick_check().await {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    let tencent = if !cfg.aliyun.enabled {
        (false, "未启用".to_string())
    } else if cfg.aliyun.access_key_id.is_empty() || cfg.aliyun.access_key_secret.is_empty() {
        (false, "未填写密钥".to_string())
    } else {
        let engine = engines::aliyun::AliyunEngine::new(
            cfg.aliyun.access_key_id.clone(),
            cfg.aliyun.access_key_secret.clone(),
        );
        match engine.quick_check().await {
            Ok(()) => (true, String::new()),
            Err(e) => (false, e),
        }
    };
    let ok = llm.0 || deepl.0 || tencent.0;
    let fmt = |name: &str, r: (bool, String)| -> String {
        if r.0 {
            format!("{name}: ✓")
        } else {
            format!("{name}: {}", r.1)
        }
    };
    let parts = vec![fmt("大模型", llm), fmt("DeepL", deepl), fmt("腾讯云", tencent)];
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
    open_util_window(&app, SETTINGS, "ChatLingo 设置", "settings.html", 680.0, 720.0);
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
    open_util_window(&app, "history", "ChatLingo 翻译历史", "history.html", 560.0, 640.0);
}

// ---------- 术语库命令 ----------

/// DeepL 远端资源标脏（术语增删改/导入后调用）
fn mark_deepl_glossary_dirty(app: &AppHandle) {
    state(app).deepl_glossary.lock().unwrap().dirty = true;
}

#[tauri::command]
fn glossary_list(app: AppHandle, query: String) -> Vec<store::GlossaryEntry> {
    let list = state(&app).history.glossary_list(&app, &query);
    list
}

#[tauri::command]
fn glossary_upsert(app: AppHandle, entry: store::GlossaryEntry) -> Result<i64, String> {
    if entry.source.trim().is_empty() || entry.target.trim().is_empty() {
        return Err("原文与译法均不能为空".into());
    }
    let r = state(&app).history.glossary_upsert(&app, &entry).ok_or("数据库不可用")?;
    mark_deepl_glossary_dirty(&app);
    Ok(r)
}

#[tauri::command]
fn glossary_delete(app: AppHandle, id: i64) -> Result<usize, String> {
    let r = state(&app).history.glossary_delete(&app, id).ok_or("数据库不可用")?;
    mark_deepl_glossary_dirty(&app);
    Ok(r)
}

#[tauri::command]
fn glossary_clear(app: AppHandle) -> Result<usize, String> {
    let r = state(&app).history.glossary_clear(&app).ok_or("数据库不可用")?;
    mark_deepl_glossary_dirty(&app);
    Ok(r)
}

#[tauri::command]
fn glossary_import(app: AppHandle, content: String) -> Result<Value, String> {
    let (imported, skipped) = state(&app).history.glossary_import(&app, &content)?;
    mark_deepl_glossary_dirty(&app);
    Ok(serde_json::json!({ "imported": imported, "skipped": skipped }))
}

#[tauri::command]
fn glossary_export(app: AppHandle) -> Result<String, String> {
    Ok(state(&app).history.glossary_export(&app))
}

#[tauri::command]
fn open_glossary(app: AppHandle) {
    open_util_window(&app, "glossary", "ChatLingo 术语库", "glossary.html", 640.0, 620.0);
}

/// 兜底：浏览器打开 Release 页（更新检查/下载失败时）
#[tauri::command]
fn open_release_page() {
    let url = "https://github.com/rem-sy/chatlingo/releases/latest";
    #[cfg(windows)]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(not(windows))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
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
        // 截图选区窗口懒建；首建时等 webview 加载完再发图
        if app.get_webview_window(SNIP).is_none() {
            if !ensure_snip_window(&app) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(900)).await;
        }
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
    // panel-start / compare / panel-done 均由 run_translation 统一负责
    let _ = run_translation(&app, PANEL, trimmed, cfg.output_target.clone(), Direction::Output).await;
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
    let check_update_item = MenuItem::with_id(app, "check_update", "检查更新…", true, None::<&str>)?;
    let glossary_item = MenuItem::with_id(app, "glossary", "术语库…", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "翻译历史…", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&toggle_quick, &check_update_item, &glossary_item, &history_item, &settings_item, &quit],
    )?;

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
            "check_update" => check_update(app.clone()),
            "glossary" => open_glossary(app.clone()),
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

/// 渠道预设列表（编译期嵌入 src-tauri/providers.json，前端读同一份）
const PROVIDERS_JSON: &str = include_str!("../providers.json");

#[tauri::command]
fn list_providers() -> Result<Value, String> {
    serde_json::from_str(PROVIDERS_JSON).map_err(|e| e.to_string())
}

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
            // 二次启动：唤起设置窗口提示已有实例在运行（懒建兜底）
            open_util_window(app, SETTINGS, "ChatLingo 设置", "settings.html", 680.0, 720.0);
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_notification::init())
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
                deepl_glossary: Mutex::new(DeeplGlossaryCache { dirty: true, ..Default::default() }),
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
            eprintln!("[chatlingo] setup: scheduling updater silent check");
            spawn_updater_silent_check(app.handle().clone());
            eprintln!("[chatlingo] setup: done");
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 设置/历史窗口点关闭只隐藏，不销毁，便于托盘再次唤起
                if window.label() == SETTINGS || window.label() == "history" || window.label() == "glossary" {
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
            open_glossary,
            glossary_list,
            glossary_upsert,
            glossary_delete,
            glossary_clear,
            glossary_import,
            glossary_export,
            history_list,
            history_clear,
            get_autostart,
            set_autostart,
            check_update,
            open_release_page,
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
            list_providers,
            submit_screenshot,
            submit_input,
            translate_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running ChatLingo");
}
