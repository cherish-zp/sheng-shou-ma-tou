import { cn } from "@/lib/utils";
import type { TunnelStatus } from "@/types/tunnel";

interface StatusDotProps {
  status: TunnelStatus;
  className?: string;
}

// Light mode uses slightly darker Tailwind greens/ambers for contrast on
// white cards; dark mode uses the lighter --success/--warning brand tokens.
const STATUS_STYLES: Record<TunnelStatus, { dot: string; ping?: string }> = {
  running: {
    dot: "bg-emerald-600 dark:bg-success",
    ping: "bg-emerald-500/60 dark:bg-success/60 [animation-duration:2.4s]",
  },
  starting: {
    dot: "bg-amber-500 dark:bg-warning",
    ping: "bg-amber-400/70 dark:bg-warning/60 [animation-duration:1.4s]",
  },
  reconnecting: {
    dot: "bg-amber-500 dark:bg-warning",
    ping: "bg-amber-400/70 dark:bg-warning/60 [animation-duration:1.4s]",
  },
  error: {
    dot: "bg-destructive",
  },
  stopped: {
    dot: "bg-muted-foreground/40",
  },
};

/** Colored status dot with a subtle breathing ring while active. */
export function StatusDot({ status, className }: StatusDotProps) {
  const styles = STATUS_STYLES[status];
  return (
    <span
      className={cn(
        "relative inline-flex size-2.5 shrink-0",
        className,
      )}
    >
      {styles.ping ? (
        <span
          className={cn(
            "absolute inline-flex size-full rounded-full animate-ping",
            styles.ping,
          )}
        />
      ) : null}
      <span
        className={cn(
          "relative inline-flex size-2.5 rounded-full",
          styles.dot,
          status === "running" && "animate-pulse [animation-duration:2.4s]",
          (status === "starting" || status === "reconnecting") &&
            "animate-pulse [animation-duration:1.2s]",
        )}
      />
    </span>
  );
}
