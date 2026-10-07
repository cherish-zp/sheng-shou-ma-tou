// Global tunnel store: a module-level singleton consumed via
// useSyncExternalStore. It loads the initial data from the backend and keeps
// itself in sync with the "tunnel://state" event. Per-second traffic stats
// ("tunnel://stats") live in a separate snapshot below so their updates only
// re-render the cards whose data actually changed.

import { useSyncExternalStore } from "react";
import { toast } from "sonner";

import { api, onTunnelState, onTunnelStats } from "@/lib/tauri";
import { errorMessage } from "@/lib/utils";
import { pushSample, type StatsSample } from "@/lib/stats-history";
import i18n from "@/i18n";
import type {
  Backend,
  BinaryStatus,
  TunnelConfig,
  TunnelState,
  TunnelStats,
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
  installing: { cloudflare: false, bore: false, frp: false, cloudflareNamed: false },
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
    // Subscriptions live for the whole app session; no teardown needed.
    await onTunnelState((next) => {
      setState({ states: { ...state.states, [next.id]: next } });
      // A fresh run restarts the byte counters — drop stale history so the
      // sparkline and deltas reflect the new run only.
      if (next.status === "starting") resetTunnelStats(next.id);
    });
    await onTunnelStats(handleStatsEvent);
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
  resetTunnelStats(id);
  setState({
    configs: state.configs.filter((c) => c.id !== id),
    states,
  });
}

/** Merge a backend-provided tunnel state (e.g. result of start/stop). */
export function mergeState(next: TunnelState) {
  if (next.status === "starting") resetTunnelStats(next.id);
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

// ---------------------------------------------------------------------------
// Per-tunnel traffic stats ("tunnel://stats", throttled to 1/s per tunnel).
//
// Kept OUT of the main snapshot above on purpose: the main state re-renders
// every consumer on each status change, while stats tick every second. Each
// tunnel gets its own immutable entry object; a stats event only replaces the
// entry of the tunnel it belongs to, so useSyncExternalStore's Object.is
// check filters out unrelated per-second updates.
// ---------------------------------------------------------------------------

export interface TunnelStatsEntry {
  /** Latest event payload; null until the first event for this tunnel. */
  stats: TunnelStats | null;
  /** Ring buffer of per-second samples (see lib/stats-history.ts). */
  history: StatsSample[];
}

const EMPTY_STATS_ENTRY: TunnelStatsEntry = { stats: null, history: [] };

let statsEntries: Record<string, TunnelStatsEntry> = {};
const statsListeners = new Set<() => void>();

function subscribeStats(listener: () => void): () => void {
  statsListeners.add(listener);
  return () => statsListeners.delete(listener);
}

function notifyStatsListeners() {
  for (const listener of statsListeners) listener();
}

function handleStatsEvent(next: TunnelStats) {
  const prev = statsEntries[next.tunnelId] ?? EMPTY_STATS_ENTRY;
  const entry: TunnelStatsEntry = {
    stats: {
      tunnelId: next.tunnelId,
      bytesIn: next.bytesIn,
      bytesOut: next.bytesOut,
      connActive: next.connActive,
    },
    history: pushSample(prev.history, {
      t: Date.now(),
      bytesIn: next.bytesIn,
      bytesOut: next.bytesOut,
    }),
  };
  statsEntries = { ...statsEntries, [next.tunnelId]: entry };
  notifyStatsListeners();
}

/** Drop the stats entry of one tunnel (fresh start / tunnel removed). */
export function resetTunnelStats(id: string) {
  if (!statsEntries[id]) return;
  const entries = { ...statsEntries };
  delete entries[id];
  statsEntries = entries;
  notifyStatsListeners();
}

/**
 * React hook: live stats + sample history for one tunnel. Re-renders only
 * when this tunnel's own entry object is replaced.
 */
export function useTunnelStats(tunnelId: string): TunnelStatsEntry {
  return useSyncExternalStore(
    subscribeStats,
    () => statsEntries[tunnelId] ?? EMPTY_STATS_ENTRY,
    () => statsEntries[tunnelId] ?? EMPTY_STATS_ENTRY,
  );
}
