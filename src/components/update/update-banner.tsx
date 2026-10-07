import { useTranslation } from "react-i18next";
import { ArrowDownToLine, Loader2, RefreshCw, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useAppUpdater } from "@/composables/use-app-updater";

/**
 * Auto-update banner: floats at the bottom of the main window whenever the
 * updater is doing something the user should know about (new version,
 * download progress, ready-to-restart, or an error).
 */
export function UpdateBanner() {
  const { t } = useTranslation();
  const {
    state,
    downloadAndInstall,
    restartAndUpdate,
    ignoreCurrent,
    dismissError,
  } = useAppUpdater();

  if (
    state.phase === "idle" ||
    state.phase === "checking" ||
    state.phase === "up-to-date"
  ) {
    return null;
  }

  if (state.phase === "error") {
    return (
      <div className="fixed bottom-4 left-1/2 z-50 w-[min(92%,560px)] -translate-x-1/2 rounded-lg border border-destructive/40 bg-background p-3 shadow-lg">
        <div className="flex items-center gap-2 text-sm">
          <span className="text-destructive">{t("update.checkFailed")}</span>
          <span className="truncate text-xs text-muted-foreground">
            {state.error}
          </span>
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto shrink-0"
            onClick={dismissError}
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </div>
    );
  }

  if (state.phase === "available") {
    return (
      <div className="fixed bottom-4 left-1/2 z-50 w-[min(92%,560px)] -translate-x-1/2 rounded-lg border bg-background p-3 shadow-lg">
        <div className="flex items-center gap-3">
          <span className="flex size-8 shrink-0 items-center justify-center rounded-full bg-success/15">
            <ArrowDownToLine className="size-4 text-success" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">
              {t("update.available", { version: state.version ?? "" })}
            </p>
            {state.notes && (
              <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                {state.notes}
              </p>
            )}
          </div>
          <Button size="sm" onClick={() => void downloadAndInstall()}>
            {t("update.installNow")}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            onClick={ignoreCurrent}
            title={t("update.ignoreVersion")}
          >
            <X className="size-3.5" />
          </Button>
        </div>
      </div>
    );
  }

  if (state.phase === "downloading") {
    const pct =
      state.progress || (state.total ? Math.round((state.downloaded / state.total) * 100) : 0);
    return (
      <div className="fixed bottom-4 left-1/2 z-50 w-[min(92%,560px)] -translate-x-1/2 rounded-lg border bg-background p-3 shadow-lg">
        <div className="flex items-center gap-3">
          <Loader2 className="size-4 shrink-0 animate-spin text-muted-foreground" />
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium">{t("update.downloading")}</p>
            <div className="mt-1.5 h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-success transition-all"
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>
          <span className="shrink-0 font-mono text-xs text-muted-foreground">
            {pct}%
          </span>
        </div>
      </div>
    );
  }

  // ready: restart to apply
  return (
    <div className="fixed bottom-4 left-1/2 z-50 w-[min(92%,560px)] -translate-x-1/2 rounded-lg border border-success/40 bg-background p-3 shadow-lg">
      <div className="flex items-center gap-3">
        <RefreshCw className="size-4 shrink-0 text-success" />
        <p className="min-w-0 flex-1 text-sm font-medium">
          {t("update.readyToRestart")}
        </p>
        <Button size="sm" onClick={() => void restartAndUpdate()}>
          {t("update.restartNow")}
        </Button>
      </div>
    </div>
  );
}
