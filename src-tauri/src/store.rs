use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

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

pub struct HistoryStore {
    conn: Mutex<Option<Connection>>,
}

impl HistoryStore {
    pub fn new() -> Self {
        HistoryStore { conn: Mutex::new(None) }
    }

    fn init_path(app: &AppHandle) -> PathBuf {
        let dir = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| PathBuf::from("."));
        std::fs::create_dir_all(&dir).ok();
        dir.join("history.db")
    }

    /// 惰性建库；失败不影响主流程（历史不可用时静默跳过）
    fn with_conn<T>(&self, app: &AppHandle, f: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>) -> Option<T> {
        let mut guard = self.conn.lock().unwrap();
        if guard.is_none() {
            match Connection::open(Self::init_path(app)) {
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
                    *guard = Some(conn);
                }
                Err(e) => {
                    eprintln!("[chatlingo] history db open failed: {e}");
                    return None;
                }
            }
        }
        guard.as_ref().and_then(|c| f(c).ok())
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
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
