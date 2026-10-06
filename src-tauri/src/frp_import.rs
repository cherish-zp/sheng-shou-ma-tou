// R4 IMPLEMENTS: frpc config (toml/ini) import — parse an existing frpc
// config file text into Pier tunnel configs (preview; caller persists them
// via create_tunnel).
use crate::models::TunnelConfig;

/// Parse frpc config text (TOML preferred, INI legacy supported).
/// Returns only mappable proxies (tcp/http types); unsupported entries are
/// skipped. `server_id` is attached when provided (import-through-server),
/// else tunnels are created as `Backend::Frp` without a server (user picks
/// one later).
pub fn parse_frpc_config(_text: &str, _server_id: Option<&str>) -> Result<Vec<TunnelConfig>, String> {
    Err("frp_import: not implemented".into())
}
