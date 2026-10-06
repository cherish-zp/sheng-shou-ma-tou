// Global tunnel store: a module-level singleton consumed via
// useSyncExternalStore. It loads the initial data from the backend and keeps
// itself in sync with the "tunnel://state" event.

import { useSyncExternalStore } from "react";
import { toast } from "sonner";

import { api, onTunnelState } from "@/lib/tauri";
import { errorMessage } from "@/lib/utils";
import i18n from "@/i18n";
import type {
  Backend,
  BinaryStatus,
  TunnelConfig,
  TunnelState,
} from "@/types/tunnel";

interface TunnelStoreState {
  /** True once the initial listTunnels/listStates round-trip finished (or failed). */
  initialized: boolean;
  configs: TunnelConfig[];
  states: Record<string, TunnelState>;
  binaryStatus: BinaryStatus | null;
  installing: Record<Backend, boolean>;
}

const initialState: TunnelStoreState = {
  initialized: false,
  configs: [],
  states: {},
  binaryStatus: null,
  installing: { cloudflare: false, bore: false },
};

let state: TunnelStoreState = initialState;
const listeners = new Set<() => void>();
let initPromise: Promise<void> | null = null;

function emit() {
  for (const listener of listeners) listener();
}

function setState(patch: Partial<TunnelStoreState>) {
  state = { ...state, ...patch };
  emit();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): TunnelStoreState {
  return state;
}

/** React hook: read the tunnel store and re-render on changes. */
export function useTunnelStore(): TunnelStoreState {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

/** Idempotent bootstrap: load tunnels + states and subscribe to updates. */
export function ensureStoreInitialized(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    try {
      const [configs, states] = await Promise.all([
        api.listTunnels(),
        api.listStates(),
      ]);
      setState({
        configs,
        states: Object.fromEntries(states.map((s) => [s.id, s])),
        initialized: true,
      });
    } catch (error) {
      setState({ initialized: true });
      toast.error(i18n.t("errors.loadFailed"), {
        description: errorMessage(error),
      });
    }
    // Subscription lives for the whole app session; no teardown needed.
    await onTunnelState((next) => {
      setState({ states: { ...state.states, [next.id]: next } });
    });
  })();
  return initPromise;
}

/** Insert or replace a tunnel config locally (after create/update). */
export function upsertConfig(config: TunnelConfig) {
  const configs = [...state.configs];
  const index = configs.findIndex((c) => c.id === config.id);
  if (index >= 0) configs[index] = config;
  else configs.push(config);
  setState({ configs });
}

/** Remove a tunnel config and its state locally (after delete). */
export function removeConfig(id: string) {
  const states = { ...state.states };
  delete states[id];
  setState({
    configs: state.configs.filter((c) => c.id !== id),
    states,
  });
}

/** Merge a backend-provided tunnel state (e.g. result of start/stop). */
export function mergeState(next: TunnelState) {
  setState({ states: { ...state.states, [next.id]: next } });
}

/** Fetch engine install status from the backend. */
export async function refreshBinaryStatus(): Promise<void> {
  try {
    const binaryStatus = await api.readBinaryStatus();
    setState({ binaryStatus });
  } catch (error) {
    toast.error(i18n.t("settings.loadFailed"), {
      description: errorMessage(error),
    });
  }
}

/** Install (or reinstall) an engine binary. */
export async function installEngine(backend: Backend): Promise<void> {
  if (state.installing[backend]) return;
  setState({ installing: { ...state.installing, [backend]: true } });
  try {
    const binaryStatus = await api.installBinary(backend);
    setState({ binaryStatus });
    toast.success(i18n.t("settings.installSuccess"));
  } catch (error) {
    toast.error(i18n.t("settings.installFailed"), {
      description: errorMessage(error),
    });
  } finally {
    setState({ installing: { ...state.installing, [backend]: false } });
  }
}
