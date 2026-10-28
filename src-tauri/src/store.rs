use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use std::sync::Mutex;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub ts: i64,
    pub direction: String,
    pub source_text: String,
    pub translated: String,
    pub engine: String,
}

// ---------- 术语库（与 history 同库） ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryEntry {
    #[serde(default)]
    pub id: i64,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub note: String,
    #[serde(default = "glossary_default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub created_at: i64,
}

/// 建表（幂等）。HistoryStore 惰性建库与 example 直测共用。
pub fn glossary_init(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS glossary (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source TEXT NOT NULL,
            target TEXT NOT NULL,
            note TEXT DEFAULT '',
            enabled INTEGER DEFAULT 1,
            created_at INTEGER NOT NULL
        );",
    )
}

/// 匹配归一化：ASCII 范围全角→半角 + 小写
pub fn glossary_norm(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let c = match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            other => other,
        };
        // 大小写不敏感只对 ASCII 有意义；非 ASCII to_lowercase 也安全
        for lc in c.to_lowercase() {
            out.push(lc);
        }
    }
    out
}

/// 列表（source/target/note 模糊搜索）
pub fn glossary_list_conn(conn: &Connection, query: &str) -> Result<Vec<GlossaryEntry>, rusqlite::Error> {
    let like = format!("%{}%", query);
    let mut stmt = conn.prepare(
        "SELECT id, source, target, note, enabled, created_at FROM glossary
         WHERE source LIKE ?1 OR target LIKE ?1 OR note LIKE ?1
         ORDER BY id ASC",
    )?;
    let rows = stmt.query_map(rusqlite::params![like], |r| {
        Ok(GlossaryEntry {
            id: r.get(0)?,
            source: r.get(1)?,
            target: r.get(2)?,
            note: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            created_at: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// 增改：id>0 按 id 更新；否则按 source 去重（已存在则更新 target/note/enabled）
pub fn glossary_upsert_conn(conn: &Connection, e: &GlossaryEntry) -> Result<i64, rusqlite::Error> {
    if e.id > 0 {
        conn.execute(
            "UPDATE glossary SET source=?1, target=?2, note=?3, enabled=?4 WHERE id=?5",
            rusqlite::params![e.source.trim(), e.target.trim(), e.note, e.enabled as i64, e.id],
        )?;
        return Ok(e.id);
    }
    let changed = conn.execute(
        "UPDATE glossary SET target=?1, note=?2, enabled=?3 WHERE source=?4",
        rusqlite::params![e.target.trim(), e.note, e.enabled as i64, e.source.trim()],
    )?;
    if changed > 0 {
        let id: i64 = conn.query_row(
            "SELECT id FROM glossary WHERE source=?1",
            rusqlite::params![e.source.trim()],
            |r| r.get(0),
        )?;
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO glossary (source, target, note, enabled, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![e.source.trim(), e.target.trim(), e.note, e.enabled as i64, now_ts()],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn glossary_delete_conn(conn: &Connection, id: i64) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM glossary WHERE id=?1", rusqlite::params![id])
}

pub fn glossary_clear_conn(conn: &Connection) -> Result<usize, rusqlite::Error> {
    conn.execute("DELETE FROM glossary", [])
}

/// 全部启用术语（翻译注入用；同 source 冲突取 id 最小，与设计一致）
pub fn glossary_enabled_conn(conn: &Connection) -> Result<Vec<GlossaryEntry>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, source, target, note, enabled, created_at FROM glossary
         WHERE enabled=1 ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(GlossaryEntry {
            id: r.get(0)?,
            source: r.get(1)?,
            target: r.get(2)?,
            note: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            created_at: r.get(5)?,
        })
    })?;
    let mut all: Vec<GlossaryEntry> = rows.collect::<Result<_, _>>()?;
    // 同 source 多条：只留 id 最小的一条（SQL 已按 id 升序）
    let mut seen = std::collections::HashSet::new();
    all.retain(|e| seen.insert(e.source.clone()));
    Ok(all)
}

/// 命中匹配：原文子串（归一化后 contains），按 source 长度降序（长词优先），截断到 max
pub fn glossary_match_in(entries: &[GlossaryEntry], text: &str, max: usize) -> Vec<(String, String)> {
    let norm_text = glossary_norm(text);
    let mut hits: Vec<(String, String)> = entries
        .iter()
        .filter(|e| !e.source.is_empty() && norm_text.contains(&glossary_norm(&e.source)))
        .map(|e| (e.source.clone(), e.target.clone()))
        .collect();
    hits.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()).then(a.0.cmp(&b.0)));
    hits.truncate(max);
    hits
}

/// 导出 JSON（全量，含停用项）
pub fn glossary_export_conn(conn: &Connection) -> Result<String, rusqlite::Error> {
    let list = glossary_list_conn(conn, "")?;
    let items: Vec<serde_json::Value> = list
        .iter()
        .map(|e| {
            serde_json::json!({ "source": e.source, "target": e.target, "note": e.note, "enabled": e.enabled })
        })
        .collect();
    Ok(serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".into()))
}

/// 导入：JSON 数组或 CSV（source,target,note,enabled），按 source 去重更新。
/// 返回 (导入条数, 跳过条数)
pub fn glossary_import_conn(conn: &Connection, content: &str) -> Result<(usize, usize), String> {
    let trimmed = content.trim();
    let mut entries: Vec<GlossaryEntry> = Vec::new();
    if trimmed.starts_with('[') {
        let v: Vec<GlossaryEntry> =
            serde_json::from_str(trimmed).map_err(|e| format!("JSON 解析失败: {e}"))?;
        entries = v;
    } else {
        // CSV：首行可以是表头
        let mut first = true;
        for line in trimmed.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if first {
                first = false;
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("source") {
                    continue; // 表头
                }
            }
            // 简单 CSV 切分：不支持引号转义（术语场景足够）
            let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
            if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
                continue;
            }
            entries.push(GlossaryEntry {
                id: 0,
                source: parts[0].to_string(),
                target: parts[1].to_string(),
                note: parts.get(2).unwrap_or(&"").to_string(),
                enabled: parts.get(3).map(|s| *s != "0" && !s.eq_ignore_ascii_case("false")).unwrap_or(true),
                created_at: 0,
            });
        }
    }
    let mut imported = 0;
    let mut skipped = 0;
    for e in entries {
        if e.source.trim().is_empty() || e.target.trim().is_empty() {
            skipped += 1;
            continue;
        }
        glossary_upsert_conn(conn, &e).map_err(|e| e.to_string())?;
        imported += 1;
    }
    Ok((imported, skipped))
}

pub struct HistoryStore {
    conn: Mutex<Option<Connection>>,
}

impl HistoryStore {
    pub fn new() -> Self {
        HistoryStore { conn: Mutex::new(None) }
    }

    /// 关闭连接（数据目录迁移后重新在新位置打开）
    pub fn reset(&self) {
        self.conn.lock().unwrap().take();
    }

    /// 惰性建库；失败不影响主流程（历史不可用时静默跳过）
    fn with_conn<T>(&self, app: &AppHandle, f: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>) -> Option<T> {
        self.with_conn_res(app, |c| f(c).map_err(|e| e.to_string())).ok()
    }

    /// 同 with_conn，但闭包返回 String 错误（导入解析等非 SQLite 错误场景）；错误上抛
    fn with_conn_res<T>(&self, app: &AppHandle, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.conn.lock().unwrap();
        if guard.is_none() {
            match Connection::open(crate::config::data_dir(app).join("history.db")) {
                Ok(conn) => {
                    let _ = conn.execute_batch(
                        "CREATE TABLE IF NOT EXISTS history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            ts INTEGER NOT NULL,
                            direction TEXT NOT NULL,
                            source_text TEXT NOT NULL,
                            translated TEXT NOT NULL,
                            engine TEXT NOT NULL
                        );",
                    );
                    let _ = glossary_init(&conn);
                    *guard = Some(conn);
                }
                Err(e) => {
                    eprintln!("[chatlingo] history db open failed: {e}");
                    return Err(format!("数据库打开失败: {e}"));
                }
            }
        }
        match guard.as_ref() {
            Some(c) => f(c),
            None => Err("数据库不可用".into()),
        }
    }

    pub fn add(
        &self,
        app: &AppHandle,
        direction: &str,
        source: &str,
        translated: &str,
        engine: &str,
    ) {
        self.with_conn(app, |c| {
            c.execute(
                "INSERT INTO history (ts, direction, source_text, translated, engine) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![now_ts(), direction, source, translated, engine],
            )
        });
    }

    pub fn list(&self, app: &AppHandle, query: &str, limit: usize) -> Vec<HistoryEntry> {
        self.with_conn(app, |c| {
            let like = format!("%{}%", query);
            let mut stmt = c.prepare(
                "SELECT id, ts, direction, source_text, translated, engine FROM history
                 WHERE source_text LIKE ?1 OR translated LIKE ?1
                 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![like, limit as i64], |r| {
                Ok(HistoryEntry {
                    id: r.get(0)?,
                    ts: r.get(1)?,
                    direction: r.get(2)?,
                    source_text: r.get(3)?,
                    translated: r.get(4)?,
                    engine: r.get(5)?,
                })
            })?;
            rows.collect()
        })
        .unwrap_or_default()
    }

    pub fn clear(&self, app: &AppHandle) {
        self.with_conn(app, |c| c.execute("DELETE FROM history", []));
    }

    // ---------- 术语库 ----------

    pub fn glossary_list(&self, app: &AppHandle, query: &str) -> Vec<GlossaryEntry> {
        self.with_conn(app, |c| glossary_list_conn(c, query)).unwrap_or_default()
    }

    pub fn glossary_upsert(&self, app: &AppHandle, entry: &GlossaryEntry) -> Option<i64> {
        self.with_conn(app, |c| glossary_upsert_conn(c, entry))
    }

    pub fn glossary_delete(&self, app: &AppHandle, id: i64) -> Option<usize> {
        self.with_conn(app, |c| glossary_delete_conn(c, id))
    }

    pub fn glossary_clear(&self, app: &AppHandle) -> Option<usize> {
        self.with_conn(app, |c| glossary_clear_conn(c))
    }

    pub fn glossary_export(&self, app: &AppHandle) -> String {
        self.with_conn(app, |c| glossary_export_conn(c))
            .unwrap_or_else(|| "[]".into())
    }

    pub fn glossary_import(&self, app: &AppHandle, content: &str) -> Result<(usize, usize), String> {
        self.with_conn_res(app, |c| glossary_import_conn(c, content))
    }

    /// 翻译前术语匹配（总开关/截断在调用侧判断）
    pub fn glossary_match(&self, app: &AppHandle, text: &str, max: usize) -> Vec<(String, String)> {
        self.with_conn(app, |c| glossary_enabled_conn(c))
            .map(|list| glossary_match_in(&list, text, max))
            .unwrap_or_default()
    }
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn glossary_default_true() -> bool {
    true
}
