// Module-level deploy session store: tracks frps deployments
// (serverId -> session) so the ServersPage can be left and revisited without
// losing the live progress view. The "deploy://progress" / "deploy://done"
// listeners are registered once per app session here, not inside components.

import { useSyncExternalStore } from "react";

import { api, onDeployDone, onDeployProgress } from "@/lib/tauri";
import { errorMessage } from "@/lib/utils";
import type { DeployResult, ServerConfig, StepStatus } from "@/types/tunnel";

export interface DeployStepState {
  status: StepStatus;
  message: string | null;
}

/** A deployment run tracked by the store, keyed by the server's id. */
export interface DeploySession {
  server: ServerConfig;
  steps: Record<string, DeployStepState>;
  done: DeployResult | null;
  phase: "running" | "success" | "failed";
}

interface DeployStoreState {
  sessions: Record<string, DeploySession>;
  /** Servers with a deployServer call in flight. */
  deployingIds: ReadonlySet<string>;
  /** Which session the progress dialog currently shows. */
  activeServerId: string | null;
  /** Whether the progress dialog is open (survives page navigation). */
  open: boolean;
}

const initialState: DeployStoreState = {
  sessions: {},
  deployingIds: new Set(),
  activeServerId: null,
  open: false,
};

let state: DeployStoreState = initialState;
const listeners = new Set<() => void>();
let initPromise: Promise<void> | null = null;

function emit() {
  for (const listener of listeners) listener();
}

function setState(patch: Partial<DeployStoreState>) {
  state = { ...state, ...patch };
  emit();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getSnapshot(): DeployStoreState {
  return state;
}

/** React hook: read the deploy store and re-render on changes. */
export function useDeployStore(): DeployStoreState {
  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}

function patchSession(
  serverId: string,
  patch: (prev: DeploySession) => DeploySession,
) {
  const prev = state.sessions[serverId];
  if (!prev) return;
  setState({ sessions: { ...state.sessions, [serverId]: patch(prev) } });
}

/** Idempotent bootstrap: resident global deploy event subscriptions. */
export function ensureDeployStoreInitialized(): Promise<void> {
  if (initPromise) return initPromise;
  initPromise = (async () => {
    // Subscriptions live for the whole app session; no teardown needed.
    await onDeployProgress((progress) => {
      patchSession(progress.serverId, (prev) => ({
        ...prev,
        steps: {
          ...prev.steps,
          [progress.step]: { status: progress.status, message: progress.message },
        },
      }));
    });
    await onDeployDone((result) => {
      patchSession(result.serverId, (prev) => ({
        ...prev,
        done: result,
        phase: result.ok ? "success" : "failed",
      }));
    });
  })();
  return initPromise;
}

/**
 * Start (or retry) a deployment for `server`: create a fresh session, open
 * the progress dialog and drive it via events + the deployServer result.
 * Resolves once the deployServer invocation settles.
 */
export async function startDeploy(server: ServerConfig): Promise<void> {
  await ensureDeployStoreInitialized();
  if (state.deployingIds.has(server.id)) return;
  setState({
    sessions: {
      ...state.sessions,
      [server.id]: { server, steps: {}, done: null, phase: "running" },
    },
    deployingIds: new Set(state.deployingIds).add(server.id),
    activeServerId: server.id,
    open: true,
  });
  try {
    const result = await api.deployServer(server.id);
    patchSession(server.id, (prev) => ({
      ...prev,
      done: result,
      phase: result.ok ? "success" : "failed",
    }));
  } catch (error) {
    patchSession(server.id, (prev) => ({
      ...prev,
      phase: "failed",
      done: {
        serverId: server.id,
        ok: false,
        error: errorMessage(error),
        token: null,
      },
    }));
  } finally {
    const deployingIds = new Set(state.deployingIds);
    deployingIds.delete(server.id);
    setState({ deployingIds });
  }
}

/** Re-open the progress dialog of a session (e.g. "View deploy progress"). */
export function showDeploySession(serverId: string) {
  setState({ activeServerId: serverId, open: true });
}

/** Open/close the progress dialog without touching sessions. */
export function setDeployDialogOpen(open: boolean) {
  setState({ open });
}

/** Dismiss a finished session (Done / Close) and drop it from the store. */
export function finishDeploy(serverId: string) {
  const sessions = { ...state.sessions };
  delete sessions[serverId];
  const activeServerId =
    state.activeServerId === serverId ? null : state.activeServerId;
  setState({
    sessions,
    activeServerId,
    open: activeServerId ? state.open : false,
  });
}
