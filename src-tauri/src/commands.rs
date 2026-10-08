// Tauri command layer — thin bridge between the frontend and the engine.
// Command names/args MUST stay in sync with src/lib/tauri.ts:
//   list_tunnels / create_tunnel / update_tunnel / delete_tunnel /
//   start_tunnel / stop_tunnel / get_state / list_states /
//   read_binary_status / install_binary

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Manager, State};

use crate::engine::Engine;
use crate::models::{Backend, BinaryStatus, TunnelConfig, TunnelState, TunnelStatus, TunnelType};
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
            .map(|d| d.join(crate::brand::DISPLAY_NAME_ZH))
            .unwrap_or_else(std::env::temp_dir)
    })
}


/// One-time migration: copy every keychain secret we can address from the
/// local config files (tunnels + servers) into the SQLite secret store.
/// Reading the keychain may show ONE final authorization prompt; after this
/// the app never touches the keychain again.
fn migrate_keychain_secrets(app: &AppHandle) {
    use crate::servers_store as ss;


    // Direct keyring reads — the ss::/cloudflare:: helpers now read the
    // SQLite store, so the migration must go to the source itself.
    fn legacy_get(account: &str) -> Option<String> {
        let entry = keyring::Entry::new("com.masterfulhands.pier", account).ok()?;
        match entry.get_password() {
            Ok(v) => Some(v),
            Err(keyring::Error::NoEntry) => None,
            Err(_) => None,
        }
    }

    let mut migrated = 0usize;
    // SSH secrets + frps tokens per server
    for srv in ss::load_servers(app) {
        for account in [
            ss::ssh_secret_account(&srv.id),
            ss::frps_token_account(&srv.id),
        ] {
            if let Some(v) = legacy_get(&account) {
                if crate::secrets_store::try_get(&account).ok().flatten().is_none() {
                    let _ = crate::secrets_store::set(&account, &v);
                    migrated += 1;
                }
            }
        }
    }
    // Tunnel auth passwords + Cloudflare tokens per tunnel
    let tunnels_path = data_dir(app).join("tunnels.json");
    if let Ok(text) = std::fs::read_to_string(&tunnels_path) {
        if let Ok(configs) = serde_json::from_str::<Vec<crate::models::TunnelConfig>>(&text) {
            for cfg in configs {
                let entries = [
                    format!("tunnel-auth-{}", cfg.id),
                    format!("cf-{}", cfg.id),
                    format!("cf-tunnel-token-{}", cfg.id),
                    format!("frps-token-cf-{}", cfg.id),
                ];
                for account in entries {
                    if let Some(v) = legacy_get(&account) {
                        if crate::secrets_store::try_get(&account).ok().flatten().is_none() {
                            let _ = crate::secrets_store::set(&account, &v);
                            migrated += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!("[pier] 钥匙串迁移完成：{migrated} 条秘密已迁入本地秘密库");
}

/// Called from `lib.rs` on `RunEvent::Exit`: stop every running tunnel process
/// so the app never leaks child processes.
pub fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        tauri::async_runtime::block_on(state.engine.stop_all());
    }
}

/// v0.1.1 更名（Pier → 圣手码头）迁移：bundle identifier 变更后
/// `app_data_dir` 从 `com.masterfulhands.pier` 变为新路径，把旧目录中的
/// 隧道/服务器配置与引擎二进制搬过来。幂等：新目录已有配置则跳过。
/// keyring 的服务名是固定字符串（不随 identifier 变），SSH 密码与
/// frps token / 隧道鉴权密码无需迁移。
fn migrate_legacy_data_dir(app: &AppHandle) {
    let new_dir = data_dir(app);
    if new_dir.join("tunnels.json").exists() || new_dir.join("servers.json").exists() {
        return;
    }
    let Some(legacy_dir) = dirs::data_dir().map(|d| d.join(crate::brand::LEGACY_IDENTIFIER))
    else {
        return;
    };
    if !legacy_dir.exists() {
        return;
    }
    let entries = match std::fs::read_dir(&legacy_dir) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[pier] legacy data dir unreadable: {e}");
            return;
        }
    };
    let _ = std::fs::create_dir_all(&new_dir);
    for entry in entries.flatten() {
        let target = new_dir.join(entry.file_name());
        if target.exists() {
            continue;
        }
        if let Err(e) = std::fs::rename(entry.path(), &target) {
            // Cross-device fallback: copy instead of rename.
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let _ = copy_dir_all(&entry.path(), &target);
            } else if let Err(e2) = std::fs::copy(entry.path(), &target) {
                eprintln!("[pier] migrate {}: {e} / copy: {e2}", entry.path().display());
            }
        }
    }
    let _ = std::fs::remove_dir_all(&legacy_dir);
    eprintln!(
        "[pier] migrated legacy data dir {} → {}",
        legacy_dir.display(),
        new_dir.display()
    );
}

fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Called once from `lib.rs` setup: build + manage `AppState`, then kick off
/// auto-start for tunnels flagged `auto_start` (async, non-blocking).
pub fn init_runtime(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    migrate_legacy_data_dir(app);
    // v0.2.0: secrets live in local SQLite now — pull any pre-existing
    // keychain entries over once (may prompt once for keychain access),
    // then the app never touches the keychain again.
    crate::secrets_store::init(app);
    migrate_keychain_secrets(app);
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
    // UDP typed-port tunnels are frp-only (bore is TCP-only, Cloudflare has
    // no plain-UDP ingress) and need a remote port on the server.
    if cfg.tunnel_type == TunnelType::Udp {
        if cfg.backend != Backend::Frp {
            return Err(
                "UDP 端口转发仅支持自建服务器通道 (UDP forwarding requires the frp backend)".into(),
            );
        }
        if cfg.remote_port.is_none() {
            return Err("UDP 隧道缺少远程端口 (remotePort is required for udp tunnels)".into());
        }
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

// ---------------------------------------------------------------------------
// M2: servers (self-hosted frps), frpc import, diagnosis
// ---------------------------------------------------------------------------

use crate::models::{DeployResult, Diagnosis, ServerConfig, ServerInput, ServerStatus};

#[tauri::command]
pub fn list_servers(app: AppHandle) -> Vec<ServerConfig> {
    crate::servers_store::load_servers(&app)
}

#[tauri::command]
pub fn add_server(app: AppHandle, input: ServerInput) -> Result<ServerConfig, String> {
    if input.host.trim().is_empty() || input.username.trim().is_empty() {
        return Err("host and username are required".into());
    }
    let server = ServerConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name: if input.name.trim().is_empty() {
            input.host.clone()
        } else {
            input.name.trim().to_string()
        },
        host: input.host.trim().to_string(),
        port: input.port,
        username: input.username.trim().to_string(),
        auth_kind: input.auth_kind,
        frps_bind_port: input.frps_bind_port.unwrap_or(7000),
        frps_vhost_http_port: input.frps_vhost_http_port.unwrap_or(8080),
        frps_vhost_https_port: input.frps_vhost_https_port.unwrap_or(8443),
        frps_dashboard_port: input.frps_dashboard_port.unwrap_or(7500),
        subdomain_host: input.subdomain_host.map(|s| s.trim().to_string()),
        frps_proxy_port_start: input.frps_proxy_port_start,
        frps_proxy_port_end: input.frps_proxy_port_end,
        deployed: false,
        frps_version: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    // A proxy port range must be well-formed: start <= end inside 1..=65535.
    if let (Some(start), Some(end)) = (server.frps_proxy_port_start, server.frps_proxy_port_end) {
        if start == 0 || start > end {
            return Err(
                "转发端口段无效 (proxy port range is invalid: need 1 <= start <= end <= 65535)"
                    .into(),
            );
        }
    }
    crate::servers_store::set_ssh_secret(&app, &server.id, &input.secret)?;
    crate::servers_store::save_server(&app, &server)?;
    Ok(server)
}

#[tauri::command]
pub fn remove_server(app: AppHandle, id: String) -> Result<(), String> {
    crate::servers_store::remove_server(&app, &id).map(|_| ())
}

#[tauri::command]
pub async fn test_server(app: AppHandle, id: String) -> Result<ServerStatus, String> {
    let server = crate::servers_store::load_servers(&app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "server not found".to_string())?;
    // Connection probe lives in the SSH layer; deploy module owns it.
    let reachable = crate::frp_deploy::status(&app, server).await.is_ok();
    Ok(ServerStatus {
        server_id: id,
        reachable,
        frps_running: false,
        frps_version: None,
        detail: None,
    })
}

/// Kick off deployment in the background; progress arrives via the
/// "deploy://progress" events and completion via "deploy://done".
#[tauri::command]
pub async fn deploy_server(app: AppHandle, id: String) -> Result<DeployResult, String> {
    let server = crate::servers_store::load_servers(&app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "server not found".to_string())?;
    let handle = tauri::async_runtime::spawn(crate::frp_deploy::deploy(app.clone(), server));
    handle
        .await
        .map_err(|e| format!("deploy task failed: {e}"))
}

#[tauri::command]
pub async fn undeploy_server(app: AppHandle, id: String) -> Result<(), String> {
    let server = crate::servers_store::load_servers(&app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "server not found".to_string())?;
    crate::frp_deploy::undeploy(&app, server).await
}

#[tauri::command]
pub async fn get_server_status(app: AppHandle, id: String) -> Result<ServerStatus, String> {
    let server = crate::servers_store::load_servers(&app)
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "server not found".to_string())?;
    let frps_running = crate::frp_deploy::status(&app, server.clone()).await.unwrap_or(false);
    Ok(ServerStatus {
        server_id: id,
        reachable: true,
        frps_running,
        frps_version: server.frps_version,
        detail: None,
    })
}

#[tauri::command]
pub fn import_frpc_config(text: String, server_id: Option<String>) -> Result<Vec<TunnelConfig>, String> {
    crate::frp_import::parse_frpc_config(&text, server_id.as_deref())
}

#[tauri::command]
pub fn diagnose_tunnel(state: State<'_, AppState>, id: String) -> Result<Diagnosis, String> {
    let snap = state.engine.snapshot(&id).ok_or("tunnel not found")?;
    let logs = state.engine.recent_logs(&id, 200);
    Ok(crate::diagnostics::diagnose(&id, snap.error.as_deref(), &logs))
}

/// Set (Some) or clear (None) a tunnel's basic-auth password. The password
/// never enters tunnels.json — only the OS keychain.
#[tauri::command]
pub fn set_tunnel_auth(app: AppHandle, id: String, password: Option<String>) -> Result<(), String> {
    match password {
        Some(p) if !p.is_empty() => {
            crate::servers_store::set_tunnel_auth_password(&app, &id, &p)
        }
        _ => crate::servers_store::delete_tunnel_auth_password(&app, &id),
    }
}

// ---------------------------------------------------------------------------
// Cloudflare Named Tunnel (v0.2.0) — fixed hostnames
// ---------------------------------------------------------------------------

use crate::models::{CfAccount, CfProvisionInput, CfZone};

#[tauri::command]
pub fn cf_verify_token(token: String) -> Result<Vec<CfAccount>, String> {
    crate::cloudflare::verify_token(&token)
}

#[tauri::command]
pub fn cf_list_zones(token: String) -> Result<Vec<CfZone>, String> {
    crate::cloudflare::list_zones(&token)
}

/// Provision a fixed-hostname tunnel end-to-end and store it. The API token
/// and the tunnel-run token both go to the OS keychain, never into
/// tunnels.json.
#[tauri::command]
pub fn cf_provision(
    state: State<'_, AppState>,
    input: CfProvisionInput,
) -> Result<TunnelConfig, String> {
    if input.subdomain.trim().is_empty() {
        return Err("subdomain is required".into());
    }
    let cfg = crate::cloudflare::provision(&input)?;
    let run_token = crate::cloudflare::get_tunnel_run_token(&cfg.id)?;
    crate::cloudflare::set_api_token(&cfg.id, &input.token)?;
    crate::cloudflare::set_tunnel_run_token(&cfg.id, &run_token)?;
    state.store.add(cfg.clone())?;
    Ok(cfg)
}

/// Change a named tunnel's fixed hostname. Uses the stored API token; the
/// remote ingress + CNAME are re-pointed and the local config updated.
#[tauri::command]
pub fn cf_update_hostname(
    state: State<'_, AppState>,
    id: String,
    zone_id: String,
    subdomain: String,
) -> Result<TunnelConfig, String> {
    let subdomain = subdomain.trim().to_lowercase();
    if subdomain.is_empty() {
        return Err("subdomain is required".into());
    }
    let mut cfg = state
        .store
        .load()
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("tunnel not found")?;
    let old_hostname = cfg
        .cf_hostname
        .clone()
        .ok_or("tunnel is not a Cloudflare fixed-hostname tunnel")?;
    let account_id = cfg
        .cf_account_id
        .clone()
        .ok_or("tunnel is not a Cloudflare fixed-hostname tunnel")?;
    let cf_tunnel_id = cfg
        .cf_tunnel_id
        .clone()
        .ok_or("tunnel is not a Cloudflare fixed-hostname tunnel")?;
    let token = crate::cloudflare::stored_api_token(&id)?;

    let new_hostname = crate::cloudflare::update_hostname(
        &token,
        &account_id,
        &cf_tunnel_id,
        &zone_id,
        &old_hostname,
        &subdomain,
    )?;
    cfg.cf_hostname = Some(new_hostname.clone());
    cfg.name = subdomain;
    cfg.cf_account_id = Some({
        // zone may live under a different account than before — re-resolve
        crate::cloudflare::zone_account(&token, &zone_id)
    });
    state.store.update(cfg.clone())?;
    Ok(cfg)
}

/// Stored API token for a tunnel (frontend eye-reveal).
#[tauri::command]
pub fn cf_get_api_token(id: String) -> Result<String, String> {
    crate::cloudflare::stored_api_token(&id)
}

/// Zones visible to a STORED tunnel's API token (edit-mode dropdown).
#[tauri::command]
pub fn cf_list_zones_stored(id: String) -> Result<Vec<crate::models::CfZone>, String> {
    let token = crate::cloudflare::stored_api_token(&id)?;
    crate::cloudflare::list_zones(&token)
}

/// Tear down the remote tunnel (and optionally its DNS record) and remove
/// the local tunnel config.
#[tauri::command]
pub fn cf_deprovision(state: State<'_, AppState>, id: String, delete_dns: bool) -> Result<(), String> {
    let cfg = state
        .store
        .load()
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("tunnel not found")?;
    if cfg.backend != crate::models::Backend::CloudflareNamed {
        return Err("only Cloudflare fixed-hostname tunnels have cloud resources".into());
    }
    let hostname = cfg
        .cf_hostname
        .clone()
        .ok_or("tunnel is not a Cloudflare fixed-hostname tunnel")?;
    let cf_tunnel_id = cfg
        .cf_tunnel_id
        .clone()
        .ok_or("tunnel is not a Cloudflare fixed-hostname tunnel")?;
    let token = crate::cloudflare::stored_api_token(&id)?;
    let zone_id = crate::cloudflare::resolve_zone_id(&token, &hostname)?;
    crate::cloudflare::deprovision(&token, &zone_id, &cf_tunnel_id, delete_dns)?;
    crate::cloudflare::delete_stored_tokens(&id);
    state.store.remove(&id)?;
    Ok(())
}
