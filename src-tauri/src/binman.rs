// Binary manager — locates (and later downloads) the tunnel engine binaries.
// R1 (engine) only calls `resolve`; R2 (system integration) owns the full
// download implementation. Keep the public signatures below stable.

use std::path::PathBuf;

use tauri::Manager;

use crate::models::{Backend, BinaryInfo, BinaryStatus};

/// Filename of the engine binary for `backend` on the current platform.
pub fn binary_file_name(backend: Backend) -> &'static str {
    match backend {
        Backend::Cloudflare => {
            if cfg!(target_os = "windows") {
                "cloudflared.exe"
            } else {
                "cloudflared"
            }
        }
        Backend::Bore => {
            if cfg!(target_os = "windows") {
                "bore.exe"
            } else {
                "bore"
            }
        }
    }
}

/// Directory where Pier stores engine binaries:
/// `<app_data_dir>/bin`.
pub fn bin_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?
        .join("bin");
    Ok(dir)
}

/// Resolve the binary path for `backend`.
///
/// Stub implementation: looks in `<app_data_dir>/bin` first, then the system
/// PATH. The download/install flow is implemented in the system-integration
/// milestone and must keep this signature.
pub fn resolve(app: &tauri::AppHandle, backend: Backend) -> Result<PathBuf, String> {
    let name = binary_file_name(backend);
    if let Ok(dir) = bin_dir(app) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    // Fall back to the system PATH.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(format!(
        "engine binary for {backend:?} not found (looked in app data dir and PATH)"
    ))
}

/// Probe the version of the binary at `path` (best-effort).
fn probe_version(path: &PathBuf) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next()?.trim().to_string();
    if first.is_empty() {
        None
    } else {
        Some(first)
    }
}

pub fn read_status(app: &tauri::AppHandle) -> Result<BinaryStatus, String> {
    let info = |backend: Backend| {
        match resolve(app, backend) {
            Ok(path) => BinaryInfo {
                backend,
                version: probe_version(&path),
                path: Some(path.to_string_lossy().to_string()),
                installed: true,
            },
            Err(_) => BinaryInfo {
                backend,
                version: None,
                path: None,
                installed: false,
            },
        }
    };
    Ok(BinaryStatus {
        cloudflare: info(Backend::Cloudflare),
        bore: info(Backend::Bore),
    })
}

/// Download & install the engine binary for `backend` into `<app_data_dir>/bin`.
/// Implemented by the system-integration milestone; stub returns an error.
pub fn install(app: &tauri::AppHandle, backend: Backend) -> Result<BinaryInfo, String> {
    let _ = app;
    Err(format!(
        "download of {backend:?} engine is not implemented yet"
    ))
}
