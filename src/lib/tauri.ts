// Typed wrappers around Tauri invoke/listen. The command names and payload
// shapes MUST match src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Backend,
  BinaryStatus,
  DeployProgressEvent,
  DeployResult,
  Diagnosis,
  ServerConfig,
  ServerInput,
  ServerStatus,
  TunnelConfig,
  TunnelLogEvent,
  TunnelState,
  TunnelStateEvent,
} from "@/types/tunnel";

export const TUNNEL_STATE_EVENT = "tunnel://state";
export const TUNNEL_LOG_EVENT = "tunnel://log";
export const DEPLOY_PROGRESS_EVENT = "deploy://progress";
export const DEPLOY_DONE_EVENT = "deploy://done";

export const api = {
  listTunnels: () => invoke<TunnelConfig[]>("list_tunnels"),
  createTunnel: (config: TunnelConfig) =>
    invoke<TunnelConfig>("create_tunnel", { config }),
  updateTunnel: (config: TunnelConfig) =>
    invoke<TunnelConfig>("update_tunnel", { config }),
  deleteTunnel: (id: string) => invoke<void>("delete_tunnel", { id }),
  startTunnel: (id: string) => invoke<TunnelState>("start_tunnel", { id }),
  stopTunnel: (id: string) => invoke<TunnelState>("stop_tunnel", { id }),
  getState: (id: string) => invoke<TunnelState>("get_state", { id }),
  listStates: () => invoke<TunnelState[]>("list_states"),
  readBinaryStatus: () => invoke<BinaryStatus>("read_binary_status"),
  installBinary: (backend: Backend) =>
    invoke<BinaryStatus>("install_binary", { backend }),

  // M2: servers / import / diagnosis
  listServers: () => invoke<ServerConfig[]>("list_servers"),
  addServer: (input: ServerInput) => invoke<ServerConfig>("add_server", { input }),
  removeServer: (id: string) => invoke<void>("remove_server", { id }),
  testServer: (id: string) => invoke<ServerStatus>("test_server", { id }),
  deployServer: (id: string) => invoke<DeployResult>("deploy_server", { id }),
  undeployServer: (id: string) => invoke<void>("undeploy_server", { id }),
  getServerStatus: (id: string) => invoke<ServerStatus>("get_server_status", { id }),
  importFrpcConfig: (text: string, serverId?: string) =>
    invoke<TunnelConfig[]>("import_frpc_config", { text, serverId }),
  diagnoseTunnel: (id: string) => invoke<Diagnosis>("diagnose_tunnel", { id }),
};

export function onTunnelState(
  handler: (state: TunnelState) => void,
): Promise<UnlistenFn> {
  return listen<TunnelStateEvent>(TUNNEL_STATE_EVENT, (event) =>
    handler(event.payload.state),
  );
}

export function onTunnelLog(
  handler: (log: TunnelLogEvent) => void,
): Promise<UnlistenFn> {
  return listen<TunnelLogEvent>(TUNNEL_LOG_EVENT, (event) =>
    handler(event.payload),
  );
}

export function onDeployProgress(
  handler: (progress: DeployProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<DeployProgressEvent>(DEPLOY_PROGRESS_EVENT, (event) =>
    handler(event.payload),
  );
}

export function onDeployDone(
  handler: (result: DeployResult) => void,
): Promise<UnlistenFn> {
  return listen<DeployResult>(DEPLOY_DONE_EVENT, (event) =>
    handler(event.payload),
  );
}
