// Tunnel domain types — the single source of truth for the
// frontend <-> Rust contract. Keep in sync with src-tauri/src/models.rs.

export type TunnelType = "http" | "tcp";

export type Backend = "cloudflare" | "bore";

export type TunnelStatus =
  | "stopped"
  | "starting"
  | "running"
  | "reconnecting"
  | "error";

export interface TunnelConfig {
  id: string;
  name: string;
  tunnelType: TunnelType;
  backend: Backend;
  localHost: string;
  localPort: number;
  autoStart: boolean;
  createdAt: string;
}

export interface TunnelState {
  id: string;
  status: TunnelStatus;
  publicUrl: string | null;
  error: string | null;
  startedAt: string | null;
  bytesIn: number;
  bytesOut: number;
}

export interface BinaryInfo {
  backend: Backend;
  version: string | null;
  path: string | null;
  installed: boolean;
}

export interface BinaryStatus {
  cloudflare: BinaryInfo;
  bore: BinaryInfo;
}

/** Event "tunnel://state" payload — emitted on every status transition. */
export interface TunnelStateEvent {
  state: TunnelState;
}

/** Event "tunnel://log" payload — one log line from the tunnel process. */
export interface TunnelLogEvent {
  tunnelId: string;
  level: "info" | "warn" | "error";
  line: string;
  ts: string;
}
