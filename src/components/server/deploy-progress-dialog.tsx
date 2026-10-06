// Deploy progress dialog: renders the live step checklist emitted through the
// "deploy://progress" / "deploy://done" events while `deploy_server` runs.

import { useTranslation } from "react-i18next";
import {
  Circle,
  CircleCheck,
  CircleX,
  Copy,
  LoaderCircle,
  Minus,
  RotateCcw,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { copyText, cn } from "@/lib/utils";
import type { DeploySession } from "@/store/deploy-store";
import type { StepStatus } from "@/types/tunnel";

/** Canonical steps in backend order (frp_deploy.rs); unknown slugs appended. */
export const DEPLOY_STEPS = [
  "connect",
  "probe",
  "ports",
  "download",
  "config",
  "systemd",
  "firewall",
  "verify",
] as const;

interface DeployProgressDialogProps {
  session: DeploySession;
  open: boolean;
  /** Called on close; the page decides whether closing is allowed. */
  onOpenChange: (open: boolean) => void;
  /** Minimize while the deployment keeps running in the background. */
  onBackground: () => void;
  /** Re-run deployServer after a failure. */
  onRetry: () => void;
  /** Dismiss after success/failure: clears the session and refreshes data. */
  onFinish: () => void;
}

function StepIcon({ status }: { status?: StepStatus }) {
  switch (status) {
    case "running":
      return (
        <LoaderCircle className="size-4 shrink-0 animate-spin text-muted-foreground" />
      );
    case "ok":
      return (
        <CircleCheck className="size-4 shrink-0 text-emerald-600 dark:text-success" />
      );
    case "fail":
      return <CircleX className="size-4 shrink-0 text-destructive" />;
    case "skip":
      return <Minus className="size-4 shrink-0 text-muted-foreground/50" />;
    default:
      return <Circle className="size-4 shrink-0 text-muted-foreground/30" />;
  }
}

export function DeployProgressDialog({
  session,
  open,
  onOpenChange,
  onBackground,
  onRetry,
  onFinish,
}: DeployProgressDialogProps) {
  const { t } = useTranslation();
  const running = session.phase === "running";

  // Canonical order first, then any unknown steps reported by the backend.
  const stepSlugs = [...DEPLOY_STEPS, ...Object.keys(session.steps).filter(
    (slug) => !(DEPLOY_STEPS as readonly string[]).includes(slug),
  )];

  async function handleCopyToken() {
    const token = session.done?.token;
    if (!token) return;
    const ok = await copyText(token);
    if (ok) toast.success(t("common.copied"));
    else toast.error(t("common.copyFailed"));
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="max-w-md"
        showCloseButton={!running}
        onEscapeKeyDown={(e) => {
          if (running) e.preventDefault();
        }}
        onPointerDownOutside={(e) => {
          if (running) e.preventDefault();
        }}
        onInteractOutside={(e) => {
          if (running) e.preventDefault();
        }}
      >
        <DialogHeader>
          <DialogTitle>{t("deploy.title")}</DialogTitle>
          <DialogDescription>
            {session.phase === "success"
              ? t("deploy.successDescription")
              : session.phase === "failed"
                ? t("deploy.failedTitle")
                : t("deploy.subtitle", { name: session.server.name || session.server.host })}
          </DialogDescription>
        </DialogHeader>

        {/* Step checklist */}
        <ol className="flex flex-col gap-1">
          {stepSlugs.map((slug) => {
            const step = session.steps[slug];
            const status = step?.status;
            return (
              <li key={slug} className="flex flex-col gap-0.5">
                <div
                  className={cn(
                    "flex items-center gap-2.5 rounded-md px-2 py-1.5 text-sm",
                    status === "fail" && "bg-destructive/10",
                  )}
                >
                  <StepIcon status={status} />
                  <span
                    className={cn(
                      status === undefined && "text-muted-foreground",
                      status === "skip" && "text-muted-foreground/60",
                    )}
                  >
                    {t(`deploy.steps.${slug}`, slug)}
                  </span>
                </div>
                {step?.message ? (
                  <p className="pl-9 text-xs leading-relaxed text-muted-foreground/80">
                    {step.message}
                  </p>
                ) : null}
              </li>
            );
          })}
        </ol>

        {running ? (
          <p className="rounded-lg border bg-muted/40 px-3 py-2.5 text-[13px] leading-relaxed text-muted-foreground">
            {t("deploy.runningHint")}
          </p>
        ) : null}

        {session.phase === "success" ? (
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-2.5 rounded-lg border border-emerald-600/30 bg-emerald-600/10 px-3 py-2.5 dark:border-success/30 dark:bg-success/10">
              <CircleCheck className="size-5 shrink-0 text-emerald-600 dark:text-success" />
              <p className="text-sm font-medium text-emerald-700 dark:text-success">
                {t("deploy.successTitle")}
              </p>
            </div>

            <div className="flex flex-col gap-2 rounded-lg border px-3 py-3">
              <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                {t("deploy.portsTitle")}
              </p>
              <PortRow
                label={t("deploy.portBind")}
                value={session.server.frpsBindPort}
              />
              <PortRow
                label={t("deploy.portVhostHttp")}
                value={session.server.frpsVhostHttpPort}
              />
              <PortRow
                label={t("deploy.portVhostHttps")}
                value={session.server.frpsVhostHttpsPort}
              />
              <PortRow
                label={t("deploy.portDashboard")}
                value={session.server.frpsDashboardPort}
              />
              {session.done?.token ? (
                <>
                  <p className="mt-1 text-xs font-medium uppercase tracking-wider text-muted-foreground">
                    {t("deploy.tokenLabel")}
                  </p>
                  <div className="flex items-center gap-2">
                    <code className="min-w-0 flex-1 truncate rounded bg-muted/60 px-2 py-1 font-mono text-xs">
                      {session.done.token}
                    </code>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      className="text-muted-foreground hover:text-foreground"
                      onClick={() => void handleCopyToken()}
                      aria-label={t("common.copy")}
                    >
                      <Copy className="size-3.5" />
                    </Button>
                  </div>
                </>
              ) : null}
            </div>
          </div>
        ) : null}

        {session.phase === "failed" ? (
          <div className="flex flex-col gap-1.5 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2.5">
            <p className="text-[13px] font-medium text-destructive">
              {t("deploy.failedTitle")}
            </p>
            <p className="break-all font-mono text-xs leading-relaxed text-destructive/90">
              {session.done?.error || t("errors.unknown")}
            </p>
          </div>
        ) : null}

        <DialogFooter className="gap-2 sm:gap-2">
          {running ? (
            <Button variant="outline" onClick={onBackground}>
              {t("deploy.background")}
            </Button>
          ) : session.phase === "failed" ? (
            <>
              <Button variant="outline" onClick={onFinish}>
                {t("common.close")}
              </Button>
              <Button onClick={onRetry}>
                <RotateCcw className="size-4" />
                {t("deploy.retry")}
              </Button>
            </>
          ) : (
            <Button onClick={onFinish}>{t("deploy.done")}</Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function PortRow({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex items-center justify-between text-[13px]">
      <span className="text-muted-foreground">{label}</span>
      <span className="font-mono">{value}</span>
    </div>
  );
}
