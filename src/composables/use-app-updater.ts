import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * App auto-update state machine (dbx-style orchestration on top of
 * tauri-plugin-updater):
 *
 *   idle → checking → available → downloading (progress) → ready → (relaunch)
 *                    ↘ up-to-date / error
 *
 * Triggers: once on startup (delayed), every 60 minutes, and manual check
 * from Settings. "Ignore this version" is persisted to localStorage.
 */

export type UpdatePhase =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "ready"
  | "error";

export interface UpdateState {
  phase: UpdatePhase;
  /** New version string (when available/ready). */
  version: string | null;
  /** Release notes from latest.json. */
  notes: string | null;
  /** 0–100 while downloading. */
  progress: number;
  /** Downloaded / total bytes while downloading. */
  downloaded: number;
  total: number;
  error: string | null;
  /** Version the user chose to skip. */
  ignoredVersion: string | null;
}

const IGNORE_KEY = "update.ignoredVersion";
const CHECK_INTERVAL_MS = 60 * 60 * 1000; // 60 minutes
const STARTUP_DELAY_MS = 8 * 1000; // let the app settle before the first check

function getIgnoredVersion(): string | null {
  return localStorage.getItem(IGNORE_KEY);
}

export function ignoreVersion(version: string) {
  localStorage.setItem(IGNORE_KEY, version);
}

export function clearIgnoredVersion() {
  localStorage.removeItem(IGNORE_KEY);
}

export function useAppUpdater() {
  const [state, setState] = useState<UpdateState>({
    phase: "idle",
    version: null,
    notes: null,
    progress: 0,
    downloaded: 0,
    total: 0,
    error: null,
    ignoredVersion: getIgnoredVersion(),
  });

  const patch = (p: Partial<UpdateState> | ((s: UpdateState) => Partial<UpdateState>)) =>
    setState((s) => ({ ...s, ...(typeof p === "function" ? p(s) : p) }));

  async function checkNow(): Promise<UpdateState["phase"]> {
    if (state.phase === "checking" || state.phase === "downloading") {
      return state.phase;
    }
    patch({ phase: "checking", error: null });
    try {
      const update = await check();
      if (!update) {
        patch({ phase: "up-to-date" });
        return "up-to-date";
      }
      const ignored = getIgnoredVersion();
      if (ignored === update.version) {
        patch({ phase: "up-to-date", version: update.version });
        return "up-to-date";
      }
      patch({
        phase: "available",
        version: update.version,
        notes: update.body ?? null,
        error: null,
      });
      return "available";
    } catch (e) {
      patch({ phase: "error", error: String(e) });
      return "error";
    }
  }

  async function downloadAndInstall() {
    if (state.phase !== "available") return;
    patch({ phase: "downloading", progress: 0, downloaded: 0, total: 0 });
    try {
      const update = await check();
      if (!update) {
        patch({ phase: "error", error: "update disappeared, please re-check" });
        return;
      }
      let contentLength = 0;
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength = event.data.contentLength ?? 0;
            patch({ total: contentLength });
            break;
          case "Progress":
            patch((s) => ({
              downloaded: s.downloaded + event.data.chunkLength,
              progress: contentLength
                ? Math.min(
                    100,
                    Math.round(((s.downloaded + event.data.chunkLength) / contentLength) * 100),
                  )
                : s.progress,
            }));
            break;
          case "Finished":
            patch({ progress: 100 });
            break;
        }
      });
      patch({ phase: "ready" });
    } catch (e) {
      patch({ phase: "error", error: String(e) });
    }
  }

  async function restartAndUpdate() {
    await relaunch();
  }

  function ignoreCurrent() {
    if (state.version) {
      ignoreVersion(state.version);
      patch({ ignoredVersion: state.version, phase: "idle", version: null, notes: null });
    }
  }

  // Startup check (delayed) + interval.
  useEffect(() => {
    const t = setTimeout(() => void checkNow(), STARTUP_DELAY_MS);
    const interval = setInterval(() => void checkNow(), CHECK_INTERVAL_MS);
    return () => {
      clearTimeout(t);
      clearInterval(interval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  function dismissError() {
    patch({ phase: "idle", error: null });
  }

  return {
    state,
    checkNow,
    downloadAndInstall,
    restartAndUpdate,
    ignoreCurrent,
    clearIgnoredVersion,
    dismissError,
  };
}
