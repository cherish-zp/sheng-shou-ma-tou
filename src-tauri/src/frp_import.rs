// frpc config (toml/ini) import — parse an existing frpc config file text
// into Pier tunnel configs (preview; caller persists them via create_tunnel).
//
// Supported input shapes:
// * modern TOML with a `[[proxies]]` array (frp >= 0.52), camelCase keys
// * legacy TOML: a `[common]` table plus one table per proxy (snake_case keys)
// * legacy INI: `[common]` section plus one `[section]` per proxy
//   (snake_case keys; `type` defaults to tcp, matching frp's legacy default)
//
// Mapping rules:
// * type tcp    -> TunnelType::Tcp, keeps remote_port
// * type http   -> TunnelType::Http, subdomain taken from the `subdomain`
//   field, else from the first label of the first customDomain
// * any other type (udp/stcp/xtcp/sudp/...) is SKIPPED: Pier cannot map it
//   yet. A file without a single mappable proxy yields an Err so the UI can
//   tell the user why nothing was imported.
// * server_id: `Some(sid)` attaches the tunnel to that Pier server; `None`
//   leaves it unbound (Backend::Frp, user picks a server later).

use crate::models::{Backend, TunnelConfig, TunnelType};

/// Parse frpc config text (TOML preferred, INI legacy supported).
/// Returns only mappable proxies (tcp/http types); unsupported entries are
/// skipped. `server_id` is attached when provided (import-through-server),
/// else tunnels are created as `Backend::Frp` without a server (user picks
/// one later).
pub fn parse_frpc_config(text: &str, server_id: Option<&str>) -> Result<Vec<TunnelConfig>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("配置内容为空 (the pasted config is empty)".into());
    }
    // Valid TOML wins; anything that fails TOML parsing is treated as legacy
    // INI (INI values are usually unquoted, which is invalid TOML, so the two
    // shapes are mutually exclusive in practice).
    let tunnels = match toml::from_str::<toml::Value>(text) {
        Ok(value) => parse_toml(&value, server_id)?,
        Err(_) => parse_ini(text, server_id)?,
    };
    if tunnels.is_empty() {
        return Err(
            "未找到可导入的 tcp/http 代理 (no mappable tcp/http proxies found; \
             udp/stcp/xtcp proxies are not supported yet)"
                .into(),
        );
    }
    Ok(tunnels)
}

// ---------------------------------------------------------------------------
// TOML parsing
// ---------------------------------------------------------------------------

fn parse_toml(value: &toml::Value, server_id: Option<&str>) -> Result<Vec<TunnelConfig>, String> {
    // Modern layout: `[[proxies]]` array of proxy tables.
    if let Some(proxies) = value.get("proxies").and_then(|p| p.as_array()) {
        return Ok(proxies
            .iter()
            .filter_map(|item| from_modern_toml(item, server_id))
            .collect());
    }
    // Legacy TOML layout: `[common]` plus one table per proxy. Section names
    // double as proxy names (mirrors the legacy INI behavior).
    if let Some(root) = value.as_table() {
        if root.contains_key("common") {
            return Ok(root
                .iter()
                .filter(|(key, _)| {
                    let key = key.as_str();
                    key != "common" && key != "visitors"
                })
                .filter_map(|(name, item)| {
                    from_legacy_toml(name, item.as_table()?, server_id)
                })
                .collect());
        }
    }
    Err("TOML 中未找到 [[proxies]] 或 [common] (the TOML has no [[proxies]] array and no [common] table)".into())
}

fn from_modern_toml(item: &toml::Value, server_id: Option<&str>) -> Option<TunnelConfig> {
    let table = item.as_table()?;
    let ptype = table.get("type")?.as_str()?.to_string();
    let raw = RawProxy {
        name: table.get("name").and_then(|v| v.as_str()).map(Into::into),
        ptype,
        local_ip: table
            .get("localIP")
            .and_then(|v| v.as_str())
            .map(Into::into),
        local_port: table.get("localPort").and_then(as_u16),
        remote_port: table.get("remotePort").and_then(as_u16),
        subdomain: table.get("subdomain").and_then(|v| v.as_str()).map(Into::into),
        custom_domains: string_list(table.get("customDomains")),
    };
    into_tunnel_config(raw, server_id)
}

fn from_legacy_toml(
    name: &str,
    table: &toml::Table,
    server_id: Option<&str>,
) -> Option<TunnelConfig> {
    let raw = RawProxy {
        name: Some(name.to_string()),
        // Legacy configs may omit `type`; frp's legacy default is tcp.
        ptype: table
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp")
            .to_string(),
        local_ip: table.get("local_ip").and_then(|v| v.as_str()).map(Into::into),
        local_port: table.get("local_port").and_then(as_u16),
        remote_port: table.get("remote_port").and_then(as_u16),
        subdomain: table.get("subdomain").and_then(|v| v.as_str()).map(Into::into),
        custom_domains: string_list(table.get("custom_domains")),
    };
    into_tunnel_config(raw, server_id)
}

/// Collect a TOML value into a list of strings: accepts a real array
/// (`customDomains = ["a.com"]`) or a single comma-separated string
/// (`custom_domains = "a.com,b.com"`); `None` yields an empty list.
fn string_list(value: Option<&toml::Value>) -> Vec<String> {
    match value {
        Some(toml::Value::Array(items)) => items
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(toml::Value::String(s)) => s
            .split(',')
            .map(|part| part.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn as_u16(value: &toml::Value) -> Option<u16> {
    value.as_integer().and_then(|i| u16::try_from(i).ok())
}

// ---------------------------------------------------------------------------
// Legacy INI parsing
// ---------------------------------------------------------------------------

fn parse_ini(text: &str, server_id: Option<&str>) -> Result<Vec<TunnelConfig>, String> {
    // Sections in document order: (name, key/value pairs).
    let mut sections: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            match rest.strip_suffix(']') {
                // New section (frp legacy sections double as proxy names).
                Some(name) => sections.push((name.trim().to_string(), Vec::new())),
                None => continue, // malformed header — ignore the line
            }
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let Some(section) = sections.last_mut() else {
                continue; // key before any [section] header — ignore
            };
            section
                .1
                .push((key.trim().to_ascii_lowercase(), strip_quotes(value)));
        }
    }
    if sections.is_empty() {
        return Err(
            "无法识别的配置格式 (the text is neither valid TOML nor a legacy INI config)".into(),
        );
    }
    Ok(sections
        .iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case("common"))
        .filter_map(|(name, pairs)| {
            let get = |key: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
            };
            let raw = RawProxy {
                name: Some(name.clone()),
                // Legacy default when `type` is omitted.
                ptype: get("type").unwrap_or_else(|| "tcp".to_string()),
                local_ip: get("local_ip"),
                local_port: get("local_port").and_then(|v| v.parse().ok()),
                remote_port: get("remote_port").and_then(|v| v.parse().ok()),
                subdomain: get("subdomain"),
                custom_domains: get("custom_domains")
                    .map(|v| {
                        v.split(',')
                            .map(|part| part.trim().to_string())
                            .filter(|part| !part.is_empty())
                            .collect()
                    })
                    .unwrap_or_default(),
            };
            into_tunnel_config(raw, server_id)
        })
        .collect())
}

/// Strip one pair of surrounding quotes from an INI value.
fn strip_quotes(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

// ---------------------------------------------------------------------------
// Shared mapping
// ---------------------------------------------------------------------------

/// A proxy in whatever source syntax, normalized before mapping.
struct RawProxy {
    name: Option<String>,
    ptype: String,
    local_ip: Option<String>,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    subdomain: Option<String>,
    custom_domains: Vec<String>,
}

/// Map a normalized proxy to a Pier tunnel config. Returns `None` for
/// unsupported types (udp/stcp/xtcp/...) or entries without a local port —
/// both are skipped silently (the import result cannot carry skip info; a
/// file with no mappable proxy at all errors out instead).
fn into_tunnel_config(proxy: RawProxy, server_id: Option<&str>) -> Option<TunnelConfig> {
    let ptype = proxy.ptype.trim().to_ascii_lowercase();
    let local_port = proxy.local_port?;
    let (tunnel_type, subdomain, remote_port) = match ptype.as_str() {
        "tcp" => (TunnelType::Tcp, None, proxy.remote_port),
        "http" => {
            let subdomain = proxy
                .subdomain
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    proxy
                        .custom_domains
                        .first()
                        .and_then(|domain| first_domain_label(domain))
                });
            (TunnelType::Http, subdomain, None)
        }
        // udp / stcp / xtcp / sudp / tcpmux / ... : not supported by Pier.
        _ => return None,
    };
    let name = proxy
        .name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{ptype}-{local_port}"));
    let local_host = proxy
        .local_ip
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    Some(TunnelConfig {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        tunnel_type,
        backend: Backend::Frp,
        local_host,
        local_port,
        auto_start: false,
        created_at: chrono::Utc::now().to_rfc3339(),
        server_id: server_id.map(str::to_string),
        subdomain,
        remote_port,
    })
}

/// `web1.example.com` -> `web1`; a bare label stays itself.
fn first_domain_label(domain: &str) -> Option<String> {
    let domain = domain.trim();
    if domain.is_empty() {
        return None;
    }
    let label = domain.split('.').next().unwrap_or_default().trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TunnelType;

    #[test]
    fn modern_toml_tcp_http_udp_mix() {
        let text = r#"
serverAddr = "1.2.3.4"
serverPort = 7000
auth.token = "tok"

[[proxies]]
name = "ssh"
type = "tcp"
localIP = "192.168.1.10"
localPort = 22
remotePort = 6022

[[proxies]]
name = "web"
type = "http"
localPort = 8080
subdomain = "myapp"

[[proxies]]
name = "dns"
type = "udp"
localPort = 53
remotePort = 6053
"#;
        let tunnels = parse_frpc_config(text, Some("srv1")).expect("parse modern toml");
        // The udp proxy is skipped.
        assert_eq!(tunnels.len(), 2);

        let ssh = &tunnels[0];
        assert_eq!(ssh.name, "ssh");
        assert_eq!(ssh.tunnel_type, TunnelType::Tcp);
        assert_eq!(ssh.backend, Backend::Frp);
        assert_eq!(ssh.server_id.as_deref(), Some("srv1"));
        assert_eq!(ssh.local_host, "192.168.1.10");
        assert_eq!(ssh.local_port, 22);
        assert_eq!(ssh.remote_port, Some(6022));
        assert_eq!(ssh.subdomain, None);
        assert!(!ssh.auto_start);
        assert!(!ssh.id.is_empty());
        assert!(!ssh.created_at.is_empty());

        let web = &tunnels[1];
        assert_eq!(web.name, "web");
        assert_eq!(web.tunnel_type, TunnelType::Http);
        assert_eq!(web.subdomain.as_deref(), Some("myapp"));
        assert_eq!(web.remote_port, None);
        // Defaults applied.
        assert_eq!(web.local_host, "127.0.0.1");
    }

    #[test]
    fn modern_toml_http_subdomain_from_first_custom_domain() {
        let text = r#"
[[proxies]]
name = "web"
type = "http"
localPort = 8080
customDomains = ["web1.example.com", "web2.example.com"]
"#;
        let tunnels = parse_frpc_config(text, None).expect("parse");
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].tunnel_type, TunnelType::Http);
        assert_eq!(tunnels[0].subdomain.as_deref(), Some("web1"));
        // No server -> unbound Frp tunnel.
        assert_eq!(tunnels[0].backend, Backend::Frp);
        assert_eq!(tunnels[0].server_id, None);
    }

    #[test]
    fn legacy_ini_common_and_sections() {
        let text = "
[common]
server_addr = 1.2.3.4
server_port = 7000
token = mytoken

[ssh]
type = tcp
local_ip = 127.0.0.1
local_port = 22
remote_port = 6022

[web]
type = http
local_port = 8080
custom_domains = web1.example.com,web2.example.com
";
        let tunnels = parse_frpc_config(text, Some("srv2")).expect("parse ini");
        assert_eq!(tunnels.len(), 2);

        let ssh = &tunnels[0];
        assert_eq!(ssh.name, "ssh");
        assert_eq!(ssh.tunnel_type, TunnelType::Tcp);
        assert_eq!(ssh.remote_port, Some(6022));
        assert_eq!(ssh.local_port, 22);
        assert_eq!(ssh.server_id.as_deref(), Some("srv2"));

        let web = &tunnels[1];
        assert_eq!(web.name, "web");
        assert_eq!(web.tunnel_type, TunnelType::Http);
        assert_eq!(web.subdomain.as_deref(), Some("web1"));
    }

    #[test]
    fn legacy_ini_type_defaults_to_tcp() {
        let text = "
[common]
server_addr = 1.2.3.4

[rdp]
local_port = 3389
remote_port = 6389
";
        let tunnels = parse_frpc_config(text, None).expect("parse ini");
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].tunnel_type, TunnelType::Tcp);
        assert_eq!(tunnels[0].local_port, 3389);
        assert_eq!(tunnels[0].remote_port, Some(6389));
    }

    #[test]
    fn legacy_toml_common_style() {
        let text = r#"
[common]
server_addr = "1.2.3.4"
server_port = 7000
token = "mytoken"

[ssh]
type = "tcp"
local_port = 22
remote_port = 6022

[web]
type = "http"
local_port = 8080
subdomain = "app"
"#;
        let tunnels = parse_frpc_config(text, Some("srv3")).expect("parse legacy toml");
        assert_eq!(tunnels.len(), 2);
        assert_eq!(tunnels[0].name, "ssh");
        assert_eq!(tunnels[0].remote_port, Some(6022));
        assert_eq!(tunnels[1].subdomain.as_deref(), Some("app"));
    }

    #[test]
    fn invalid_inputs_error_out() {
        assert!(parse_frpc_config("", None).is_err());
        assert!(parse_frpc_config("   \n  ", None).is_err());
        // Neither TOML nor INI.
        assert!(parse_frpc_config("this is just prose without any structure", None).is_err());
        // TOML without proxies/common.
        let text = r#"
serverAddr = "1.2.3.4"
serverPort = 7000
"#;
        assert!(parse_frpc_config(text, None).is_err());
        // INI with only [common].
        let text = "\n[common]\nserver_addr = 1.2.3.4\n";
        assert!(parse_frpc_config(text, None).is_err());
    }

    #[test]
    fn unsupported_proxy_types_only_error_out() {
        let text = r#"
[[proxies]]
name = "dns"
type = "udp"
localPort = 53
remotePort = 6053

[[proxies]]
name = "secret"
type = "stcp"
localPort = 22
sk = "abc"
"#;
        let err = parse_frpc_config(text, None).expect_err("all proxies unsupported");
        assert!(err.contains("no mappable"));
    }

    #[test]
    fn entries_without_local_port_are_skipped() {
        let text = r#"
[[proxies]]
name = "broken"
type = "tcp"
remotePort = 6022

[[proxies]]
name = "ok"
type = "tcp"
localPort = 22
remotePort = 6023
"#;
        let tunnels = parse_frpc_config(text, None).expect("parse");
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].name, "ok");
    }

    #[test]
    fn missing_name_falls_back_to_type_and_port() {
        let text = r#"
[[proxies]]
type = "tcp"
localPort = 9090
remotePort = 6909
"#;
        let tunnels = parse_frpc_config(text, None).expect("parse");
        assert_eq!(tunnels.len(), 1);
        assert_eq!(tunnels[0].name, "tcp-9090");
    }
}
