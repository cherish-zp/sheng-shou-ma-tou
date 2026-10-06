// One-click diagnosis — turn a tunnel's error state + recent log lines into
// a structured result: a stable `code` slug (the frontend renders localized
// title/suggestions from `diagnosis.<code>.*` i18n keys) plus `detail`
// holding the raw evidence line (truncated to 300 chars).
//
// Codes (keep in sync with the frontend i18n dictionary):
//   tokenMismatch, authFailed, versionMismatch, portConflict,
//   connectionRefused, dnsFailed, connectionTimeout (NEW — report to
//   frontend), localServiceDown, binaryMissing, remoteServerUnreachable,
//   genericError, allHealthy
//
// Rules are calibrated against real frpc 0.71.0 output (see providers.rs):
// * fatal login failures log as [W] "connect to server error: token in login
//   doesn't match token from configuration" plus a bare "login to the server
//   failed: ..." stderr line — note the "the".
// * a dead local service logs as [E] "... connect to local service [ip:port]
//   error: dial tcp ...: connect: connection refused" — the "connection
//   refused" rule must NOT win over localServiceDown for that line.
// * DNS resolution failures surface only as "connect to server error:
//   session shutdown" (the resolver error is swallowed by frp), so dnsFailed
//   mostly fires for other-version outputs; such lines fall through to
//   genericError.
use crate::models::{Diagnosis, DiagnosisLevel};

/// Cap for the `detail` evidence string.
const DETAIL_MAX_LEN: usize = 300;

pub fn diagnose(tunnel_id: &str, state_error: Option<&str>, recent_logs: &[String]) -> Diagnosis {
    // Evidence pool in DOCUMENT order: ring buffer (oldest -> newest) then
    // the terminal state error. Every rule scans it BACKWARDS, so the
    // effective priority is: state error first, then log lines newest-first.
    let mut evidence: Vec<&str> = recent_logs.iter().map(String::as_str).collect();
    if let Some(err) = state_error.map(str::trim).filter(|s| !s.is_empty()) {
        evidence.push(err);
    }

    let rule_hits = |needles: &[&str]| {
        evidence
            .iter()
            .find(|line| {
                let lower = line.to_ascii_lowercase();
                needles.iter().all(|n| lower.contains(n))
            })
            .copied()
    };

    let with_code = |code: &str, level: DiagnosisLevel, detail: &str| Diagnosis {
        tunnel_id: tunnel_id.to_string(),
        code: code.to_string(),
        level,
        detail: truncate(detail, DETAIL_MAX_LEN),
    };

    // --- log-level rules, in priority order ---------------------------------
    // 1. token mismatch: frpc "login to the server failed: token in login
    //    doesn't match token from configuration" (also older "login to
    //    server failed" phrasing).
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        (lower.contains("token in login doesn't match")
            || (lower.contains("login to")
                && lower.contains("server failed")
                && (lower.contains("token") || lower.contains("auth"))))
            && (lower.contains("token") || lower.contains("auth"))
    }) {
        return with_code("tokenMismatch", DiagnosisLevel::Error, line);
    }
    // 2. auth failures.
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("user is disabled") || lower.contains("authentication failed")
    }) {
        return with_code("authFailed", DiagnosisLevel::Error, line);
    }
    // 3. version mismatch.
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("version mismatch") || lower.contains("incompatible")
    }) {
        return with_code("versionMismatch", DiagnosisLevel::Error, line);
    }
    // 4. port conflicts (frpc remotePort allocation, frps bind errors).
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("address already in use")
            || lower.contains("port already used")
            || lower.contains("bind port")
    }) {
        return with_code("portConflict", DiagnosisLevel::Error, line);
    }
    // 5. connection refused while dialing the SERVER (the local-service
    //    variant of the same message is handled by localServiceDown below —
    //    real frpc line: "connect to local service [...] error: dial tcp ...
    //    connection refused").
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("connection refused")
            && !lower.contains("local service")
            && (lower.contains("connect to server") || lower.contains("dial tcp"))
    }) {
        return with_code("connectionRefused", DiagnosisLevel::Error, line);
    }
    // 6. DNS resolution failure.
    if let Some(line) = rule_hits(&["no such host"]) {
        return with_code("dnsFailed", DiagnosisLevel::Error, line);
    }
    // 7. dial timeout (NEW code — frontend i18n needs a connectionTimeout
    //    entry; reported separately).
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        (lower.contains("i/o timeout") || lower.contains("connection timed out"))
            && lower.contains("dial")
    }) {
        return with_code("connectionTimeout", DiagnosisLevel::Warn, line);
    }
    // 8. the local service behind the tunnel is down.
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("connect to local service")
            || (lower.contains("local service")
                && (lower.contains("error") || lower.contains("failed")))
    }) {
        return with_code("localServiceDown", DiagnosisLevel::Error, line);
    }

    // --- state_error-only rules ---------------------------------------------
    if let Some(err) = state_error.map(str::trim).filter(|s| !s.is_empty()) {
        let lower = err.to_ascii_lowercase();
        // Deployment problems (ssh/deploy pipeline errors from frp_deploy).
        if lower.contains("deploy") {
            return with_code("remoteServerUnreachable", DiagnosisLevel::Error, err);
        }
        // Engine binary problems ("engine binary for Frp not found (...)").
        if lower.contains("binary") || lower.contains("not found") {
            return with_code("binaryMissing", DiagnosisLevel::Error, err);
        }
        return with_code("genericError", DiagnosisLevel::Error, err);
    }

    // --- fallback: unrecognized error line in the ring buffer ----------------
    if let Some(line) = evidence.iter().rev().find(|line| {
        let lower = line.to_ascii_lowercase();
        lower.contains("[e]")
            || lower.contains("error")
            || lower.contains("failed")
            || lower.contains("refused")
            || lower.contains("panic")
    }) {
        return with_code("genericError", DiagnosisLevel::Error, line);
    }

    with_code("allHealthy", DiagnosisLevel::Info, "no error lines found in recent tunnel output")
}

fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars).collect();
    out.push('\u{2026}');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DiagnosisLevel;

    fn logs(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|s| s.to_string()).collect()
    }

    fn assert_code(d: &Diagnosis, code: &str, level: DiagnosisLevel) {
        assert_eq!(d.code, code, "code mismatch, detail: {}", d.detail);
        assert_eq!(d.level, level, "level mismatch for {code}");
        assert!(!d.detail.is_empty(), "{code} must carry evidence");
    }

    /// Real frpc 0.71.0 token-mismatch evidence (ANSI stripped).
    const TOKEN_LINE: &str = "2026-10-06 20:18:02.936 [W] [client/service.go:323] connect to server error: token in login doesn't match token from configuration";
    const BARE_LOGIN_FAILED: &str = "login to the server failed: token in login doesn't match token from configuration. With loginFailExit enabled, no additional retries will be attempted";
    const LOCAL_SERVICE_LINE: &str = "2026-10-06 20:20:57.726 [E] [proxy/proxy.go:237] [8ffeba1af99c60df] [web-tcp] connect to local service [127.0.0.1:8080] error: dial tcp 127.0.0.1:8080: connect: connection refused";
    const SERVER_REFUSED_LINE: &str = "2026-10-06 20:19:56.281 [W] [client/service.go:323] connect to server error: dial tcp 127.0.0.1:19999: connect: connection refused";
    const PORT_USED_LINE: &str = "2026-10-06 20:19:35.272 [W] [client/control.go:172] [fbcb25fbd0d6eaab] [other-tcp] start error: port already used";

    #[test]
    fn token_mismatch() {
        let d = diagnose("t1", None, &logs(&[TOKEN_LINE, BARE_LOGIN_FAILED]));
        assert_code(&d, "tokenMismatch", DiagnosisLevel::Error);
    }

    #[test]
    fn token_mismatch_older_phrasing() {
        // Older frp versions log "login to server failed: token ...".
        let d = diagnose(
            "t1",
            None,
            &logs(&["[E] login to server failed: token in login doesn't match token in configuration"]),
        );
        assert_code(&d, "tokenMismatch", DiagnosisLevel::Error);
    }

    #[test]
    fn auth_failed() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[E] [client/service.go:261] login to server failed: user is disabled"]),
        );
        assert_code(&d, "authFailed", DiagnosisLevel::Error);
    }

    #[test]
    fn version_mismatch() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[W] protocol version mismatch, please update frps"]),
        );
        assert_code(&d, "versionMismatch", DiagnosisLevel::Error);
    }

    #[test]
    fn port_conflict() {
        let d = diagnose("t1", None, &logs(&[PORT_USED_LINE]));
        assert_code(&d, "portConflict", DiagnosisLevel::Error);
    }

    #[test]
    fn port_conflict_bind_address_in_use() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[E] [server.go:88] run error: listen tcp: bind 0.0.0.0:7000: address already in use"]),
        );
        assert_code(&d, "portConflict", DiagnosisLevel::Error);
    }

    #[test]
    fn connection_refused_to_server() {
        let d = diagnose("t1", None, &logs(&[SERVER_REFUSED_LINE]));
        assert_code(&d, "connectionRefused", DiagnosisLevel::Error);
    }

    #[test]
    fn local_service_down_beats_connection_refused() {
        // The real line contains BOTH "connection refused" and "dial tcp";
        // the "local service" evidence must win.
        let d = diagnose("t1", None, &logs(&[LOCAL_SERVICE_LINE]));
        assert_code(&d, "localServiceDown", DiagnosisLevel::Error);
    }

    #[test]
    fn dns_failed() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[W] connect to server error: dial tcp: lookup myserver.example.net: no such host"]),
        );
        assert_code(&d, "dnsFailed", DiagnosisLevel::Error);
    }

    #[test]
    fn connection_timeout() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[W] connect to server error: dial tcp 1.2.3.4:7000: i/o timeout"]),
        );
        assert_code(&d, "connectionTimeout", DiagnosisLevel::Warn);
    }

    #[test]
    fn binary_missing_from_state_error() {
        let d = diagnose(
            "t1",
            Some("engine binary for Frp not found (looked in app data dir and PATH)"),
            &[],
        );
        assert_code(&d, "binaryMissing", DiagnosisLevel::Error);
    }

    #[test]
    fn remote_server_unreachable_from_deploy_error() {
        let d = diagnose(
            "t1",
            Some("deploy failed: ssh dial to 1.2.3.4:22 timed out"),
            &[],
        );
        assert_code(&d, "remoteServerUnreachable", DiagnosisLevel::Error);
    }

    #[test]
    fn generic_error_from_unrecognized_state_error() {
        let d = diagnose("t1", Some("something exploded"), &[]);
        assert_code(&d, "genericError", DiagnosisLevel::Error);
    }

    #[test]
    fn generic_error_from_unrecognized_log_line() {
        let d = diagnose(
            "t1",
            None,
            &logs(&[
                "[I] login to server success, get run id [x]",
                "[W] connect to server error: session shutdown",
            ]),
        );
        assert_code(&d, "genericError", DiagnosisLevel::Error);
        assert!(d.detail.contains("session shutdown"));
    }

    #[test]
    fn all_healthy() {
        let d = diagnose(
            "t1",
            None,
            &logs(&[
                "[I] try to connect to server...",
                "[I] login to server success, get run id [x]",
                "[I] [web-tcp] start proxy success",
            ]),
        );
        assert_code(&d, "allHealthy", DiagnosisLevel::Info);
    }

    #[test]
    fn all_healthy_with_no_evidence() {
        let d = diagnose("t1", None, &[]);
        assert_code(&d, "allHealthy", DiagnosisLevel::Info);
    }

    #[test]
    fn priority_token_mismatch_over_generic() {
        // Newest-first: the bare login-failed line is the LAST log line, but
        // tokenMismatch is the higher-priority rule for both lines.
        let d = diagnose(
            "t1",
            None,
            &logs(&[TOKEN_LINE, SERVER_REFUSED_LINE, BARE_LOGIN_FAILED]),
        );
        assert_code(&d, "tokenMismatch", DiagnosisLevel::Error);
    }

    #[test]
    fn detail_is_truncated_to_300_chars() {
        let long = "x".repeat(1000);
        let d = diagnose("t1", Some(&long), &[]);
        assert_eq!(d.code, "genericError");
        assert_eq!(d.detail.chars().count(), 301); // 300 + ellipsis
    }

    #[test]
    fn newest_error_line_wins_within_same_rule() {
        let d = diagnose(
            "t1",
            None,
            &logs(&["[E] first connect to local service [a] error", "[E] second connect to local service [b] error"]),
        );
        assert!(d.detail.contains("[b]"), "newest line expected: {}", d.detail);
    }
}
