// R7 IMPLEMENTS: Cloudflare API client for Named Tunnels.
//
// All functions take the user's API token explicitly (the token also lives
// in the OS keychain under `cf-api-token` — see servers_store conventions).
// Endpoints per docs/FEATURE-cloudflare-named-tunnel.md §3.2.
//
// Public signatures below are frozen — commands.rs and providers.rs /
// engine.rs code against them.
use crate::models::{CfAccount, CfProvisionInput, CfZone, TunnelConfig};

/// Validate the API token; return the accounts it can see.
pub fn verify_token(_token: &str) -> Result<Vec<CfAccount>, String> {
    Err("cloudflare: not implemented".into())
}

/// List zones (hosted domains) visible to the token.
pub fn list_zones(_token: &str) -> Result<Vec<CfZone>, String> {
    Err("cloudflare: not implemented".into())
}

/// Full provisioning in one call:
///   create tunnel (config_src=cloudflare) -> write ingress (hostname +
///   catch-all 404) -> upsert CNAME (`<subdomain>.<zone>` -> `<tunnel-id>
///   .cfargotunnel.com`, proxied). Returns the ready-to-store TunnelConfig
///   (Backend::CloudflareNamed, cf_tunnel_id/cf_hostname set, no token
///   inside — tokens go to the keychain by the caller).
pub fn provision(_input: &CfProvisionInput) -> Result<TunnelConfig, String> {
    Err("cloudflare: not implemented".into())
}

/// Store the tunnel-run token under keychain `cf-tunnel-token-{tunnel_id}`.
pub fn set_tunnel_run_token(_tunnel_id: &str, _tunnel_token: &str) -> Result<(), String> {
    Err("cloudflare: not implemented".into())
}

/// Fetch the tunnel-run token (engine passes it to cloudflared via the
/// TUNNEL_TOKEN env var — never as a CLI argument).
pub fn get_tunnel_run_token(_tunnel_id: &str) -> Result<String, String> {
    Err("cloudflare: not implemented".into())
}

/// Delete the remote tunnel object. `delete_dns` also removes the CNAME
/// record (Cloudflare does not clean it up on tunnel deletion).
pub fn deprovision(
    _token: &str,
    _zone_id: &str,
    _tunnel_id: &str,
    _delete_dns: bool,
) -> Result<(), String> {
    Err("cloudflare: not implemented".into())
}

/// Query remote tunnel health ("inactive" | "degraded" | "healthy" | "down").
pub fn tunnel_status(_token: &str, _tunnel_id: &str) -> Result<String, String> {
    Err("cloudflare: not implemented".into())
}
