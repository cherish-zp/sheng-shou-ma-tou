// R5 IMPLEMENTS: local forwarder + traffic stats + access control.
//
// Every active tunnel points its engine binary at a local forwarder
// listener (127.0.0.1, random port) instead of directly at the user's
// service. The forwarder:
//   1. counts bytes in/out per tunnel (throttled "tunnel://stats" events),
//   2. enforces Basic Auth for HTTP-type tunnels (401 on bad credentials,
//      password from the keychain via servers_store::get_tunnel_auth_password),
//   3. enforces the tunnel's IP allowlist (empty = allow all),
//   4. transparently proxies everything else byte-for-byte.
//
// Public API below is frozen — commands.rs and engine.rs code against it.
use std::net::IpAddr;
use std::sync::Arc;

use tauri::AppHandle;

use crate::models::TunnelConfig;

/// A running forwarder bound to 127.0.0.1 on an ephemeral port.
#[allow(dead_code)]
pub struct Forwarder;

impl Forwarder {
    /// Start a forwarder for `cfg` and emit throttled stats events on `app`.
    /// Returns the local port the tunnel binary must connect to.
    #[allow(dead_code)]
    pub async fn start(
        _app: AppHandle,
        _cfg: TunnelConfig,
        _auth_password: Option<String>,
    ) -> Result<(Arc<Forwarder>, u16), String> {
        Err("forwarder: not implemented".into())
    }

    /// Stop the forwarder and all proxied connections.
    #[allow(dead_code)]
    pub async fn stop(&self) {}
}

/// Parse an IP or CIDR (e.g. `10.0.0.0/8`) allowlist entry for `ip`.
#[allow(dead_code)]
pub fn ip_allowed(_ip: IpAddr, _allowlist: &[String]) -> bool {
    true
}

/// Check a Basic Auth header value against user + password.
#[allow(dead_code)]
pub fn basic_auth_ok(_header_value: &str, _username: &str, _password: &str) -> bool {
    false
}
