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
// Byte-count semantics ("in" = direction of travel INTO the local service):
//   * bytes_in  — bytes arriving from the public internet through the tunnel
//     and handed to the local service (client -> upstream),
//   * bytes_out — bytes returned by the local service back through the
//     tunnel (upstream -> client).
//
// Public API below is frozen — commands.rs and engine.rs code against it.
use std::collections::HashMap;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use base64::Engine as _;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::engine::{TunnelLogEvent, TUNNEL_LOG_EVENT};
use crate::models::{TunnelAuth, TunnelConfig, TunnelStats, TunnelType};

/// Per-tunnel traffic stats, emitted by the forwarder at most once per second
/// (only when something changed) plus one final event on stop.
pub const TUNNEL_STATS_EVENT: &str = "tunnel://stats";

/// Upper bound for the intercepted HTTP request head (headers only).
const HEADER_BLOCK_CAP: usize = 8 * 1024;
/// A client that sends neither a complete head nor EOF within this window is
/// dropped, so a stalled socket cannot hold a forwarder slot forever.
const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(10);
const STATS_INTERVAL: Duration = Duration::from_secs(1);
/// Same-key rate limit for the "cannot reach local service" log line.
const UNREACHABLE_LOG_INTERVAL_MS: u64 = 1_000;
/// Upper bound for draining tasks during `stop()` before giving up on a
/// single one (they are aborted anyway; this only bounds the wait).
const STOP_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);

const UNAUTHORIZED_RESPONSE: &[u8] = b"HTTP/1.1 401 Unauthorized\r\n\
WWW-Authenticate: Basic realm=\"Pier\"\r\n\
Content-Length: 0\r\n\
Connection: close\r\n\
\r\n";

/// A running forwarder bound to 127.0.0.1 on an ephemeral port.
pub struct Forwarder {
    /// `None` only in unit tests (no Tauri app available there): emits become
    /// no-ops while every other behaviour stays identical.
    app: Option<AppHandle>,
    tunnel_id: String,
    port: u16,
    /// The real user service the forwarder proxies to.
    upstream_host: String,
    upstream_port: u16,
    /// `Some((username, password))` for HTTP-type tunnels with basic auth.
    /// TCP tunnels never carry it: their payload is an arbitrary protocol
    /// whose headers cannot be intercepted (documented limitation).
    auth: Option<(String, String)>,
    allowlist: Arc<Vec<String>>,
    bytes_in: Arc<AtomicU64>,
    bytes_out: Arc<AtomicU64>,
    conn_active: Arc<AtomicU32>,
    /// `true` once stop was requested; observed by the accept loop, the stats
    /// loop and queried via `is_stopped()`.
    stop_tx: watch::Sender<bool>,
    accept_task: Mutex<Option<JoinHandle<()>>>,
    /// Live proxied connections; aborted and drained by `stop()`.
    conns: Mutex<HashMap<u64, JoinHandle<()>>>,
    next_conn_id: AtomicU64,
    last_unreachable_log_ms: AtomicU64,
    final_stats_emitted: AtomicBool,
}

impl Forwarder {
    /// Start a forwarder for `cfg` and emit throttled stats events on `app`.
    /// Returns the local port the tunnel binary must connect to.
    pub async fn start(
        app: AppHandle,
        cfg: TunnelConfig,
        auth_password: Option<String>,
    ) -> Result<(Arc<Forwarder>, u16), String> {
        Self::spawn(Some(app), cfg, auth_password).await
    }

    /// Real constructor; `app` is optional so unit tests can run the full
    /// pipeline without a Tauri handle.
    async fn spawn(
        app: Option<AppHandle>,
        cfg: TunnelConfig,
        auth_password: Option<String>,
    ) -> Result<(Arc<Forwarder>, u16), String> {
        let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
            format!("forwarder: 绑定本地监听端口失败 (failed to bind local listener): {e}")
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("forwarder: failed to read local addr: {e}"))?
            .port();

        let (stop_tx, stop_rx) = watch::channel(false);
        // Basic auth only applies to HTTP-type tunnels whose auth.kind is
        // "basic"; a TCP tunnel's payload is an opaque protocol, so
        // intercepting headers is impossible.
        let auth = match cfg.tunnel_type {
            TunnelType::Http => cfg
                .auth
                .as_ref()
                .filter(|a| a.kind == TunnelAuth::BASIC)
                .zip(auth_password)
                .map(|(a, p)| (a.username.clone(), p)),
            TunnelType::Tcp => None,
        };

        let fwd = Arc::new(Self {
            app,
            tunnel_id: cfg.id.clone(),
            port,
            upstream_host: cfg.local_host.clone(),
            upstream_port: cfg.local_port,
            auth,
            allowlist: Arc::new(cfg.ip_allowlist.clone()),
            bytes_in: Arc::new(AtomicU64::new(0)),
            bytes_out: Arc::new(AtomicU64::new(0)),
            conn_active: Arc::new(AtomicU32::new(0)),
            stop_tx,
            accept_task: Mutex::new(None),
            conns: Mutex::new(HashMap::new()),
            next_conn_id: AtomicU64::new(0),
            last_unreachable_log_ms: AtomicU64::new(0),
            final_stats_emitted: AtomicBool::new(false),
        });

        let accept = tokio::spawn(accept_loop(listener, fwd.clone(), stop_rx.clone()));
        *fwd.accept_task.lock().unwrap_or_else(|e| e.into_inner()) = Some(accept);
        tokio::spawn(stats_loop(fwd.clone(), stop_rx));
        Ok((fwd, port))
    }

    /// Stop the forwarder and all proxied connections. Idempotent.
    pub async fn stop(&self) {
        // 1. Signal: the accept loop exits and closes the listening socket.
        let _ = self.stop_tx.send(true);
        // 2. Wait for the accept loop to finish (it owns the listener).
        let accept = self
            .accept_task
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(task) = accept {
            let _ = timeout(STOP_DRAIN_TIMEOUT, task).await;
        }
        // 3. Abort every proxied connection; awaiting the aborted handles
        //    guarantees their locals (sockets, conn_active guard) are dropped
        //    before the final stats event is emitted.
        let conns: Vec<JoinHandle<()>> = self
            .conns
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .drain()
            .map(|(_, task)| task)
            .collect();
        for task in conns {
            task.abort();
            let _ = timeout(STOP_DRAIN_TIMEOUT, task).await;
        }
        // 4. One final stats event (the periodic loop is gone by now).
        if !self.final_stats_emitted.swap(true, Ordering::SeqCst) {
            self.emit_stats();
        }
    }

    /// The local port the tunnel binary must dial.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Cumulative bytes handed to the local service since the last reset.
    pub fn bytes_in(&self) -> u64 {
        self.bytes_in.load(Ordering::SeqCst)
    }

    /// Cumulative bytes returned by the local service since the last reset.
    pub fn bytes_out(&self) -> u64 {
        self.bytes_out.load(Ordering::SeqCst)
    }

    /// True when this forwarder still proxies to the given target (used to
    /// decide whether an existing forwarder can be reused after a restart).
    pub fn upstream_matches(&self, host: &str, port: u16) -> bool {
        self.upstream_host == host && self.upstream_port == port
    }

    /// True once `stop()` was requested (the forwarder is unusable).
    pub fn is_stopped(&self) -> bool {
        *self.stop_tx.borrow()
    }

    /// Zero the byte counters for a fresh run. Traffic from previous runs must
    /// not leak into the new run's card display. `conn_active` is intentionally
    /// left alone — it tracks live connections, not history.
    pub fn reset_counters(&self) {
        self.bytes_in.store(0, Ordering::SeqCst);
        self.bytes_out.store(0, Ordering::SeqCst);
    }

    fn counters(&self) -> (u64, u64, u32) {
        (
            self.bytes_in.load(Ordering::SeqCst),
            self.bytes_out.load(Ordering::SeqCst),
            self.conn_active.load(Ordering::SeqCst),
        )
    }

    fn emit_stats(&self) {
        let Some(app) = &self.app else { return };
        let (bytes_in, bytes_out, conn_active) = self.counters();
        let _ = app.emit(
            TUNNEL_STATS_EVENT,
            TunnelStats {
                tunnel_id: self.tunnel_id.clone(),
                bytes_in,
                bytes_out,
                conn_active,
            },
        );
    }

    /// "Local service unreachable" log line, rate-limited to one per second.
    fn log_unreachable(&self, err: &std::io::Error) {
        let now = now_ms();
        let last = self.last_unreachable_log_ms.load(Ordering::Relaxed);
        if now.saturating_sub(last) < UNREACHABLE_LOG_INTERVAL_MS {
            return;
        }
        self.last_unreachable_log_ms.store(now, Ordering::Relaxed);
        let line = format!(
            "forwarder: cannot reach local service {}:{}: {err}",
            self.upstream_host, self.upstream_port
        );
        if let Some(app) = &self.app {
            let _ = app.emit(
                TUNNEL_LOG_EVENT,
                TunnelLogEvent {
                    tunnel_id: self.tunnel_id.clone(),
                    level: "error",
                    line,
                    ts: chrono::Utc::now().to_rfc3339(),
                },
            );
        }
    }
}

/// Accept loop: spawns one handler task per inbound connection until stopped.
/// The listening socket is owned by this task, so exiting it releases the port.
async fn accept_loop(
    listener: TcpListener,
    fwd: Arc<Forwarder>,
    mut stop_rx: watch::Receiver<bool>,
) {
    loop {
        let accepted = tokio::select! {
            res = listener.accept() => res,
            _ = wait_for_stop(&mut stop_rx) => break,
        };
        let (stream, peer) = match accepted {
            Ok(pair) => pair,
            Err(e) => {
                // Transient accept failure (e.g. fd pressure): back off a
                // little instead of spinning, and keep serving.
                eprintln!("[pier] forwarder accept error: {e}");
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }
        };

        let id = fwd.next_conn_id.fetch_add(1, Ordering::Relaxed);
        let conn_fwd = fwd.clone();
        let task = tokio::spawn(async move {
            conn_fwd.conn_active.fetch_add(1, Ordering::SeqCst);
            let _guard = ConnGuard(conn_fwd.conn_active.clone());
            handle_conn(&conn_fwd, stream, peer.ip()).await;
            conn_fwd
                .conns
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&id);
        });
        let mut conns = fwd.conns.lock().unwrap_or_else(|e| e.into_inner());
        // The task may already have finished (and removed itself) between
        // spawn and lock; don't keep a stale entry around.
        if !task.is_finished() {
            conns.insert(id, task);
        }
    }
}

/// One proxied connection: allowlist -> optional basic-auth interception ->
/// upstream connect -> counted bidirectional copy.
async fn handle_conn(fwd: &Forwarder, mut stream: TcpStream, peer: IpAddr) {
    if !ip_allowed(peer, &fwd.allowlist) {
        // Denied by the allowlist: close without exchanging a single byte.
        return;
    }

    // HTTP tunnels with basic auth: intercept the request head, validate it,
    // and (on success) stitch the buffered bytes back in front of the stream.
    let mut head: Vec<u8> = Vec::new();
    if let Some((username, password)) = &fwd.auth {
        match read_request_head(&mut stream).await {
            Some(block) => {
                let authorized = extract_authorization(&block)
                    .map(|value| basic_auth_ok(&value, username, password))
                    .unwrap_or(false);
                if !authorized {
                    let _ = stream.write_all(UNAUTHORIZED_RESPONSE).await;
                    let _ = stream.flush().await;
                    return;
                }
                head = block;
            }
            // Malformed / oversized / stalled / half-closed request head.
            None => return,
        }
    }

    let mut upstream =
        match TcpStream::connect((fwd.upstream_host.as_str(), fwd.upstream_port)).await {
            Ok(u) => u,
            Err(e) => {
                fwd.log_unreachable(&e);
                return;
            }
        };

    if !head.is_empty() {
        // The buffered head goes UPSTREAM (to the local service), not back to
        // the client — `stream` is the client side of the connection.
        if upstream.write_all(&head).await.is_err() {
            return;
        }
        // The buffered head entered through the tunnel: count it as inbound.
        fwd.bytes_in.fetch_add(head.len() as u64, Ordering::SeqCst);
    }

    // Counters wrap the client side only, so every byte crosses exactly one
    // counter: reads = bytes arriving from the tunnel (in), writes = bytes
    // going back out to the tunnel (out). Mid-stream errors are routine
    // (client aborts, service closes); the counters keep their sums.
    let mut counted = CountingStream {
        inner: stream,
        bytes_in: fwd.bytes_in.clone(),
        bytes_out: fwd.bytes_out.clone(),
    };
    let _ = tokio::io::copy_bidirectional(&mut counted, &mut upstream).await;
}

/// Read the request head (up to and including the blank line) with an 8KB cap
/// and a read timeout. `None` = malformed, oversized, stalled or closed.
async fn read_request_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    loop {
        if buf.len() > HEADER_BLOCK_CAP {
            return None;
        }
        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
            return Some(buf);
        }
        let n = match timeout(HEADER_READ_TIMEOUT, stream.read(&mut chunk)).await {
            Ok(Ok(n)) => n,
            Ok(Err(_)) | Err(_) => return None,
        };
        if n == 0 {
            return None; // EOF before the head was complete
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Extract the value of the `Authorization` header from a raw request head.
/// Returns `None` when the header is absent.
fn extract_authorization(head: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(head);
    for line in text.lines() {
        if line.is_empty() {
            break; // end of the header section
        }
        let Some((name, value)) = line.split_once(':') else {
            continue; // request line or malformed line
        };
        if name.trim().eq_ignore_ascii_case("authorization") {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// Periodic stats emitter: at most one event per second per tunnel, and only
/// when bytes or the active-connection count changed since the last emit.
/// The final event after a stop is emitted by `stop()` itself.
async fn stats_loop(fwd: Arc<Forwarder>, mut stop_rx: watch::Receiver<bool>) {
    // Sentinel forces exactly one initial emit (UI can initialise to 0/0/0).
    let mut last_in = u64::MAX;
    let mut last_out = u64::MAX;
    let mut last_conn = u32::MAX;
    let mut ticker = tokio::time::interval(STATS_INTERVAL);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let (bi, bo, ca) = fwd.counters();
                if bi != last_in || bo != last_out || ca != last_conn {
                    fwd.emit_stats();
                    (last_in, last_out, last_conn) = (bi, bo, ca);
                }
            }
            _ = wait_for_stop(&mut stop_rx) => return,
        }
    }
}

/// Resolves as soon as a stop is requested (or the sender is dropped).
async fn wait_for_stop(stop_rx: &mut watch::Receiver<bool>) {
    while !*stop_rx.borrow_and_update() {
        if stop_rx.changed().await.is_err() {
            return; // sender gone == stopped
        }
    }
}

/// Decrements `conn_active` when dropped — including when the connection task
/// is aborted by `stop()`, because dropping a future drops its locals.
struct ConnGuard(Arc<AtomicU32>);

impl Drop for ConnGuard {
    fn drop(&mut self) {
        // saturating_sub via fetch_update: an in-flight decrement racing a
        // forced `store` must never wrap the counter around.
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_sub(1))
            });
    }
}

/// Wraps the client-side socket of a proxied connection: bytes read arrive
/// from the public side through the tunnel (bytes_in), bytes written travel
/// back out (bytes_out).
///
/// The counters use `SeqCst`, not `Relaxed`: the OS write inside `poll_write`
/// can deliver data (and the eventual FIN) to the peer before the following
/// counter store becomes visible on another core — on weakly-ordered ARM a
/// reader that observes EOF must still observe the full count.
struct CountingStream<S> {
    inner: S,
    bytes_in: Arc<AtomicU64>,
    bytes_out: Arc<AtomicU64>,
}

impl<S: AsyncRead + Unpin> AsyncRead for CountingStream<S> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let before = buf.filled().len();
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let n = (buf.filled().len() - before) as u64;
                if n > 0 {
                    this.bytes_in.fetch_add(n, Ordering::SeqCst);
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for CountingStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_write(cx, buf) {
            Poll::Ready(Ok(n)) => {
                if n > 0 {
                    this.bytes_out.fetch_add(n as u64, Ordering::SeqCst);
                }
                Poll::Ready(Ok(n))
            }
            other => other,
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Parse an IP or CIDR (e.g. `10.0.0.0/8`) allowlist entry for `ip`.
///
/// * empty allowlist = allow everything;
/// * `a.b.c.d` (no `/`) matches exactly that address (/32, /128);
/// * `addr/prefix` masks both sides — IPv4 and IPv6, family must match;
/// * an entry that fails to parse (`banana`, `10.0.0.0/33`, `300.1.2.3`, …)
///   never matches, so a typo can never accidentally widen access.
pub fn ip_allowed(ip: IpAddr, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return true;
    }
    allowlist.iter().any(|entry| cidr_entry_matches(ip, entry))
}

fn cidr_entry_matches(ip: IpAddr, entry: &str) -> bool {
    let entry = entry.trim();
    if entry.is_empty() {
        return false;
    }
    let (addr_part, prefix) = match entry.split_once('/') {
        Some((addr, bits)) => match bits.trim().parse::<u32>() {
            Ok(bits) => (addr, Some(bits)),
            Err(_) => return false, // bad prefix length
        },
        None => (entry, None),
    };
    let Ok(net) = addr_part.trim().parse::<IpAddr>() else {
        return false; // not an IP literal (hostnames are not supported)
    };
    let Some(prefix) = prefix else {
        return ip == net; // plain IP: exact match
    };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => {
            if prefix > 32 {
                return false;
            }
            mask_v4(prefix) & u32::from(a) == mask_v4(prefix) & u32::from(n)
        }
        (IpAddr::V6(a), IpAddr::V6(n)) => {
            if prefix > 128 {
                return false;
            }
            mask_v6(prefix) & u128::from(a) == mask_v6(prefix) & u128::from(n)
        }
        // IPv4 entry can never match an IPv6 address and vice versa.
        _ => false,
    }
}

fn mask_v4(prefix: u32) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn mask_v6(prefix: u32) -> u128 {
    if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    }
}

/// Check a Basic Auth header value against user + password.
///
/// Accepts the RFC 7235 shape `Basic <base64(user:pass)>` (scheme comparison
/// is case-insensitive; missing or unpadded `=` are tolerated). The comparison
/// itself is constant-time; only the *length* of the compared values leaks.
pub fn basic_auth_ok(header_value: &str, username: &str, password: &str) -> bool {
    let Some((_scheme, credentials)) = header_value.trim().split_once(' ') else {
        return false; // no "Basic <credentials>" shape at all
    };
    if !_scheme.eq_ignore_ascii_case("basic") {
        return false; // Bearer/other schemes are not ours to accept
    }
    let credentials = credentials.trim();
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(credentials)
        .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(credentials));
    let Ok(decoded) = decoded else {
        return false; // not valid base64
    };
    let Some(colon) = decoded.iter().position(|&b| b == b':') else {
        return false; // credentials must be `user:password`
    };
    constant_time_eq(&decoded[..colon], username.as_bytes())
        && constant_time_eq(&decoded[colon + 1..], password.as_bytes())
}

/// Bytes-wise constant-time equality. Length mismatch returns early — the
/// length of a username/password is not treated as a secret here.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Backend, TunnelAuth};

    fn cfg(tunnel_type: TunnelType, local_port: u16) -> TunnelConfig {
        TunnelConfig {
            id: "t1".into(),
            name: "t".into(),
            tunnel_type,
            backend: Backend::Cloudflare,
            local_host: "127.0.0.1".into(),
            local_port,
            auto_start: false,
            created_at: String::new(),
            server_id: None,
            subdomain: None,
            remote_port: None,
            auth: None,
            ip_allowlist: Vec::new(),
        }
    }

    fn auth_cfg(tunnel_type: TunnelType, local_port: u16, username: &str) -> TunnelConfig {
        let mut c = cfg(tunnel_type, local_port);
        c.auth = Some(TunnelAuth {
            kind: TunnelAuth::BASIC.into(),
            username: username.into(),
        });
        c
    }

    /// Plain TCP echo server; every accepted connection echoes until EOF.
    async fn echo_server() -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut r, mut w) = sock.into_split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        (port, handle)
    }

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s.as_bytes())
    }

    // --- ip_allowed ----------------------------------------------------------

    #[test]
    fn empty_allowlist_allows_everything() {
        assert!(ip_allowed("127.0.0.1".parse().unwrap(), &[]));
        assert!(ip_allowed("203.0.113.9".parse().unwrap(), &[]));
        assert!(ip_allowed("::1".parse().unwrap(), &[]));
    }

    #[test]
    fn single_ip_entry_matches_exactly() {
        let list = ["192.0.2.7".to_string()];
        assert!(ip_allowed("192.0.2.7".parse().unwrap(), &list));
        assert!(!ip_allowed("192.0.2.8".parse().unwrap(), &list));
        // A bare IPv6 entry matches exactly too.
        let v6 = ["2001:db8::1".to_string()];
        assert!(ip_allowed("2001:db8::1".parse().unwrap(), &v6));
        assert!(!ip_allowed("2001:db8::2".parse().unwrap(), &v6));
    }

    #[test]
    fn ipv4_cidr_masks_network_bits() {
        let list = ["10.0.0.0/8".to_string()];
        assert!(ip_allowed("10.1.2.3".parse().unwrap(), &list));
        assert!(ip_allowed("10.255.255.255".parse().unwrap(), &list));
        assert!(!ip_allowed("11.0.0.1".parse().unwrap(), &list));
        assert!(!ip_allowed("110.0.0.1".parse().unwrap(), &list));

        let exact = ["192.0.2.0/32".to_string()];
        assert!(ip_allowed("192.0.2.0".parse().unwrap(), &exact));
        assert!(!ip_allowed("192.0.2.1".parse().unwrap(), &exact));

        let everything = ["0.0.0.0/0".to_string()];
        assert!(ip_allowed("203.0.113.9".parse().unwrap(), &everything));
    }

    #[test]
    fn ipv6_cidr_masks_network_bits() {
        let list = ["2001:db8::/32".to_string()];
        assert!(ip_allowed("2001:db8:1::1".parse().unwrap(), &list));
        assert!(ip_allowed("2001:db8::".parse().unwrap(), &list));
        assert!(!ip_allowed("2001:db9::1".parse().unwrap(), &list));

        let loopback = ["::1/128".to_string()];
        assert!(ip_allowed("::1".parse().unwrap(), &loopback));
        assert!(!ip_allowed("::2".parse().unwrap(), &loopback));

        let everything = ["::/0".to_string()];
        assert!(ip_allowed("2001:db8::1".parse().unwrap(), &everything));
    }

    #[test]
    fn family_mismatch_never_matches() {
        let v4 = ["0.0.0.0/0".to_string()];
        assert!(!ip_allowed("::1".parse().unwrap(), &v4));
        let v6 = ["::/0".to_string()];
        assert!(!ip_allowed("192.0.2.1".parse().unwrap(), &v6));
    }

    #[test]
    fn invalid_entries_never_match_and_dont_break_valid_ones() {
        for bad in [
            "banana",
            "10.0.0.0/33",
            "10.0.0.0/-1",
            "10.0.0.0/abc",
            "300.1.2.3",
            "",
            "   ",
            "/8",
            "10.0.0.0/",
            "10.0.0.0/8/9",
        ] {
            let list = [bad.to_string()];
            assert!(
                !ip_allowed("10.0.0.1".parse().unwrap(), &list),
                "bad entry {bad:?} must never match"
            );
        }
        // A mix: the invalid entry is skipped, the valid one decides.
        let mixed = ["banana".to_string(), "10.0.0.0/24".to_string()];
        assert!(ip_allowed("10.0.0.77".parse().unwrap(), &mixed));
        assert!(!ip_allowed("10.0.1.77".parse().unwrap(), &mixed));
    }

    // --- basic_auth_ok -------------------------------------------------------

    #[test]
    fn basic_auth_accepts_correct_credentials() {
        assert!(basic_auth_ok(
            &format!("Basic {}", b64("alice:s3cret")),
            "alice",
            "s3cret"
        ));
        // Scheme is case-insensitive; extra spaces tolerated.
        assert!(basic_auth_ok(
            &format!("basic  {}", b64("alice:s3cret")),
            "alice",
            "s3cret"
        ));
        // Passwords may contain colons — split on the FIRST colon only.
        assert!(basic_auth_ok(
            &format!("Basic {}", b64("alice:pa:ss:wd")),
            "alice",
            "pa:ss:wd"
        ));
        // Empty password is still a credential the user may have set.
        assert!(basic_auth_ok(
            &format!("Basic {}", b64("alice:")),
            "alice",
            ""
        ));
        // Unpadded base64 (some clients omit `=`).
        let unpadded = base64::engine::general_purpose::STANDARD
            .encode("alice:s3cret")
            .trim_end_matches('=')
            .to_string();
        assert!(basic_auth_ok(
            &format!("Basic {unpadded}"),
            "alice",
            "s3cret"
        ));
    }

    #[test]
    fn basic_auth_rejects_bad_credentials_and_shapes() {
        // Wrong password / wrong username.
        assert!(!basic_auth_ok(
            &format!("Basic {}", b64("alice:wrong")),
            "alice",
            "s3cret"
        ));
        assert!(!basic_auth_ok(
            &format!("Basic {}", b64("bob:s3cret")),
            "alice",
            "s3cret"
        ));
        // Missing/mistyped scheme ("Bearer" and friends are not Basic).
        assert!(!basic_auth_ok("dXNlcjpwYXNz", "user", "pass"));
        assert!(!basic_auth_ok(
            &format!("Bearer {}", b64("alice:s3cret")),
            "alice",
            "s3cret"
        ));
        assert!(!basic_auth_ok("BasicdXNlcjpwYXNz", "user", "pass"));
        // Not base64 / no colon inside / empty header.
        assert!(!basic_auth_ok("Basic !!!!", "alice", "s3cret"));
        assert!(!basic_auth_ok(
            &format!("Basic {}", b64("alice-no-colon")),
            "alice",
            "s3cret"
        ));
        assert!(!basic_auth_ok("", "alice", "s3cret"));
    }

    // --- end-to-end forwarder behaviour ---------------------------------------

    #[tokio::test]
    async fn forwarder_proxies_and_counts_without_auth() {
        let (service_port, _echo) = echo_server().await;
        let (fwd, port) = Forwarder::spawn(None, cfg(TunnelType::Http, service_port), None)
            .await
            .unwrap();
        assert!(fwd.upstream_matches("127.0.0.1", service_port));
        assert_eq!(fwd.port(), port);

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"hello pier").await.unwrap();
        let mut buf = [0u8; 10];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hello pier");

        // The round trip is fully counted (in = towards the service).
        assert!(fwd.bytes_in() >= 10, "bytes_in = {}", fwd.bytes_in());
        assert!(fwd.bytes_out() >= 10, "bytes_out = {}", fwd.bytes_out());

        // The connection winds down and conn_active returns to zero.
        drop(client);
        for _ in 0..100 {
            if fwd.counters().2 == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(fwd.counters().2, 0, "conn_active must drain to 0");

        // stop() is idempotent and releases the listening port.
        fwd.stop().await;
        fwd.stop().await;
        assert!(fwd.is_stopped());
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
    }

    #[tokio::test]
    async fn forwarder_enforces_basic_auth_and_forwards_original_head() {
        let (service_port, _echo) = echo_server().await;
        let (fwd, port) = Forwarder::spawn(
            None,
            auth_cfg(TunnelType::Http, service_port, "alice"),
            Some("s3cret".into()),
        )
        .await
        .unwrap();
        let mut buf = vec![0u8; 512];

        // (1) No Authorization header at all -> 401.
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client
            .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        let n = client.read(&mut buf).await.unwrap();
        assert!(
            buf[..n].starts_with(b"HTTP/1.1 401"),
            "{}",
            String::from_utf8_lossy(&buf[..n])
        );
        assert!(buf[..n].windows(b"Basic".len()).any(|w| w == b"Basic"));
        drop(client);

        // (2) Wrong password -> 401.
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let bad = format!(
            "GET / HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {}\r\n\r\n",
            b64("alice:wrong")
        );
        client.write_all(bad.as_bytes()).await.unwrap();
        let n = client.read(&mut buf).await.unwrap();
        assert!(buf[..n].starts_with(b"HTTP/1.1 401"));
        drop(client);

        // (3) Correct credentials -> proxied, and the bytes buffered during the
        // handshake (head + pipelined body) are stitched back in front of the
        // stream: the local service must see the ORIGINAL request, byte for byte.
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let good = format!(
            "GET / HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {}\r\n\r\nBODY",
            b64("alice:s3cret")
        );
        client.write_all(good.as_bytes()).await.unwrap();
        client.shutdown().await.unwrap(); // half-close: let the echo finish
        let mut echoed = Vec::new();
        client.read_to_end(&mut echoed).await.unwrap();
        assert_eq!(echoed, good.as_bytes());
        assert!(fwd.bytes_in() >= good.len() as u64);
        assert!(fwd.bytes_out() >= good.len() as u64);
        fwd.stop().await;
    }

    #[tokio::test]
    async fn forwarder_allowlist_rejects_ips_outside_the_list() {
        let (service_port, _echo) = echo_server().await;
        let mut c = cfg(TunnelType::Tcp, service_port);
        // Tunnel clients always dial from loopback, so an allowlist WITHOUT
        // loopback exercises the deny path deterministically.
        c.ip_allowlist = vec!["10.0.0.0/8".into()];
        let (fwd, port) = Forwarder::spawn(None, c, None).await.unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let _ = client.write_all(b"nope").await; // may hit the closed socket
        let mut buf = [0u8; 4];
        let res = client.read(&mut buf).await;
        assert!(
            matches!(res, Ok(0) | Err(_)),
            "expected immediate close, got {res:?}"
        );
        // Nothing was proxied, nothing was counted.
        assert_eq!(fwd.bytes_in(), 0);
        assert_eq!(fwd.bytes_out(), 0);
        fwd.stop().await;
    }

    #[tokio::test]
    async fn forwarder_stop_drops_established_connections() {
        // A service that accepts but never answers: the proxied connection
        // would stay open forever unless stop() tears it down.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let service_port = listener.local_addr().unwrap().port();
        let _silent = tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                drop(sock);
            }
        });
        let (fwd, port) = Forwarder::spawn(None, cfg(TunnelType::Tcp, service_port), None)
            .await
            .unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"ping").await.unwrap();
        let started = std::time::Instant::now();
        fwd.stop().await;
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "stop must be prompt"
        );

        // The proxied connection is closed by the stop: EOF or reset.
        let mut buf = [0u8; 4];
        match timeout(Duration::from_secs(1), client.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {}
            other => panic!("connection survived stop(): {other:?}"),
        }
    }

    #[tokio::test]
    async fn forwarder_tcp_type_ignores_basic_auth() {
        // Documented limitation: a TCP tunnel carries an opaque protocol, so
        // the forwarder cannot intercept headers — auth config is ignored and
        // bytes pass through untouched.
        let (service_port, _echo) = echo_server().await;
        let (fwd, port) = Forwarder::spawn(
            None,
            auth_cfg(TunnelType::Tcp, service_port, "alice"),
            Some("pw".into()),
        )
        .await
        .unwrap();

        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"RAW-PROTOCOL").await.unwrap();
        let mut buf = [0u8; 12];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"RAW-PROTOCOL");
        fwd.stop().await;
    }

    #[tokio::test]
    async fn forwarder_closes_when_local_service_is_unreachable() {
        // Bind then immediately drop: nothing listens on this port anymore.
        let dead = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_port = dead.local_addr().unwrap().port();
        drop(dead);
        // Give the OS a beat to actually release the socket.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let (fwd, port) = Forwarder::spawn(None, cfg(TunnelType::Http, dead_port), None)
            .await
            .unwrap();
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        client.write_all(b"GET / HTTP/1.1\r\n\r\n").await.unwrap();
        let mut buf = [0u8; 4];
        // The forwarder must close the client side promptly, not hang.
        match timeout(Duration::from_secs(2), client.read(&mut buf)).await {
            Ok(Ok(0)) | Ok(Err(_)) => {}
            other => panic!("client connection not closed on unreachable service: {other:?}"),
        }
        assert_eq!(fwd.bytes_in(), 0);
        fwd.stop().await;
    }

    // --- real-network end-to-end ---------------------------------------------

    /// A real HTTP server that answers every request with a marker body.
    async fn http_marker_server(mark: String) -> (u16, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = tokio::spawn(async move {
            while let Ok((sock, _)) = listener.accept().await {
                let body = format!("PIER-E2E-MARK-{mark}");
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                tokio::spawn(async move {
                    let mut sock = sock;
                    // Drain the request head so the client sees a clean close.
                    let mut buf = [0u8; 4096];
                    let _ = tokio::io::AsyncReadExt::read(&mut sock, &mut buf).await;
                    let _ = tokio::io::AsyncWriteExt::write_all(&mut sock, resp.as_bytes()).await;
                });
            }
        });
        (port, handle)
    }

    /// Fetch (or reuse) a cloudflared binary under /tmp for e2e testing.
    async fn ensure_cloudflared() -> std::path::PathBuf {
        let dir = std::path::PathBuf::from("/tmp/pier-e2e-cloudflared");
        let bin = dir.join("cloudflared");
        if bin.is_file() {
            return bin;
        }
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let url = if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-arm64.tgz"
            } else {
                "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-darwin-amd64.tgz"
            }
        } else if cfg!(target_os = "windows") {
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-windows-amd64.exe"
        } else if cfg!(target_arch = "aarch64") {
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-arm64.tgz"
        } else {
            "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-amd64.tgz"
        };
        let resp = reqwest::Client::new()
            .get(url)
            .header("User-Agent", concat!("Pier/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .expect("download cloudflared");
        let bytes = resp.error_for_status().unwrap().bytes().await.unwrap();
        if url.ends_with(".tgz") {
            let gz = flate2::read::GzDecoder::new(&bytes[..]);
            let mut ar = tar::Archive::new(gz);
            ar.unpack(&dir).expect("unpack cloudflared archive");
        } else {
            tokio::fs::write(&bin, &bytes).await.unwrap();
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755))
                .await
                .unwrap();
        }
        assert!(bin.is_file(), "cloudflared was not extracted");
        bin
    }

    /// REAL-NETWORK end-to-end: forwarder -> cloudflared quick tunnel ->
    /// public trycloudflare.com URL -> response from the local service.
    /// Not part of CI; run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "real network: downloads cloudflared and opens a public quick tunnel"]
    async fn real_cloudflared_quick_tunnel_end_to_end() {
        use tokio::io::AsyncBufReadExt;

        let mark = uuid::Uuid::new_v4().simple().to_string()[..12].to_string();
        let (upstream_port, server) = http_marker_server(mark.clone()).await;

        let cfg = cfg(TunnelType::Http, upstream_port);
        let (fwd, fwd_port) = Forwarder::spawn(None, cfg, None).await.unwrap();

        let cf_bin = ensure_cloudflared().await;
        let mut child = tokio::process::Command::new(&cf_bin)
            .args([
                "tunnel",
                "--url",
                &format!("http://127.0.0.1:{fwd_port}"),
                // Same rationale as providers::build_args: QUIC is blocked on
                // some networks and cloudflared's auto mode does not degrade.
                "--protocol",
                "http2",
                "--no-autoupdate",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn cloudflared");
        let stderr = child.stderr.take().unwrap();
        let mut lines = tokio::io::BufReader::new(stderr).lines();

        // Wait (up to 45s) for the quick-tunnel URL on cloudflared's stderr.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        let mut public_url = None;
        while tokio::time::Instant::now() < deadline {
            match timeout(Duration::from_secs(5), lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    if let Some(url) = crate::providers::parse_public_endpoint(
                        crate::models::Backend::Cloudflare,
                        &line,
                    ) {
                        public_url = Some(url);
                        break;
                    }
                }
                _ => continue,
            }
        }
        let public_url = public_url.expect("cloudflared did not report a quick tunnel URL");
        println!("public URL: {public_url}");

        // The edge needs a few seconds to become reachable; retry.
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap();
        let mut last_err = String::new();
        let mut body = String::new();
        for _ in 0..10 {
            match client.get(&public_url).send().await {
                Ok(resp) => match resp.error_for_status() {
                    Ok(r) => {
                        body = r.text().await.unwrap_or_default();
                        if body.contains(&format!("PIER-E2E-MARK-{mark}")) {
                            break;
                        }
                        last_err = format!("unexpected body: {body}");
                    }
                    Err(e) => last_err = e.to_string(),
                },
                Err(e) => last_err = e.to_string(),
            }
            tokio::time::sleep(Duration::from_secs(4)).await;
        }
        assert!(
            body.contains(&format!("PIER-E2E-MARK-{mark}")),
            "public round-trip failed: {last_err}"
        );

        let stats_in = fwd.bytes_in();
        let stats_out = fwd.bytes_out();
        println!("forwarded bytes: in={stats_in} out={stats_out}");
        assert!(stats_in > 0, "forwarder counted no inbound bytes");
        assert!(stats_out > 0, "forwarder counted no outbound bytes");

        let _ = child.kill().await;
        server.abort();
        fwd.stop().await;
    }
}
