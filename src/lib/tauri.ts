// Typed wrappers around Tauri invoke/listen. The command names and payload
// shapes MUST match src-tauri/src/commands.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Backend,
  BinaryStatus,
  TunnelConfig,
  TunnelLogEvent,
  TunnelState,
  TunnelStateEvent,
} from "@/types/tunnel";

export const TUNNEL_STATE_EVENT = "tunnel://state";
export const TUNNEL_LOG_EVENT = "tunnel://log";

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
