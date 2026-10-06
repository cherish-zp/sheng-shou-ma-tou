// R3 IMPLEMENTS: SSH execution layer (russh) — connect, run commands with
// sudo handling, SFTP upload. Used by frp_deploy.
//
// russh is pure Rust, so this layer builds unchanged on macOS and Windows;
// the remote side is always the user's Linux VPS.
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use russh::client::{self, Handle};
use russh::keys::{PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use russh_sftp::client::SftpSession;
use tokio::io::AsyncWriteExt;

use crate::models::{AuthKind, ServerConfig};

/// Bound for establishing the TCP + SSH handshake + authentication.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Default bound for a single exec'ed command (deploy steps that need longer
/// pass their own, e.g. the frp tarball download).
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// SFTP upload chunk: well below the typical 256 KiB SFTP packet budget.
const SFTP_CHUNK: usize = 32 * 1024;

/// Host-key verification policy: accept-on-first-use *without persistence*.
///
/// Every connection trusts whatever host key the server presents. This is the
/// classic TOFU trade-off without the "O": the first connection is protected
/// by the network path only, and there is currently no known_hosts store to
/// pin the key, so an active MITM on later connections also goes undetected.
///
/// Documented risk: for a deployment tool that sends a password over the
/// authenticated channel this is weaker than OpenSSH's known_hosts, but it
/// matches what most one-click deploy scripts do and keeps the first-run
/// experience free of interactive prompts.
///
/// Follow-up (M3+): persist `server_public_key` in the app data dir keyed by
/// `host:port` (russh exposes `russh::keys::check_known_hosts_path`), and
/// fail with a "host key changed" error on mismatch instead of accepting.
struct TrustAnyHostKey;

impl client::Handler for TrustAnyHostKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        // See the struct docs: TOFU-lite, known_hosts persistence is planned.
        Ok(true)
    }
}

pub struct SshSession {
    handle: Handle<TrustAnyHostKey>,
    username: String,
    /// Set only for password authentication; used to feed `sudo -S`.
    password: Option<String>,
    /// Cached result of `sudo -n true` for key-auth sessions (None = unknown).
    passwordless_sudo: Option<bool>,
}

/// Open an SSH connection to `server`, authenticating from the OS keychain.
///
/// `AuthKind::Password` uses the stored password; `AuthKind::KeyPath` treats
/// the stored secret as a path to an unencrypted private key on this machine.
pub async fn connect(server: &ServerConfig) -> Result<SshSession, String> {
    let secret = crate::servers_store::fetch_ssh_secret(&server.id)?;

    let config = Arc::new(client::Config {
        // Drop the session if the server goes quiet; keeps half-open
        // connections from hanging deploy steps until the command timeout.
        inactivity_timeout: Some(Duration::from_secs(300)),
        keepalive_interval: Some(Duration::from_secs(15)),
        ..Default::default()
    });

    let mut handle = tokio::time::timeout(
        CONNECT_TIMEOUT,
        client::connect(config, (server.host.as_str(), server.port), TrustAnyHostKey),
    )
    .await
    .map_err(|_| format!("SSH 连接超时（15s）：{}", server.host))?
    .map_err(|e| format!("SSH 连接失败（{}:{}）：{e}", server.host, server.port))?;

    let password = match server.auth_kind {
        AuthKind::Password => {
            let auth = handle
                .authenticate_password(&server.username, &secret)
                .await
                .map_err(|e| format!("SSH 密码认证出错：{e}"))?;
            if !auth.success() {
                return Err(format!(
                    "SSH 密码认证被拒绝（用户 {}@{}）。请在服务器设置中更新密码。",
                    server.username, server.host
                ));
            }
            Some(secret)
        }
        AuthKind::KeyPath => {
            let key = russh::keys::load_secret_key(&secret, None).map_err(|e| {
                format!("无法读取私钥文件 {secret}（仅支持无口令密钥）：{e}")
            })?;
            let auth = handle
                .authenticate_publickey(
                    &server.username,
                    PrivateKeyWithHashAlg::new(Arc::new(key), None),
                )
                .await
                .map_err(|e| format!("SSH 密钥认证出错：{e}"))?;
            if !auth.success() {
                return Err(format!(
                    "SSH 密钥认证被拒绝（用户 {}@{}）。请确认公钥已加入服务器的 authorized_keys。",
                    server.username, server.host
                ));
            }
            None
        }
    };

    Ok(SshSession {
        handle,
        username: server.username.clone(),
        password,
        passwordless_sudo: None,
    })
}

/// Single-quote a string for POSIX `sh` (`'` -> `'\''`).
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl SshSession {
    /// Run a command; returns (exit_code, stdout, stderr) with the streams
    /// kept separate (port/JSON parsing depends on that). Exit code is -1
    /// when the server closes the channel without reporting a status.
    pub async fn run(&mut self, cmd: &str) -> Result<(i32, String, String), String> {
        self.exec_with_stdin(cmd, None, COMMAND_TIMEOUT).await
    }

    /// Run a command with an explicit timeout (deploy download steps need
    /// far more than the default 30s).
    pub async fn run_with_timeout(
        &mut self,
        cmd: &str,
        timeout: Duration,
    ) -> Result<(i32, String, String), String> {
        self.exec_with_stdin(cmd, None, timeout).await
    }

    /// Run with best-effort sudo when the login user is not root.
    ///
    /// * root login: the command runs as-is.
    /// * password login: `sudo -S -p '' sh -c '<cmd>'` with the password fed
    ///   through stdin (never through argv, where other users could see it).
    /// * key login: passwordless sudo is probed once; without it we cannot
    ///   supply a sudo password, so the caller gets a clear error.
    pub async fn run_sudo(&mut self, cmd: &str) -> Result<(i32, String, String), String> {
        if self.username == "root" {
            return self.run(cmd).await;
        }
        let quoted = shell_quote(cmd);
        if let Some(password) = self.password.clone() {
            let wrapper = format!("sudo -S -p '' sh -c {quoted}");
            self.exec_with_stdin(&wrapper, Some(&password), COMMAND_TIMEOUT)
                .await
        } else {
            if self.passwordless_sudo.is_none() {
                let (code, _, _) = self.run("sudo -n true").await?;
                self.passwordless_sudo = Some(code == 0);
            }
            if !self.passwordless_sudo.unwrap_or(false) {
                return Err(
                    "需要免密 sudo 或 root 登录：密钥认证方式无法自动输入 sudo 密码。请为该用户\
                     配置 NOPASSWD sudo，或改用 root/密码方式重试。"
                        .into(),
                );
            }
            self.run(&format!("sudo -n sh -c {quoted}")).await
        }
    }

    /// Shared exec core: open a session channel, run `cmd`, optionally push
    /// `stdin_data` + EOF (for `sudo -S`), and collect the streams.
    async fn exec_with_stdin(
        &mut self,
        cmd: &str,
        stdin_data: Option<&str>,
        timeout: Duration,
    ) -> Result<(i32, String, String), String> {
        let collect = async {
            let mut channel = self
                .handle
                .channel_open_session()
                .await
                .map_err(|e| format!("打开 SSH 通道失败：{e}"))?;
            channel
                .exec(false, cmd)
                .await
                .map_err(|e| format!("发送命令失败：{e}"))?;
            if let Some(data) = stdin_data {
                channel
                    .data_bytes(format!("{data}\n"))
                    .await
                    .map_err(|e| format!("写入 sudo 密码失败：{e}"))?;
                let _ = channel.eof().await;
            }

            let mut stdout: Vec<u8> = Vec::new();
            let mut stderr: Vec<u8> = Vec::new();
            let mut exit_code: Option<i32> = None;
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                    ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
                    ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status as i32),
                    ChannelMsg::Eof | ChannelMsg::ExitSignal { .. } => {}
                    ChannelMsg::Close => break,
                    _ => {}
                }
            }
            Ok((
                exit_code.unwrap_or(-1),
                String::from_utf8_lossy(&stdout).into_owned(),
                String::from_utf8_lossy(&stderr).into_owned(),
            ))
        };

        tokio::time::timeout(timeout, collect)
            .await
            .map_err(|_| format!("命令执行超时（{}s）：{cmd}", timeout.as_secs()))?
    }

    /// Upload a local file to `remote_path` via SFTP.
    pub async fn upload_file(&mut self, local: &Path, remote_path: &str) -> Result<(), String> {
        let data = tokio::fs::read(local)
            .await
            .map_err(|e| format!("读取本地文件 {} 失败：{e}", local.display()))?;

        let channel = self
            .handle
            .channel_open_session()
            .await
            .map_err(|e| format!("打开 SFTP 通道失败：{e}"))?;
        let sftp = tokio::time::timeout(CONNECT_TIMEOUT, SftpSession::new(channel.into_stream()))
            .await
            .map_err(|_| "SFTP 子系统初始化超时".to_string())?
            .map_err(|e| format!("SFTP 子系统初始化失败：{e}"))?;

        let mut file = sftp
            .create(remote_path)
            .await
            .map_err(|e| format!("SFTP 创建远端文件 {remote_path} 失败：{e}"))?;
        for chunk in data.chunks(SFTP_CHUNK) {
            file.write_all(chunk)
                .await
                .map_err(|e| format!("SFTP 上传 {remote_path} 失败：{e}"))?;
        }
        file.shutdown()
            .await
            .map_err(|e| format!("SFTP 完成 {remote_path} 失败：{e}"))?;
        Ok(())
    }

    pub async fn close(&mut self) {
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "pier: session closed", "en")
            .await;
    }
}
