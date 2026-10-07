import { useTranslation } from "react-i18next";
import { Loader2, RefreshCw, RotateCcw, ArrowDownToLine } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { useAppUpdater } from "@/composables/use-app-updater";

/** 设置页「检查更新」卡片：当前版本、手动检查、各阶段状态与操作。 */
export function UpdateSettingsSection({
  currentVersion,
  updater,
}: {
  currentVersion: string;
  updater: ReturnType<typeof useAppUpdater>;
}) {
  const { t } = useTranslation();
  const { state, checkNow, downloadAndInstall, restartAndUpdate } = updater;

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-[15px]">{t("update.settingsTitle")}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="flex items-center justify-between gap-3">
          <div className="text-sm">
            <p className="font-medium">
              {t("update.currentVersion", { version: currentVersion })}
            </p>
            <p className="mt-0.5 text-xs text-muted-foreground">
              {t("update.statusLabel")}
              {": "}
              {state.phase === "checking"
                ? t("update.statusChecking")
                : state.phase === "up-to-date"
                  ? t("update.statusUpToDate")
                  : state.phase === "available"
                    ? t("update.statusAvailable", { version: state.version ?? "" })
                    : state.phase === "downloading"
                      ? `${t("update.downloading")} ${state.progress}%`
                      : state.phase === "ready"
                        ? t("update.readyToRestart")
                        : state.phase === "error"
                          ? t("update.statusError")
                          : t("update.statusIdle")}
            </p>
            {state.phase === "error" && state.error && (
              <p className="mt-1 text-xs text-destructive">{state.error}</p>
            )}
          </div>
          <div className="flex shrink-0 gap-2">
            {state.phase === "available" && (
              <Button size="sm" onClick={() => void downloadAndInstall()}>
                <ArrowDownToLine className="size-3.5" />
                {t("update.installNow")}
              </Button>
            )}
            {state.phase === "ready" && (
              <Button size="sm" onClick={() => void restartAndUpdate()}>
                <RotateCcw className="size-3.5" />
                {t("update.restartNow")}
              </Button>
            )}
            <Button
              size="sm"
              variant="outline"
              disabled={state.phase === "checking" || state.phase === "downloading"}
              onClick={() => void checkNow()}
            >
              {state.phase === "checking" ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <RefreshCw className="size-3.5" />
              )}
              {t("update.checkNow")}
            </Button>
          </div>
        </div>
        <p className="text-xs text-muted-foreground">{t("update.autoCheckHint")}</p>
      </CardContent>
    </Card>
  );
}
