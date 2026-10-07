// Tunnel domain models — the single source of truth for the
// Rust <-> frontend contract. Keep in sync with src/types/tunnel.ts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    Http,
    Tcp,
}

/// Access control attached to a tunnel. Traffic reaches the local service
/// only through Pier's local forwarder, which enforces this.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelAuth {
    /// Currently only `basic` (HTTP Basic Auth), applied to HTTP tunnels.
    pub kind: String,
    /// Basic-auth username. The password lives in the OS keychain under
    /// `tunnel-auth-{id}` and is never serialized into config files.
    pub username: String,
}

impl TunnelAuth {
    pub const BASIC: &str = "basic";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Cloudflare,
    Bore,
    Frp,
    /// Cloudflare Named Tunnel — a user-owned fixed hostname
    /// (`https://sub.domain`) provisioned via the Cloudflare API.
    /// The explicit rename keeps the camelCase wire format the frontend
    /// matches on; the alias keeps pre-fix tunnels.json data readable.
    #[serde(rename = "cloudflareNamed", alias = "cloudflarenamed")]
    CloudflareNamed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelStatus {
    Stopped,
    Starting,
    Running,
    Reconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConfig {
    pub id: String,
    pub name: String,
    pub tunnel_type: TunnelType,
    pub backend: Backend,
    #[serde(default = "default_local_host")]
    pub local_host: String,
    pub local_port: u16,
    #[serde(default)]
    pub auto_start: bool,
    pub created_at: String,
    /// Frp tunnels only: which Pier-managed server this tunnel runs through.
    #[serde(default)]
    pub server_id: Option<String>,
    /// Frp HTTP tunnels only: subdomain under the server's `subdomainHost`.
    #[serde(default)]
    pub subdomain: Option<String>,
    /// Frp TCP tunnels only: public port allocated on the server.
    #[serde(default)]
    pub remote_port: Option<u16>,
    /// HTTP Basic Auth for this tunnel; password in the OS keychain.
    #[serde(default)]
    pub auth: Option<TunnelAuth>,
    /// CloudflareNamed only: the remote tunnel object id (UUID).
    #[serde(default)]
    pub cf_tunnel_id: Option<String>,
    /// CloudflareNamed only: full fixed hostname (`mac.example.com`).
    /// `public_url` is `https://{cf_hostname}` and never changes.
    #[serde(default)]
    pub cf_hostname: Option<String>,
    /// CloudflareNamed only: Cloudflare account id — needed to update the
    /// remote ingress (service port follows the local forwarder).
    #[serde(default)]
    pub cf_account_id: Option<String>,
    /// Only allow connections from these IPs/CIDRs; empty = allow all.
    #[serde(default)]
    pub ip_allowlist: Vec<String>,
}

fn default_local_host() -> String {
    "127.0.0.1".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelState {
    pub id: String,
    pub status: TunnelStatus,
    pub public_url: Option<String>,
    pub error: Option<String>,
    pub started_at: Option<String>,
    #[serde(default)]
    pub bytes_in: u64,
    #[serde(default)]
    pub bytes_out: u64,
}

impl TunnelState {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            status: TunnelStatus::Stopped,
            public_url: None,
            error: None,
            started_at: None,
            bytes_in: 0,
            bytes_out: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryInfo {
    pub backend: Backend,
    pub version: Option<String>,
    pub path: Option<String>,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryStatus {
    pub cloudflare: BinaryInfo,
    pub bore: BinaryInfo,
}

// ---------------------------------------------------------------------------
// Servers (M2: self-hosted frps on the user's VPS)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    /// Password, stored in the OS keychain — never serialized into files.
    Password,
    /// Path to a private key file on this machine.
    KeyPath,
}

/// A VPS the user manages through Pier. The secret (password) lives in the
/// OS keychain keyed by `id`; it is never stored in `servers.json` nor sent
/// to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_kind: AuthKind,
    #[serde(default = "default_frps_bind_port")]
    pub frps_bind_port: u16,
    #[serde(default = "default_vhost_http_port")]
    pub frps_vhost_http_port: u16,
    #[serde(default = "default_vhost_https_port")]
    pub frps_vhost_https_port: u16,
    #[serde(default = "default_dashboard_port")]
    pub frps_dashboard_port: u16,
    /// Domain pointed at this server with a wildcard A record, e.g.
    /// `mydomain.com` so tunnels can claim `foo.mydomain.com`.
    #[serde(default)]
    pub subdomain_host: Option<String>,
    #[serde(default)]
    pub deployed: bool,
    #[serde(default)]
    pub frps_version: Option<String>,
    pub created_at: String,
}

fn default_frps_bind_port() -> u16 {
    7000
}
fn default_vhost_http_port() -> u16 {
    8080
}
fn default_vhost_https_port() -> u16 {
    8443
}
fn default_dashboard_port() -> u16 {
    7500
}

/// Input for creating a server; `secret` is the password or key path and is
/// moved into the keychain on save.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_kind: AuthKind,
    pub secret: String,
    #[serde(default)]
    pub frps_bind_port: Option<u16>,
    #[serde(default)]
    pub frps_vhost_http_port: Option<u16>,
    #[serde(default)]
    pub frps_vhost_https_port: Option<u16>,
    #[serde(default)]
    pub frps_dashboard_port: Option<u16>,
    #[serde(default)]
    pub subdomain_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub server_id: String,
    pub reachable: bool,
    pub frps_running: bool,
    pub frps_version: Option<String>,
    pub detail: Option<String>,
}

// ---------------------------------------------------------------------------
// Deployment progress (event "deploy://progress")
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StepStatus {
    Running,
    Ok,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployProgress {
    pub server_id: String,
    pub step: String,
    pub status: StepStatus,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployResult {
    pub server_id: String,
    pub ok: bool,
    pub error: Option<String>,
    /// Random token generated for frps; also stored in the keychain.
    pub token: Option<String>,
}

// ---------------------------------------------------------------------------
// Diagnosis (one-click human-readable failure explanation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosisLevel {
    Info,
    Warn,
    Error,
}

/// One-click diagnosis result. `code` is a stable machine-readable slug;
/// the frontend renders title/suggestions from its i18n dictionary
/// (`diagnosis.<code>.title` / `.detail` / `.suggestions`). `detail`
/// carries the raw evidence (a relevant log line) for context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub tunnel_id: String,
    pub code: String,
    pub level: DiagnosisLevel,
    pub detail: String,
}

// ---------------------------------------------------------------------------
// Traffic stats (event "tunnel://stats", emitted by the local forwarder,
// throttled to at most one per second per tunnel)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelStats {
    pub tunnel_id: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
    /// Currently open proxied connections.
    pub conn_active: u32,
}

// ---------------------------------------------------------------------------
// Cloudflare Named Tunnel (v0.2.0) — API-driven fixed hostnames
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfAccount {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfZone {
    pub id: String,
    pub name: String,
    pub account_id: String,
}

/// Everything needed to provision a fixed-hostname tunnel in one shot.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfProvisionInput {
    /// Cloudflare API token (Account Cloudflare Tunnel:Edit + Zone DNS:Edit + Zone:Read).
    pub token: String,
    pub zone_id: String,
    /// Single label, e.g. `mac` for `mac.example.com`.
    pub subdomain: String,
    #[serde(default = "default_local_host")]
    pub local_host: String,
    pub local_port: u16,
    #[serde(default)]
    pub auto_start: bool,
}
