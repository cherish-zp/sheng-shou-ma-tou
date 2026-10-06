// Backend providers — command builders + output parsers for cloudflared/bore.
//
// Parsing rules below are calibrated against real binary output captured on
// macOS arm64 (cloudflared 2026.10.0, bore-cli 0.6.0 and 0.5.1):
//
// cloudflared (ALL logs go to stderr, quick tunnel URL inside ASCII-art bars):
//   2026-10-06T10:48:20Z INF |  https://vessels-arrow-kid-educated.trycloudflare.com   |
//   2026-10-06T10:48:20Z INF |  Your quick Tunnel has been created! Visit it at ...  |
//   2026-10-06T10:48:25Z ERR Failed to dial a quic connection error="..." connIndex=0
//   ^ NOTE: ERR lines are frequently transient — cloudflared retries on its
//     own, so they must NOT be treated as fatal by the engine.
//
// bore (logs go to stdout, WITH ANSI color escapes even when piped):
//   ESC[2m2026-10-06T10:48:08.904579Z ESC[0m ESC[32m INFO ESC[0m ... connected to server remote_port=62457
//   ESC[2m2026-10-06T10:48:08.905692Z ESC[0m ... listening at bore.pub:62457
//
// CLI shape (verified on v0.5.1 AND v0.6.0): `bore local` takes the local
// port as a POSITIONAL argument; there is no `--local-port` flag:
//   bore local --to bore.pub 8099          (correct)
//   bore local --local-port 8099 --to bore.pub   (error: unexpected argument)

use std::path::Path;

use crate::models::{Backend, TunnelConfig};

/// Public relay server used by the bore backend in M1 (config has no field
/// for it yet).
pub const BORE_DEFAULT_SERVER: &str = "bore.pub";

/// Build the argv (without the program itself) for the backend binary.
pub fn build_args(cfg: &TunnelConfig) -> Vec<String> {
    match cfg.backend {
        Backend::Cloudflare => vec![
            "tunnel".to_string(),
            "--url".to_string(),
            format!("http://{}:{}", cfg.local_host, cfg.local_port),
            "--no-autoupdate".to_string(),
        ],
        Backend::Bore => vec![
            "local".to_string(),
            "--to".to_string(),
            BORE_DEFAULT_SERVER.to_string(),
            cfg.local_port.to_string(),
        ],
        // Frp tunnels spawn `frpc -c <generated config file>`; the real
        // implementation lives with the M2 frp provider.
        Backend::Frp => vec![],
    }
}

/// Build a tokio command that runs `binary` with the backend's arguments.
pub fn build_command(cfg: &TunnelConfig, binary: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(binary);
    cmd.args(build_args(cfg));
    cmd
}

/// Parse the public endpoint from a process output line, per backend.
///
/// * cloudflare -> `https://<name>.trycloudflare.com`
/// * bore       -> `<host>:<port>` (e.g. `bore.pub:62457`)
pub fn parse_public_endpoint(backend: Backend, line: &str) -> Option<String> {
    match backend {
        Backend::Cloudflare => extract_quick_tunnel_url(line),
        Backend::Bore => extract_bore_endpoint(line)
            .or_else(|| extract_remote_port(line).map(|p| format!("{BORE_DEFAULT_SERVER}:{p}"))),
        // Real parsing (frpc "start proxy success" lines) lands with M2.
        Backend::Frp => None,
    }
}

/// Remove ANSI escape sequences (CSI ... final-byte). bore colorizes its log
/// lines even when stdout is a pipe, so this runs before every parse.
pub fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // CSI sequence: consume parameters until the final byte
                // (0x40..=0x7E, e.g. `m` for SGR).
                for f in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&f) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn is_host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '.'
}

/// Loosely match `https://[a-z0-9-]+.trycloudflare.com` anywhere in the line.
/// cloudflared wraps the URL in ASCII-art `| ... |` table cells with padded
/// spaces, so we can't rely on exact token boundaries.
pub fn extract_quick_tunnel_url(line: &str) -> Option<String> {
    const SUFFIX: &str = ".trycloudflare.com";
    const NEEDLE: &str = "https://";
    // to_ascii_lowercase preserves byte offsets, so matching against the
    // lowercased line is safe for slicing.
    let lower = strip_ansi(line).to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(idx) = lower[from..].find(NEEDLE) {
        let start = from + idx + NEEDLE.len();
        let host_len = lower[start..]
            .chars()
            .take_while(|c| is_host_char(*c))
            .map(char::len_utf8)
            .sum::<usize>();
        let host = lower[start..start + host_len].trim_end_matches('.');
        if let Some(label) = host.strip_suffix(SUFFIX) {
            if !label.is_empty()
                && label
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                return Some(format!("https://{host}"));
            }
        }
        from = start;
    }
    None
}

/// Parse `<host>:<port>` out of a single token. Returns the full endpoint.
fn parse_host_port(token: &str) -> Option<String> {
    let (host, port) = token.rsplit_once(':')?;
    if host.is_empty() || !host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return None;
    }
    // Must look like a host (dotted or containing letters), not a bare number.
    if !host.contains('.') && !host.chars().any(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let port: u16 = port.parse().ok()?;
    if port == 0 {
        return None;
    }
    Some(format!("{host}:{port}"))
}

/// Parse the bore public endpoint from a log line. Only lines that plausibly
/// announce the forwarding are considered, so the control-channel line
/// (`connected to server bore.pub:2200`) is never mistaken for the endpoint.
pub fn extract_bore_endpoint(line: &str) -> Option<String> {
    let plain = strip_ansi(line);
    let lower = plain.to_ascii_lowercase();
    let announces_forwarding = lower.contains("listening")
        || lower.contains("forwarding")
        || lower.contains("remote_port")
        || lower.contains("remote port");
    if !announces_forwarding {
        return None;
    }
    for token in plain.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '|' | '"'
                    | '\''
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | ','
                    | ';'
            )
    }) {
        let token = token.trim_matches(|c: char| c == '.' || c == ':');
        if let Some(ep) = parse_host_port(token) {
            return Some(ep);
        }
    }
    None
}

/// Fallback for older/phrased-differently bore output such as
/// `connected to server remote_port=62457` (no host in the line; the caller
/// combines the port with the known bore server).
pub fn extract_remote_port(line: &str) -> Option<u16> {
    let plain = strip_ansi(line);
    let lower = plain.to_ascii_lowercase();
    let idx = lower
        .find("remote_port=")
        .or_else(|| lower.find("remote port "))?;
    let digits: String = plain[idx..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u16>().ok().filter(|p| *p != 0)
}

/// Substrings (lowercased) that mark an error-ish output line. These only
/// surface in `state.error` for display — the engine decides life/death by
/// process exit, never by a single log line.
const CLOUDFLARE_ERROR_MARKERS: &[&str] = &[
    "failed to connect",
    "could not start tunnel",
    "failed to serve",
    "failed to request quick tunnel",
    "unable to reach the origin",
    "connection refused",
    "failed to dial",
    "handshake failed",
    "dial tcp",
];

const BORE_ERROR_MARKERS: &[&str] = &[
    "error", "failed", "refused", "timed out", "panic", "denied", "invalid",
];

pub fn is_error_line(backend: Backend, line: &str) -> bool {
    let lower = strip_ansi(line).to_ascii_lowercase();
    match backend {
        Backend::Cloudflare => CLOUDFLARE_ERROR_MARKERS.iter().any(|m| lower.contains(m)),
        Backend::Bore => BORE_ERROR_MARKERS.iter().any(|m| lower.contains(m)),
        // Refined markers land with the M2 frp provider.
        Backend::Frp => lower.contains("error") || lower.contains("failed"),
    }
}

/// Map one output line to a log level ("info" | "warn" | "error") for the
/// `tunnel://log` event. cloudflared prefixes INF/WRN/ERR; bore emits
/// INFO/WARN/ERROR tokens (wrapped in ANSI codes).
pub fn classify_level(backend: Backend, line: &str) -> &'static str {
    let plain = strip_ansi(line);
    for token in plain.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
        match token.to_ascii_lowercase().as_str() {
            "err" | "error" | "fatal" => return "error",
            "wrn" | "warn" | "warning" => return "warn",
            "inf" | "info" | "dbg" | "debug" | "trace" => return "info",
            _ => {}
        }
    }
    if is_error_line(backend, &plain) {
        "error"
    } else {
        "info"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Backend, TunnelType};

    fn cfg(backend: Backend, port: u16) -> TunnelConfig {
        TunnelConfig {
            id: "t1".into(),
            name: "t".into(),
            tunnel_type: TunnelType::Http,
            backend,
            local_host: "127.0.0.1".into(),
            local_port: port,
            auto_start: false,
            created_at: String::new(),
            server_id: None,
            subdomain: None,
            remote_port: None,
        }
    }

    // --- command builders -------------------------------------------------

    #[test]
    fn cloudflare_args() {
        assert_eq!(
            build_args(&cfg(Backend::Cloudflare, 8080)),
            vec![
                "tunnel",
                "--url",
                "http://127.0.0.1:8080",
                "--no-autoupdate"
            ]
        );
    }

    #[test]
    fn bore_args_use_positional_port() {
        // Verified: `bore local --local-port N` is REJECTED by v0.5.1/v0.6.0;
        // the local port is a positional argument.
        assert_eq!(
            build_args(&cfg(Backend::Bore, 8099)),
            vec!["local", "--to", "bore.pub", "8099"]
        );
    }

    // --- cloudflared URL parsing -------------------------------------------

    const CF_URL_LINE: &str = "2026-10-06T10:48:20Z INF |  https://vessels-arrow-kid-educated.trycloudflare.com                                      |";

    #[test]
    fn cloudflare_url_from_real_output_line() {
        assert_eq!(
            extract_quick_tunnel_url(CF_URL_LINE).as_deref(),
            Some("https://vessels-arrow-kid-educated.trycloudflare.com")
        );
    }

    #[test]
    fn cloudflare_url_bare_and_padded() {
        assert_eq!(
            extract_quick_tunnel_url("your url: https://a-b-c.trycloudflare.com").as_deref(),
            Some("https://a-b-c.trycloudflare.com")
        );
        assert_eq!(
            extract_quick_tunnel_url("https://word.trycloudflare.com, enjoy").as_deref(),
            Some("https://word.trycloudflare.com")
        );
        // trailing dot is tolerated
        assert_eq!(
            extract_quick_tunnel_url("https://word.trycloudflare.com.").as_deref(),
            Some("https://word.trycloudflare.com")
        );
    }

    #[test]
    fn cloudflare_ignores_non_quick_urls() {
        assert_eq!(
            extract_quick_tunnel_url(
                "INF pre-created named tunnel by following: https://developers.cloudflare.com/cloudflare-one/connections/connect-apps"
            ),
            None
        );
        assert_eq!(
            extract_quick_tunnel_url("INF Starting metrics server on 127.0.0.1:20241/metrics"),
            None
        );
        assert_eq!(extract_quick_tunnel_url("no urls here"), None);
    }

    #[test]
    fn cloudflare_rejects_lookalike_hosts() {
        // Suffix must be followed by the label; a sub-sub domain is not quick-tunnel.
        assert_eq!(
            extract_quick_tunnel_url("https://evil.com/u=https://good.trycloudflare.com.evil.com"),
            None
        );
        // Missing label before the suffix.
        assert_eq!(extract_quick_tunnel_url("https://.trycloudflare.com"), None);
        assert_eq!(extract_quick_tunnel_url("https://trycloudflare.com"), None);
    }

    // --- bore endpoint parsing ----------------------------------------------

    const BORE_LISTEN_LINE: &str = "\u{1b}[2m2026-10-06T10:48:08.905692Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mbore_cli::client\u{1b}[0m\u{1b}[2m:\u{1b}[0m listening at bore.pub:62457";
    const BORE_CONNECT_LINE: &str = "\u{1b}[2m2026-10-06T10:48:08.904579Z\u{1b}[0m \u{1b}[32m INFO\u{1b}[0m \u{1b}[2mbore_cli::client\u{1b}[0m\u{1b}[2m:\u{1b}[0m connected to server \u{1b}[3mremote_port\u{1b}[0m\u{1b}[2m=\u{1b}[0m62457";

    #[test]
    fn bore_endpoint_from_real_listening_line() {
        assert_eq!(
            extract_bore_endpoint(BORE_LISTEN_LINE).as_deref(),
            Some("bore.pub:62457")
        );
    }

    #[test]
    fn bore_endpoint_falls_back_to_remote_port() {
        // The "connected to server remote_port=N" line has no host:port token,
        // so extract_bore_endpoint misses it and the remote_port fallback kicks in.
        assert_eq!(extract_bore_endpoint(BORE_CONNECT_LINE), None);
        assert_eq!(extract_remote_port(BORE_CONNECT_LINE), Some(62457));
        assert_eq!(
            parse_public_endpoint(Backend::Bore, BORE_CONNECT_LINE).as_deref(),
            Some("bore.pub:62457")
        );
        assert_eq!(
            parse_public_endpoint(Backend::Bore, BORE_LISTEN_LINE).as_deref(),
            Some("bore.pub:62457")
        );
    }

    #[test]
    fn bore_accepts_forwarding_and_remote_port_phrases() {
        assert_eq!(
            extract_bore_endpoint("forwarding to bore.pub:15967").as_deref(),
            Some("bore.pub:15967")
        );
        assert_eq!(
            extract_bore_endpoint("remote port 12345 -> 10.0.0.1:8080").as_deref(),
            Some("10.0.0.1:8080")
        );
    }

    #[test]
    fn bore_ignores_control_channel_and_noise() {
        // Control connection port (2200) must never become the public endpoint.
        assert_eq!(
            extract_bore_endpoint("INFO bore_cli::client: connected to server bore.pub:2200"),
            None
        );
        assert_eq!(extract_bore_endpoint("ERROR connection refused"), None);
        assert_eq!(extract_bore_endpoint("plain text"), None);
        // "listening" gated but no host:port token present.
        assert_eq!(extract_bore_endpoint("listening for connections"), None);
    }

    // --- level classification -----------------------------------------------

    #[test]
    fn cloudflared_levels() {
        assert_eq!(
            classify_level(Backend::Cloudflare, "2026-10-06T10:48:20Z INF Version 2026.10.0"),
            "info"
        );
        assert_eq!(
            classify_level(
                Backend::Cloudflare,
                "2026-10-06T10:48:25Z ERR Failed to dial a quic connection error=\"timeout\" connIndex=0"
            ),
            "error"
        );
        assert_eq!(
            classify_level(Backend::Cloudflare, "2026-10-06T10:48:25Z WRN Retrying connection in up to 2s"),
            "warn"
        );
        // Level token absent -> keyword fallback.
        assert_eq!(
            classify_level(Backend::Cloudflare, "failed to connect to the origin"),
            "error"
        );
        assert_eq!(classify_level(Backend::Cloudflare, "just text"), "info");
    }

    #[test]
    fn bore_levels_with_ansi() {
        assert_eq!(classify_level(Backend::Bore, BORE_LISTEN_LINE), "info");
        assert_eq!(
            classify_level(
                Backend::Bore,
                "\u{1b}[31mERROR\u{1b}[0m \u{1b}[2mbore_cli::client\u{1b}[0m: connection refused"
            ),
            "error"
        );
        assert_eq!(
            classify_level(Backend::Bore, "\u{1b}[33m WARN\u{1b}[0m something"),
            "warn"
        );
    }

    #[test]
    fn strip_ansi_removes_all_sequences() {
        assert_eq!(strip_ansi(BORE_LISTEN_LINE), "2026-10-06T10:48:08.905692Z  INFO bore_cli::client: listening at bore.pub:62457");
        assert_eq!(strip_ansi("no escapes"), "no escapes");
        assert_eq!(strip_ansi("\u{1b}[1;32mgreen\u{1b}[0m"), "green");
        assert_eq!(strip_ansi("stray \u{1b} escape"), "stray  escape");
    }
}
