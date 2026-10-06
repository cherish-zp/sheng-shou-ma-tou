// R3 IMPLEMENTS: SSH execution layer (russh) — connect, run commands with
// sudo handling, SFTP upload. Used by frp_deploy.
use std::path::Path;

use crate::models::ServerConfig;

pub struct SshSession;

/// Open an SSH connection to `server`, authenticating from the OS keychain.
pub async fn connect(_server: &ServerConfig) -> Result<SshSession, String> {
    Err("ssh: not implemented".into())
}

impl SshSession {
    /// Run a command; returns (exit_code, stdout, stderr combined).
    pub async fn run(&mut self, _cmd: &str) -> Result<(i32, String, String), String> {
        Err("ssh: not implemented".into())
    }

    /// Run with best-effort sudo when the login user is not root.
    pub async fn run_sudo(&mut self, _cmd: &str) -> Result<(i32, String, String), String> {
        Err("ssh: not implemented".into())
    }

    /// Upload a local file to `remote_path` via SFTP.
    pub async fn upload_file(&mut self, _local: &Path, _remote_path: &str) -> Result<(), String> {
        Err("ssh: not implemented".into())
    }

    pub async fn close(&mut self) {}
}
