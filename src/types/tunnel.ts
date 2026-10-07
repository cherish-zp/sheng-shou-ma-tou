// Tunnel domain types — the single source of truth for the
// frontend <-> Rust contract. Keep in sync with src-tauri/src/models.rs.

export type TunnelType = "http" | "tcp";

export type Backend = "cloudflare" | "bore" | "frp" | "cloudflareNamed";

export type TunnelStatus =
  | "stopped"
  | "starting"
  | "running"
  | "reconnecting"
  | "error";

/** HTTP Basic Auth for a tunnel; the password lives in the OS keychain. */
export interface TunnelAuth {
  /** Currently only "basic". */
  kind: string;
  username: string;
}

export interface TunnelConfig {
  id: string;
  name: string;
  tunnelType: TunnelType;
  backend: Backend;
  localHost: string;
  localPort: number;
  autoStart: boolean;
  createdAt: string;
  /** Frp tunnels only: which Pier-managed server this tunnel runs through. */
  serverId?: string | null;
  /** Frp HTTP tunnels only: subdomain under the server's subdomainHost. */
  subdomain?: string | null;
  /** Frp TCP tunnels only: public port allocated on the server. */
  remotePort?: number | null;
  /** HTTP Basic Auth for this tunnel; password set via setTunnelAuth. */
  auth?: TunnelAuth | null;
  /** CloudflareNamed only: the remote tunnel object id (UUID). */
  cfTunnelId?: string | null;
  /** CloudflareNamed only: full fixed hostname (`mac.example.com`). */
  cfHostname?: string | null;
  /** Only allow these IPs/CIDRs; empty = allow all. */
  ipAllowlist?: string[];
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
  /** Optional: only reported once the backend manages frpc locally (M2+). */
  frp?: BinaryInfo;
}

// ---------------------------------------------------------------------------
// M2: servers (self-hosted frps), import, diagnosis
// ---------------------------------------------------------------------------

export type AuthKind = "password" | "keypath";

export interface ServerConfig {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authKind: AuthKind;
  frpsBindPort: number;
  frpsVhostHttpPort: number;
  frpsVhostHttpsPort: number;
  frpsDashboardPort: number;
  subdomainHost: string | null;
  deployed: boolean;
  frpsVersion: string | null;
  createdAt: string;
}

/** Input for creating a server; `secret` is moved into the OS keychain. */
export interface ServerInput {
  name: string;
  host: string;
  port: number;
  username: string;
  authKind: AuthKind;
  secret: string;
  frpsBindPort?: number;
  frpsVhostHttpPort?: number;
  frpsVhostHttpsPort?: number;
  frpsDashboardPort?: number;
  subdomainHost?: string | null;
}

export interface ServerStatus {
  serverId: string;
  reachable: boolean;
  frpsRunning: boolean;
  frpsVersion: string | null;
  detail: string | null;
}

export type StepStatus = "running" | "ok" | "fail" | "skip";

/** Event "deploy://progress" payload — one deployment step update. */
export interface DeployProgressEvent {
  serverId: string;
  step: string;
  status: StepStatus;
  message: string | null;
}

export interface DeployResult {
  serverId: string;
  ok: boolean;
  error: string | null;
  /** Random frps token generated during deployment. */
  token: string | null;
}

export type DiagnosisLevel = "info" | "warn" | "error";

/**
 * `code` is a stable slug; the UI renders title/suggestions from
 * `diagnosis.<code>.*` i18n keys. Expected codes: tokenMismatch, authFailed,
 * versionMismatch, portConflict, connectionRefused, dnsFailed,
 * localServiceDown, binaryMissing, remoteServerUnreachable, genericError,
 * allHealthy.
 */
export interface Diagnosis {
  tunnelId: string;
  code: string;
  level: DiagnosisLevel;
  detail: string;
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

/** Event "tunnel://stats" payload — throttled to 1/s per running tunnel. */
export interface TunnelStats {
  tunnelId: string;
  bytesIn: number;
  bytesOut: number;
  connActive: number;
}

// ---------------------------------------------------------------------------
// Cloudflare Named Tunnel (v0.2.0)
// ---------------------------------------------------------------------------

export interface CfAccount {
  id: string;
  name: string;
}

export interface CfZone {
  id: string;
  name: string;
  accountId: string;
}

/** Input for provisioning a fixed-hostname tunnel in one shot. */
export interface CfProvisionInput {
  /** Cloudflare API token (Tunnel:Edit + DNS:Edit + Zone:Read). */
  token: string;
  zoneId: string;
  /** Single label, e.g. `mac` for `mac.example.com`. */
  subdomain: string;
  localHost: string;
  localPort: number;
  autoStart: boolean;
}
