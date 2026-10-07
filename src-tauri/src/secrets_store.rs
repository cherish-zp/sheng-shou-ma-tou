// Secret storage — local SQLite database (v0.2.0).
//
// Previously secrets lived in the macOS keychain. An unsigned binary that is
// rebuilt/replaced (every dev build, every release) is treated as a different
// application by the keychain ACL, which popped a password prompt on every
// read. A single SQLite file under the app data dir (same user-level
// protection as tunnels.json/servers.json) removes that friction entirely;
// it is the storage model most open-source tunneling tools use.
//
// Schema: one `secrets` table of key/value rows, keyed by the same account
// strings the keyring layer used (`cf-{id}`, `cf-tunnel-token-{id}`,
// `ssh-secret-{id}`, `frps-token-{id}`, `tunnel-auth-{id}`, …) so every
// call site keeps working with an account-string-only API.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use tauri::Manager;

const DB_FILE: &str = "secrets.db";

static DB_PATH: OnceLock<PathBuf> = OnceLock::new();
static CONNECTION: Mutex<Option<rusqlite::Connection>> = Mutex::new(None);

/// Cache the database location (called once from `commands::init_runtime`).
pub fn init(app: &tauri::AppHandle) {
    let _ = DB_PATH.set(
        app.path()
            .app_data_dir()
            .map(|d| d.join(DB_FILE))
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .map(|d| d.join("圣手码头").join(DB_FILE))
                    .unwrap_or_else(|| std::env::temp_dir().join(DB_FILE))
            }),
    );
}

fn db_path() -> PathBuf {
    DB_PATH
        .get()
        .cloned()
        .unwrap_or_else(|| std::env::temp_dir().join(DB_FILE))
}

fn connection() -> Result<MutexGuard, String> {
    let mut guard = CONNECTION
        .lock()
        .map_err(|e| format!("secrets store lock poisoned: {e}"))?;
    if guard.is_none() {
        let path = db_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("无法创建数据目录 {}: {e}", parent.display()))?;
        }
        let conn = rusqlite::Connection::open(&path)
            .map_err(|e| format!("无法打开秘密数据库 {}: {e}", path.display()))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS secrets (
                 account TEXT PRIMARY KEY,
                 value   TEXT NOT NULL
             );",
        )
        .map_err(|e| format!("无法初始化秘密数据库：{e}"))?;
        *guard = Some(conn);
    }
    Ok(MutexGuard { inner: &CONNECTION })
}

// A tiny guard wrapper so callers never touch the raw static.
struct MutexGuard {
    inner: &'static Mutex<Option<rusqlite::Connection>>,
}

impl MutexGuard {
    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Option<rusqlite::Connection>>, String> {
        self.inner.lock().map_err(|e| format!("secrets store lock poisoned: {e}"))
    }
}

fn with_conn<T>(f: impl FnOnce(&rusqlite::Connection) -> Result<T, rusqlite::Error>) -> Result<T, String> {
    let guard = connection()?;
    let mut inner = guard.lock()?;
    let conn = inner.as_mut().ok_or("secrets store not initialized")?;
    f(conn).map_err(|e| format!("秘密数据库操作失败：{e}"))
}

/// Read one secret. `Err` with a human-readable message when absent.
pub fn get(account: &str) -> Result<String, String> {
    with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM secrets WHERE account = ?1")?;
        let value: Option<String> = stmt
            .query_row([account], |row| row.get(0))
            .map(Some)
            .or_else(|e| {
                if e == rusqlite::Error::QueryReturnedNoRows {
                    Ok(None)
                } else {
                    Err(e)
                }
            })?;
        value.ok_or(rusqlite::Error::InvalidQuery)
    })
    .map_err(|_| format!("本地秘密库中未保存 {account} 对应的秘密（尚未设置或已被删除）"))
}

/// Read one secret if present (`Ok(None)` when absent).
pub fn try_get(account: &str) -> Result<Option<String>, String> {
    match get(account) {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

/// Insert or update one secret.
pub fn set(account: &str, value: &str) -> Result<(), String> {
    with_conn(|conn| {
        conn.execute(
            "INSERT INTO secrets(account, value) VALUES(?1, ?2)
             ON CONFLICT(account) DO UPDATE SET value = excluded.value",
            [account, value],
        )?;
        Ok(())
    })
}

/// Delete one secret (no-op when absent).
pub fn delete(account: &str) -> Result<(), String> {
    with_conn(|conn| {
        conn.execute("DELETE FROM secrets WHERE account = ?1", [account])?;
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn sqlite_surface_roundtrip() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS secrets (account TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        conn.execute("INSERT INTO secrets(account, value) VALUES('a', 'v1')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO secrets(account, value) VALUES('a', 'v2') ON CONFLICT(account) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();
        let v: String = conn
            .query_row("SELECT value FROM secrets WHERE account='a'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, "v2");
        conn.execute("DELETE FROM secrets WHERE account='a'", []).unwrap();
        let missing: Result<String, _> =
            conn.query_row("SELECT value FROM secrets WHERE account='a'", [], |r| r.get(0));
        assert!(missing.is_err());
    }
}
