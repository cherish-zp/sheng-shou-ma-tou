// R3 IMPLEMENTS: persistent server registry (`servers.json`) + keychain
// storage for SSH passwords / frps tokens (keyring crate).
//
// The signatures below are the contract the rest of the app codes against;
// keep them stable. Secrets NEVER live in servers.json nor cross the IPC
// boundary back to the frontend.
use crate::models::ServerConfig;

/// Load all servers from `<app_data_dir>/servers.json`.
pub fn load_servers(_app: &tauri::AppHandle) -> Vec<ServerConfig> {
    vec![]
}

/// Insert or replace a server entry (by id).
pub fn save_server(_app: &tauri::AppHandle, _server: &ServerConfig) -> Result<(), String> {
    Err("servers_store: not implemented".into())
}

/// Remove a server entry and its keychain secrets. Returns true if removed.
pub fn remove_server(_app: &tauri::AppHandle, _id: &str) -> Result<bool, String> {
    Err("servers_store: not implemented".into())
}

/// Fetch the SSH secret (password, or key file path) for a server.
pub fn get_ssh_secret(_app: &tauri::AppHandle, _server_id: &str) -> Result<String, String> {
    Err("servers_store: not implemented".into())
}

/// Store the SSH secret for a server.
pub fn set_ssh_secret(
    _app: &tauri::AppHandle,
    _server_id: &str,
    _secret: &str,
) -> Result<(), String> {
    Err("servers_store: not implemented".into())
}

/// Store the frps auth token generated during deployment.
pub fn set_frps_token(
    _app: &tauri::AppHandle,
    _server_id: &str,
    _token: &str,
) -> Result<(), String> {
    Err("servers_store: not implemented".into())
}

/// Fetch the frps auth token for building frpc configs.
pub fn get_frps_token(_app: &tauri::AppHandle, _server_id: &str) -> Result<String, String> {
    Err("servers_store: not implemented".into())
}
