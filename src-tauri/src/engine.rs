// Tunnel engine — per-tunnel process supervision and the status state machine.
//
// Lifecycle of one tunnel (M1):
//
//   start -> Starting -> (public endpoint parsed from process output) -> Running
//         -> process exits unexpectedly -> Reconnecting
//            -> exponential backoff (1s, 2s, 4s ... capped at 30s, unlimited)
//            -> full start attempt again; backoff resets after an attempt that
//               had reached Running
//   user stop at ANY point -> Stopped (retry/backoff loop exits immediately)
//
// Design notes:
// * "One run attempt" (`run_attempt`) is a reentrant function so retries go
//   through the exact same path as the first start.
// * The child is killed through a `oneshot` kill switch owned by the attempt
//   task: kill first, then `wait()` to reap (no zombies).
// * Orphan protection: every child is spawned with `kill_on_drop(true)`, so a
//   dropped child (e.g. tokio runtime teardown at app exit) is killed instead
//   of leaking. `Engine::stop_all()` is provided for the integrator to wire
//   into a `RunEvent::Exit` hook later (lib.rs is outside this milestone).
// * Traffic counters (bytes_in/bytes_out) stay 0 — neither cloudflared nor
//   bore reports per-tunnel traffic on stdout/stderr; the fields are kept for
//   the UI contract (frontend renders "-").

use std::collections::{HashMap, VecDeque};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncBufReadExt;
use tokio::sync::{oneshot, Notify};

use crate::binman;
use crate::models::{Backend, TunnelConfig, TunnelState, TunnelStatus};
use crate::providers;

/// Emitted on every status transition. Payload: `{ "state": TunnelState }`.
pub const TUNNEL_STATE_EVENT: &str = "tunnel://state";
/// Emitted per tunnel-process output line. Payload: `{ tunnelId, level, line, ts }`.
pub const TUNNEL_LOG_EVENT: &str = "tunnel://log";

const BASE_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
/// How long `stop()` waits for the supervisor to confirm Stopped.
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(3);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);
const ERROR_LINE_MAX_LEN: usize = 300;
/// Ring-buffer cap for per-tunnel log lines kept for the diagnostics engine.
const LOG_RING_CAP: usize = 200;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelStateEvent {
    state: TunnelState,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelLogEvent {
    tunnel_id: String,
    level: &'static str,
    line: String,
    ts: String,
}

pub struct Engine {
    app: AppHandle,
    handles: Mutex<HashMap<String, Arc<TunnelHandle>>>,
}

struct TunnelHandle {
    state: RwLock<TunnelState>,
    /// Set when the user asks for a stop; checked by the supervisor and every
    /// attempt so the retry loop unwinds immediately.
    stop_flag: AtomicBool,
    /// Guards against double-spawning the supervisor for one tunnel.
    supervising: AtomicBool,
    /// Kill switch for the currently running attempt (`Some` while a child
    /// process is alive). `stop()` sends `()` on it.
    kill_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// Wakes the backoff sleep so a stop interrupts it instantly.
    notify: Notify,
    /// OS pid of the current child (0 when none). Informational.
    pid: AtomicU32,
    /// Ring buffer of the most recent output lines (ANSI stripped, oldest
    /// first), consumed by the diagnostics engine via `Engine::recent_logs`.
    logs: Mutex<VecDeque<String>>,
}

impl TunnelHandle {
    fn new(id: &str) -> Self {
        Self {
            state: RwLock::new(TunnelState::new(id)),
            stop_flag: AtomicBool::new(false),
            supervising: AtomicBool::new(false),
            kill_tx: Mutex::new(None),
            notify: Notify::new(),
            pid: AtomicU32::new(0),
            logs: Mutex::new(VecDeque::with_capacity(LOG_RING_CAP)),
        }
    }

    /// Append one stripped output line to the ring buffer, dropping the
    /// oldest when full.
    fn push_log(&self, line: &str) {
        let mut logs = self.logs.lock().unwrap_or_else(|e| e.into_inner());
        if logs.len() >= LOG_RING_CAP {
            logs.pop_front();
        }
        logs.push_back(line.to_string());
    }

    fn snapshot(&self) -> TunnelState {
        self.state
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn status(&self) -> TunnelStatus {
        self.state.read().unwrap_or_else(|e| e.into_inner()).status
    }

    fn update(&self, f: impl FnOnce(&mut TunnelState)) -> TunnelState {
        let mut state = self.state.write().unwrap_or_else(|e| e.into_inner());
        f(&mut state);
        state.clone()
    }
}

fn clear_to_stopped(s: &mut TunnelState) {
    s.status = TunnelStatus::Stopped;
    s.public_url = None;
    s.error = None;
    s.started_at = None;
}

fn truncate_line(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\u{2026}", &s[..end])
}

struct AttemptOutcome {
    /// The attempt had reached Running (a public endpoint was parsed).
    had_url: bool,
    /// Unrecoverable failure (binary missing / spawn failed) — the supervisor
    /// sets Error and stops retrying.
    fatal: Option<String>,
    /// Reason recorded in state.error when the attempt ends abnormally.
    error: Option<String>,
}

impl Engine {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Current state snapshot for a tunnel that has a live handle in this
    /// session. `None` when the tunnel was never started here.
    pub fn snapshot(&self, id: &str) -> Option<TunnelState> {
        self.handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|h| h.snapshot())
    }

    /// Most recent `max` log lines for a tunnel (oldest first), for the
    /// diagnostics engine. Lines are the ANSI-stripped form; returns an empty
    /// Vec when the tunnel has no live handle in this session.
    pub fn recent_logs(&self, id: &str, max: usize) -> Vec<String> {
        let handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        let Some(handle) = handles.get(id) else {
            return Vec::new();
        };
        let logs = handle.logs.lock().unwrap_or_else(|e| e.into_inner());
        let skip = logs.len().saturating_sub(max);
        logs.iter().skip(skip).cloned().collect()
    }

    /// Start (or resume retrying) a tunnel. Returns the state right after
    /// kickoff (usually Starting). No-op when already active.
    pub async fn start(self: Arc<Self>, cfg: TunnelConfig) -> Result<TunnelState, String> {
        let handle = self.get_or_create(&cfg.id);
        if matches!(
            handle.status(),
            TunnelStatus::Starting | TunnelStatus::Running | TunnelStatus::Reconnecting
        ) {
            return Ok(handle.snapshot());
        }
        if handle.supervising.swap(true, Ordering::SeqCst) {
            return Ok(handle.snapshot());
        }
        // Fail fast (and surface the error to the caller) when the binary is
        // missing; run_attempt re-resolves on every retry.
        if let Err(e) = binman::resolve(&self.app, cfg.backend) {
            handle.supervising.store(false, Ordering::SeqCst);
            let st = handle.update(|s| {
                s.status = TunnelStatus::Error;
                s.error = Some(e.clone());
                s.public_url = None;
                s.started_at = None;
            });
            self.emit_state(&st);
            return Err(e);
        }
        handle.stop_flag.store(false, Ordering::SeqCst);
        let st = handle.update(|s| {
            s.status = TunnelStatus::Starting;
            s.public_url = None;
            s.error = None;
            s.started_at = None;
            s.bytes_in = 0;
            s.bytes_out = 0;
        });
        self.emit_state(&st);

        let engine = self.clone();
        let h = handle.clone();
        tokio::spawn(async move {
            supervise(engine, h, cfg).await;
        });
        Ok(handle.snapshot())
    }

    /// Stop a tunnel: set the stop flag, kill the child (kill first, reap
    /// after), wake the backoff sleep, then wait briefly for Stopped.
    pub async fn stop(&self, id: &str) -> Result<TunnelState, String> {
        let handle = self
            .handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| "tunnel is not running".to_string())?;

        match handle.status() {
            TunnelStatus::Stopped => return Ok(handle.snapshot()),
            TunnelStatus::Error => {
                // Nothing to kill; treat stop as acknowledging the error.
                let st = handle.update(clear_to_stopped);
                self.emit_state(&st);
                return Ok(st);
            }
            _ => {}
        }

        handle.stop_flag.store(true, Ordering::SeqCst);
        if let Some(tx) = handle
            .kill_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
        handle.notify.notify_one();

        let deadline = tokio::time::Instant::now() + STOP_WAIT_TIMEOUT;
        while tokio::time::Instant::now() < deadline {
            if handle.status() == TunnelStatus::Stopped {
                break;
            }
            tokio::time::sleep(STOP_POLL_INTERVAL).await;
        }
        Ok(handle.snapshot())
    }

    /// Stop every running tunnel. Intended to be wired to app exit by the
    /// integrator (lib.rs is out of scope for this milestone); unused for now.
    #[allow(dead_code)]
    pub async fn stop_all(&self) {
        let ids: Vec<String> = self
            .handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        for id in ids {
            let _ = self.stop(&id).await;
        }
    }

    /// Drop the runtime bookkeeping for a tunnel (after delete).
    pub fn remove_handle(&self, id: &str) {
        self.handles
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
    }

    fn get_or_create(&self, id: &str) -> Arc<TunnelHandle> {
        let mut handles = self.handles.lock().unwrap_or_else(|e| e.into_inner());
        handles
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(TunnelHandle::new(id)))
            .clone()
    }

    fn emit_state(&self, st: &TunnelState) {
        let _ = self.app.emit(
            TUNNEL_STATE_EVENT,
            TunnelStateEvent { state: st.clone() },
        );
    }

    fn emit_log(&self, tunnel_id: &str, level: &'static str, line: &str) {
        let _ = self.app.emit(
            TUNNEL_LOG_EVENT,
            TunnelLogEvent {
                tunnel_id: tunnel_id.to_string(),
                level,
                line: line.to_string(),
                ts: chrono::Utc::now().to_rfc3339(),
            },
        );
    }
}

/// Supervisor loop: run attempts until the user stops; on abnormal exit go to
/// Reconnecting and retry with exponential backoff (1s..30s, unlimited).
async fn supervise(engine: Arc<Engine>, handle: Arc<TunnelHandle>, cfg: TunnelConfig) {
    let mut backoff = BASE_BACKOFF;
    loop {
        let outcome = run_attempt(&engine, &handle, &cfg).await;

        if handle.stop_flag.load(Ordering::SeqCst) {
            let st = handle.update(clear_to_stopped);
            engine.emit_state(&st);
            break;
        }
        if let Some(fatal) = outcome.fatal {
            let msg = truncate_line(&fatal, ERROR_LINE_MAX_LEN);
            let st = handle.update(|s| {
                s.status = TunnelStatus::Error;
                s.error = Some(msg);
                s.public_url = None;
                s.started_at = None;
            });
            engine.emit_state(&st);
            break;
        }

        // Abnormal disconnect (or the process died before becoming ready).
        let err = outcome.error.as_ref().map(|e| truncate_line(e, ERROR_LINE_MAX_LEN));
        let st = handle.update(|s| {
            s.status = TunnelStatus::Reconnecting;
            s.public_url = None;
            s.error = err;
        });
        engine.emit_state(&st);

        if !wait_backoff(backoff, &handle).await {
            let st = handle.update(clear_to_stopped);
            engine.emit_state(&st);
            break;
        }
        if outcome.had_url {
            backoff = BASE_BACKOFF;
        } else {
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }
    handle.supervising.store(false, Ordering::SeqCst);
}

/// Backoff sleep that is interrupted immediately by a stop request.
/// Returns true when the retry loop should continue.
async fn wait_backoff(delay: Duration, handle: &TunnelHandle) -> bool {
    if handle.stop_flag.load(Ordering::SeqCst) {
        return false;
    }
    tokio::select! {
        _ = tokio::time::sleep(delay) => !handle.stop_flag.load(Ordering::SeqCst),
        _ = handle.notify.notified() => !handle.stop_flag.load(Ordering::SeqCst),
    }
}

/// One full run attempt: resolve binary -> spawn -> stream/parse output until
/// the process exits or the user stops it.
async fn run_attempt(
    engine: &Engine,
    handle: &TunnelHandle,
    cfg: &TunnelConfig,
) -> AttemptOutcome {
    if handle.stop_flag.load(Ordering::SeqCst) {
        return AttemptOutcome { had_url: false, fatal: None, error: None };
    }
    let binary = match binman::resolve(&engine.app, cfg.backend) {
        Ok(path) => path,
        Err(e) => return AttemptOutcome { had_url: false, fatal: Some(e), error: None },
    };
    if handle.stop_flag.load(Ordering::SeqCst) {
        return AttemptOutcome { had_url: false, fatal: None, error: None };
    }

    // Frp needs a generated config file and never prints its own public
    // endpoint, so both the command and the expected public URL are prepared
    // HERE, once per attempt: a rotated frps token or an edited server is
    // picked up on every retry.
    let (mut cmd, expected_url) = match cfg.backend {
        Backend::Frp => {
            let launch = match providers::prepare_frpc_config(&engine.app, cfg).await {
                Ok(launch) => launch,
                // Unbound server / missing token / unwritable cache: a
                // configuration failure the user must fix, so surface it as
                // Error instead of retrying forever.
                Err(e) => {
                    return AttemptOutcome { had_url: false, fatal: Some(e), error: None };
                }
            };
            (
                providers::build_frpc_command(&binary, &launch.config_path),
                launch.public_url,
            )
        }
        _ => (providers::build_command(cfg, &binary), None),
    };
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // Do not flash a console window on Windows.
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return AttemptOutcome {
                had_url: false,
                fatal: Some(format!("failed to start {}: {e}", binary.display())),
                error: None,
            }
        }
    };

    let pid = child.id().unwrap_or(0);
    handle.pid.store(pid, Ordering::SeqCst);

    // Install the kill switch for this attempt.
    let (kill_tx, mut kill_rx) = oneshot::channel::<()>();
    *handle.kill_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(kill_tx);
    if handle.stop_flag.load(Ordering::SeqCst) {
        // Stop raced in between; kill right away and unwind.
        *handle.kill_tx.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let _ = child.start_kill();
        let _ = child.wait().await;
        handle.pid.store(0, Ordering::SeqCst);
        return AttemptOutcome { had_url: false, fatal: None, error: None };
    }

    let st = handle.update(|s| {
        s.status = TunnelStatus::Starting;
        s.public_url = None;
        s.error = None;
        s.started_at = None;
    });
    engine.emit_state(&st);

    // Merge stdout+stderr through one channel, line by line.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    if let Some(out) = child.stdout.take() {
        tokio::spawn(pump_lines(out, tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(pump_lines(err, tx.clone()));
    }
    drop(tx);

    let mut public_url: Option<String> = None;
    let mut last_error: Option<String> = None;
    let mut killed = false;

    loop {
        tokio::select! {
            maybe = rx.recv() => {
                let Some(raw) = maybe else { break };
                let line = raw.trim_end().to_string();
                if line.is_empty() {
                    continue;
                }
                // Keep the ANSI-stripped form in the diagnostics ring buffer.
                handle.push_log(&providers::strip_ansi(&line));
                let level = providers::classify_level(cfg.backend, &line);
                engine.emit_log(&cfg.id, level, &line);
                if level == "error" {
                    // Transient cloudflared errors are common while it dials
                    // the edge; remember the last one but keep running.
                    if handle.status() != TunnelStatus::Running {
                        let short = truncate_line(&line, ERROR_LINE_MAX_LEN);
                        handle.update(|s| s.error = Some(short));
                    }
                    last_error = Some(line.clone());
                }
                if public_url.is_none() {
                    let mut hit = providers::parse_public_endpoint(cfg.backend, &line);
                    // frpc never prints its public endpoint; bind the URL
                    // computed at config-prep time once frpc confirms the
                    // proxy started forwarding.
                    if hit.is_none()
                        && cfg.backend == Backend::Frp
                        && providers::frpc_proxy_started(&line)
                    {
                        hit = expected_url.clone();
                    }
                    if let Some(url) = hit {
                        public_url = Some(url.clone());
                        let st = handle.update(|s| {
                            s.status = TunnelStatus::Running;
                            s.public_url = Some(url);
                            s.error = None;
                            s.started_at = Some(chrono::Utc::now().to_rfc3339());
                        });
                        engine.emit_state(&st);
                    }
                }
                // frpc keeps running even when the server rejected the proxy
                // (name/port conflict: "start error: ..."); that attempt can
                // never become Running, so kill it and let the supervisor
                // reconnect with a freshly prepared config.
                if cfg.backend == Backend::Frp
                    && public_url.is_none()
                    && providers::frpc_start_error(&line)
                {
                    last_error = Some(line.clone());
                    break;
                }
            }
            _ = &mut kill_rx => {
                // User stop: kill first, then reap so nothing lingers.
                killed = true;
                let _ = child.kill().await;
                let _ = child.wait().await;
                break;
            }
        }
    }

    let pid_for_note = handle.pid.load(Ordering::SeqCst);
    handle.pid.store(0, Ordering::SeqCst);
    *handle.kill_tx.lock().unwrap_or_else(|e| e.into_inner()) = None;

    let mut exit_note: Option<String> = None;
    if !killed {
        // The readers hit EOF, so the child is gone (or daemonized its stdio);
        // make sure it is dead, then reap it.
        let _ = child.start_kill();
        match child.wait().await {
            Ok(status) => {
                exit_note =
                    Some(format!("tunnel process (pid {pid_for_note}) exited ({status})"))
            }
            Err(e) => exit_note = Some(format!("tunnel wait failed: {e}")),
        }
    }

    AttemptOutcome {
        had_url: public_url.is_some(),
        fatal: None,
        error: last_error
            .or(exit_note)
            .or_else(|| Some("tunnel process exited unexpectedly".to_string())),
    }
}

/// Forward one output stream to the shared line channel until EOF.
async fn pump_lines<R>(stream: R, tx: tokio::sync::mpsc::UnboundedSender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut reader = tokio::io::BufReader::new(stream);
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                if tx.send(buf.clone()).is_err() {
                    break;
                }
            }
        }
    }
}
