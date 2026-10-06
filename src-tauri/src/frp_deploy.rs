// R3 IMPLEMENTS: one-click frps deployment over SSH.
//
// Flow (every step emits "deploy://progress"):
//   connect -> probe (root/sudo, arch, os-release, systemd) -> ports
//   (ss -tlnp precheck) -> download (VPS-side curl with ghproxy fallback,
//   else local download + SFTP upload; sha256 verify) -> config (random
//   token -> keychain, write /opt/pier/frps.toml) -> systemd (unit with
//   product prefix, enable --now) -> firewall (ufw/firewalld, never
//   setenforce 0) -> cloud security-group hint via metadata endpoints ->
//   verify (local curl + frpc connect).
use tauri::AppHandle;

use crate::models::{DeployResult, ServerConfig};

/// Run the full deployment for `server`. Long-running; call from a spawned
/// task and report progress via the AppHandle. Idempotent: re-deploying
/// upgrades in place, and existing foreign frps installations are detected
/// and reported rather than clobbered.
pub async fn deploy(app: AppHandle, server: ServerConfig) -> DeployResult {
    let _ = &app;
    DeployResult {
        server_id: server.id,
        ok: false,
        error: Some("deploy: not implemented".into()),
        token: None,
    }
}

/// Stop and remove the Pier-managed frps service on the server.
pub async fn undeploy(_app: &AppHandle, _server: ServerConfig) -> Result<(), String> {
    Err("deploy: not implemented".into())
}

/// Query frps status (systemd is-active, version) on the server.
pub async fn status(_app: &AppHandle, _server: ServerConfig) -> Result<bool, String> {
    Err("deploy: not implemented".into())
}
