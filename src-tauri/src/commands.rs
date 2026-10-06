// R1 IMPLEMENTS: real AppState, init_runtime, and command bodies below.
// These stubs exist so the skeleton compiles; keep all command signatures.

use tauri::AppHandle;

use crate::models::{Backend, BinaryStatus, TunnelConfig, TunnelState};

pub struct AppState;

impl AppState {
    pub fn new() -> Self {
        Self
    }
}

pub fn init_runtime(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[tauri::command]
pub fn list_tunnels() -> Vec<TunnelConfig> {
    vec![]
}

#[tauri::command]
pub fn create_tunnel(config: TunnelConfig) -> Result<TunnelConfig, String> {
    Ok(config)
}

#[tauri::command]
pub fn update_tunnel(config: TunnelConfig) -> Result<TunnelConfig, String> {
    Ok(config)
}

#[tauri::command]
pub fn delete_tunnel(_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub fn start_tunnel(_id: String) -> Result<TunnelState, String> {
    Err("not implemented".into())
}

#[tauri::command]
pub fn stop_tunnel(_id: String) -> Result<TunnelState, String> {
    Err("not implemented".into())
}

#[tauri::command]
pub fn get_state(_id: String) -> Result<TunnelState, String> {
    Err("not implemented".into())
}

#[tauri::command]
pub fn list_states() -> Vec<TunnelState> {
    vec![]
}

#[tauri::command]
pub fn read_binary_status(_app: AppHandle) -> Result<BinaryStatus, String> {
    Err("not implemented".into())
}

#[tauri::command]
pub fn install_binary(_app: AppHandle, _backend: Backend) -> Result<BinaryStatus, String> {
    Err("not implemented".into())
}
