import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";

import { StatusDot } from "@/components/status-dot";
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet";
import { useTunnelLogs } from "@/hooks/use-tunnel-logs";
import { cn } from "@/lib/utils";
import type { TunnelConfig, TunnelLogEvent, TunnelState } from "@/types/tunnel";

interface LogSheetProps {
  tunnel: TunnelConfig | null;
  state?: TunnelState;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const LEVEL_CLASSES: Record<TunnelLogEvent["level"], string> = {
  info: "text-foreground/90",
  warn: "text-warning",
  error: "text-destructive",
};

export function LogSheet({ tunnel, state, open, onOpenChange }: LogSheetProps) {
  const { t } = useTranslation();
  const logs = useTunnelLogs(tunnel?.id ?? null, open);
  const scrollRef = useRef<HTMLDivElement>(null);
  const stickToBottomRef = useRef(true);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el || !stickToBottomRef.current) return;
    el.scrollTop = el.scrollHeight;
  }, [logs]);

  function handleScroll() {
    const el = scrollRef.current;
    if (!el) return;
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    stickToBottomRef.current = distance < 48;
  }

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="flex w-full flex-col gap-0 sm:max-w-xl">
        <SheetHeader className="border-b py-4">
          <SheetTitle className="flex items-center gap-2.5 text-base">
            {tunnel ? (
              <>
                <StatusDot status={state?.status ?? "stopped"} />
                <span className="truncate">{tunnel.name}</span>
                <span className="font-normal text-muted-foreground">
                  {t("logs.title")}
                </span>
              </>
            ) : (
              t("logs.title")
            )}
          </SheetTitle>
          <SheetDescription className="sr-only">
            {t("logs.title")}
          </SheetDescription>
        </SheetHeader>

        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="pier-scroll min-h-0 flex-1 overflow-y-auto bg-muted/30 px-4 py-3 font-mono text-xs leading-relaxed"
        >
          {logs.length === 0 ? (
            <p className="mt-6 text-center font-sans text-sm text-muted-foreground">
              {t("logs.empty")}
            </p>
          ) : (
            <div className="flex flex-col gap-0.5">
              {logs.map((log, index) => (
                <div key={`${log.ts}-${index}`} className="flex gap-2.5">
                  <span className="shrink-0 text-muted-foreground/60">
                    {log.ts.slice(11, 19)}
                  </span>
                  <span
                    className={cn(
                      "min-w-0 break-all",
                      LEVEL_CLASSES[log.level] ?? LEVEL_CLASSES.info,
                    )}
                  >
                    {log.line}
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>
      </SheetContent>
    </Sheet>
  );
}
