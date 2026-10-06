// Subscribes to the "tunnel://log" event and buffers lines for one tunnel.

import { useEffect, useState } from "react";
import type { UnlistenFn } from "@tauri-apps/api/event";

import { onTunnelLog } from "@/lib/tauri";
import type { TunnelLogEvent } from "@/types/tunnel";

const MAX_LINES = 500;

/**
 * Collects live log lines for `tunnelId` while `enabled` is true.
 * The buffer is capped and reset whenever the target tunnel changes.
 */
export function useTunnelLogs(
  tunnelId: string | null,
  enabled: boolean,
): TunnelLogEvent[] {
  const [logs, setLogs] = useState<TunnelLogEvent[]>([]);

  useEffect(() => {
    if (!enabled || !tunnelId) {
      setLogs([]);
      return;
    }
    setLogs([]);
    let alive = true;
    let unlisten: UnlistenFn | undefined;

    onTunnelLog((event) => {
      if (event.tunnelId !== tunnelId) return;
      setLogs((prev) => {
        const next = [...prev, event];
        return next.length > MAX_LINES ? next.slice(-MAX_LINES) : next;
      });
    })
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => {
        // Event system unavailable — leave the buffer empty.
      });

    return () => {
      alive = false;
      unlisten?.();
    };
  }, [tunnelId, enabled]);

  return logs;
}
