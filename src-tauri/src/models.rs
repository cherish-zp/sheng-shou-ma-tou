// Tunnel domain models — the single source of truth for the
// Rust <-> frontend contract. Keep in sync with src/types/tunnel.ts.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TunnelType {
    Http,
    Tcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Cloudflare,
    Bore,
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
