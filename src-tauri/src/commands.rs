// Tauri command layer — thin bridge between the frontend and the engine.
// Command names/args MUST stay in sync with src/lib/tauri.ts:
//   list_tunnels / create_tunnel / update_tunnel / delete_tunnel /
//   start_tunnel / stop_tunnel / get_state / list_states /
//   read_binary_status / install_binary

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::engine::Engine;
use crate::models::{Backend, BinaryStatus, TunnelConfig, TunnelState, TunnelStatus};
use crate::store::Store;

pub struct AppState {
    pub app: AppHandle,
    pub store: Arc<Store>,
    pub engine: Arc<Engine>,
}

impl AppState {
    /// Build the application state. Unlike the skeleton stub, `new` takes the
    /// `AppHandle`: both the engine (event emission, binary resolution) and
    /// the store (app data dir) are bound to it, and an `AppHandle` cannot be
    /// fabricated outside the Tauri runtime. Nothing else in the codebase
    /// calls the no-arg form.
    pub fn new(app: AppHandle) -> Self {
        let store = Arc::new(Store::new(data_dir(&app).join("tunnels.json")));
        let engine = Arc::new(Engine::new(app.clone()));
        Self { app, store, engine }
    }
}

/// `<app_data_dir>`, with a best-effort fallback so the app still runs if the
/// path cannot be resolved.
fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|e| {
        eprintln!("[pier] app data dir unavailable ({e}); using fallback location");
        dirs::data_dir()
            .map(|d| d.join("Pier"))
            .unwrap_or_else(std::env::temp_dir)
    })
}

/// Called from `lib.rs` on `RunEvent::Exit`: stop every running tunnel process
/// so the app never leaks child processes.
pub fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        tauri::async_runtime::block_on(state.engine.stop_all());
    }
}

/// Called once from `lib.rs` setup: build + manage `AppState`, then kick off
/// auto-start for tunnels flagged `auto_start` (async, non-blocking).
pub fn init_runtime(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(data_dir(app))?;

    let state = AppState::new(app.clone());
    let store = state.store.clone();
    let engine = state.engine.clone();
    if !app.manage(state) {
        eprintln!("[pier] AppState was already managed; keeping the existing instance");
    }

    tauri::async_runtime::spawn(async move {
        for cfg in store.load() {
            if cfg.auto_start {
                if let Err(e) = engine.clone().start(cfg).await {
                    eprintln!("[pier] auto-start failed: {e}");
                }
            }
        }
    });
    Ok(())
}

fn validate_and_fill(cfg: &mut TunnelConfig) -> Result<(), String> {
    if cfg.local_port == 0 {
        return Err("localPort must be between 1 and 65535".into());
    }
    let host = cfg.local_host.trim();
    if host.is_empty() {
        cfg.local_host = "127.0.0.1".into();
    } else {
        cfg.local_host = host.to_string();
    }
    if cfg.name.trim().is_empty() {
        cfg.name = format!("port-{}", cfg.local_port);
    }
    Ok(())
}

#[tauri::command]
pub fn list_tunnels(state: State<'_, AppState>) -> Vec<TunnelConfig> {
    state.store.load()
}

#[tauri::command]
pub fn create_tunnel(
    state: State<'_, AppState>,
    mut config: TunnelConfig,
) -> Result<TunnelConfig, String> {
    validate_and_fill(&mut config)?;
    config.id = uuid::Uuid::new_v4().to_string();
    config.created_at = chrono::Utc::now().to_rfc3339();
    state.store.add(config.clone())?;
    Ok(config)
}

#[tauri::command]
pub async fn update_tunnel(
    state: State<'_, AppState>,
    mut config: TunnelConfig,
) -> Result<TunnelConfig, String> {
    validate_and_fill(&mut config)?;
    if !state.store.update(config.clone())? {
        return Err(format!("tunnel not found: {}", config.id));
    }
    // A running tunnel keeps using the old config until restarted; stop it so
    // the change takes effect on the next explicit start.
    if let Some(st) = state.engine.snapshot(&config.id) {
        if !matches!(st.status, TunnelStatus::Stopped | TunnelStatus::Error) {
            let _ = state.engine.stop(&config.id).await;
        }
    }
    Ok(config)
}

#[tauri::command]
pub async fn delete_tunnel(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if let Some(st) = state.engine.snapshot(&id) {
        if !matches!(st.status, TunnelStatus::Stopped | TunnelStatus::Error) {
            let _ = state.engine.stop(&id).await;
        }
    }
    if !state.store.remove(&id)? {
        return Err(format!("tunnel not found: {id}"));
    }
    state.engine.remove_handle(&id);
    Ok(())
}

#[tauri::command]
pub async fn start_tunnel(
    state: State<'_, AppState>,
    id: String,
) -> Result<TunnelState, String> {
    let cfg = state
        .store
        .get(&id)?
        .ok_or_else(|| format!("tunnel not found: {id}"))?;
    state.engine.clone().start(cfg).await
}

#[tauri::command]
pub async fn stop_tunnel(state: State<'_, AppState>, id: String) -> Result<TunnelState, String> {
    match state.engine.stop(&id).await {
        Ok(st) => Ok(st),
        Err(_) => {
            // Never started in this session: report the default state instead
            // of an error, as long as the tunnel exists.
            if state.store.get(&id)?.is_some() {
                Ok(TunnelState::new(&id))
            } else {
                Err(format!("tunnel not found: {id}"))
            }
        }
    }
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>, id: String) -> Result<TunnelState, String> {
    if let Some(st) = state.engine.snapshot(&id) {
        return Ok(st);
    }
    if state.store.get(&id)?.is_some() {
        return Ok(TunnelState::new(&id));
    }
    Err(format!("tunnel not found: {id}"))
}

#[tauri::command]
pub fn list_states(state: State<'_, AppState>) -> Vec<TunnelState> {
    state
        .store
        .load()
        .into_iter()
        .map(|t| state.engine.snapshot(&t.id).unwrap_or_else(|| TunnelState::new(&t.id)))
        .collect()
}

#[tauri::command]
pub fn read_binary_status(
    _app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BinaryStatus, String> {
    crate::binman::read_status(&state.app)
}

#[tauri::command]
pub fn install_binary(
    _app: AppHandle,
    backend: Backend,
    state: State<'_, AppState>,
) -> Result<BinaryStatus, String> {
    // The download flow belongs to the system-integration milestone and the
    // stub currently errors. Report the outcome in logs, but always answer
    // with the fresh binary status so the UI can render it.
    if let Err(e) = crate::binman::install(&state.app, backend) {
        eprintln!("[pier] install {backend:?} engine failed: {e}");
    }
    crate::binman::read_status(&state.app)
}
