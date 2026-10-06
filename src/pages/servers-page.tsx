// Servers page: manage self-hosted frps servers (add / deploy / test /
// uninstall / delete) and track live deployment progress.

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Ellipsis,
  Globe,
  LoaderCircle,
  Plus,
  Server as ServerIcon,
  Stethoscope,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { toast } from "sonner";

import { AddServerDialog } from "@/components/server/add-server-dialog";
import {
  DeployProgressDialog,
  type DeploySession,
} from "@/components/server/deploy-progress-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { api, onDeployDone, onDeployProgress } from "@/lib/tauri";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { cn, errorMessage } from "@/lib/utils";
import type { ServerConfig, ServerStatus } from "@/types/tunnel";

export function ServersPage() {
  const { t } = useTranslation();

  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [statuses, setStatuses] = useState<Record<string, ServerStatus>>({});
  const [loaded, setLoaded] = useState(false);
  const [addOpen, setAddOpen] = useState(false);

  // Deployment session lives here so the progress dialog can be minimized
  // ("run in background") without losing track of the run.
  const [session, setSession] = useState<DeploySession | null>(null);
  const [sessionOpen, setSessionOpen] = useState(false);
  const [deployingIds, setDeployingIds] = useState<Set<string>>(() => new Set());

  const reload = useCallback(async () => {
    try {
      const list = await api.listServers();
      setServers(list);
      setLoaded(true);
      // Refresh the frps running state of every deployed server. Unreachable
      // servers keep whatever we last knew.
      for (const server of list) {
        if (!server.deployed) continue;
        try {
          const status = await api.getServerStatus(server.id);
          setStatuses((prev) => ({ ...prev, [server.id]: status }));
        } catch {
          // Ignore: the status dot falls back to "deployed".
        }
      }
    } catch (error) {
      setLoaded(true);
      toast.error(t("servers.loadFailed"), { description: errorMessage(error) });
    }
  }, [t]);

  useEffect(() => {
    void reload();
  }, [reload]);

  // Live deployment events: keep updating the session even when the dialog is
  // minimized. Subscriptions last for the whole page session.
  useEffect(() => {
    let unprogress: UnlistenFn | undefined;
    let undone: UnlistenFn | undefined;
    void (async () => {
      unprogress = await onDeployProgress((progress) => {
        setSession(
          (prev) =>
            prev && prev.server.id === progress.serverId
              ? {
                  ...prev,
                  steps: {
                    ...prev.steps,
                    [progress.step]: { status: progress.status, message: progress.message },
                  },
                }
              : prev,
        );
      });
      undone = await onDeployDone((result) => {
        setSession(
          (prev) =>
            prev && prev.server.id === result.serverId
              ? { ...prev, done: result, phase: result.ok ? "success" : "failed" }
              : prev,
        );
      });
    })();
    return () => {
      unprogress?.();
      undone?.();
    };
  }, []);

  const startDeploy = useCallback(
    (server: ServerConfig) => {
      setDeployingIds((prev) => new Set(prev).add(server.id));
      setSession({ server, steps: {}, done: null, phase: "running" });
      setSessionOpen(true);
      void (async () => {
        try {
          const result = await api.deployServer(server.id);
          setSession(
            (prev) =>
              prev && prev.server.id === result.serverId
                ? { ...prev, done: result, phase: result.ok ? "success" : "failed" }
                : prev,
          );
        } catch (error) {
          setSession(
            (prev) =>
              prev && prev.server.id === server.id
                ? {
                    ...prev,
                    phase: "failed",
                    done: {
                      serverId: server.id,
                      ok: false,
                      error: errorMessage(error),
                      token: null,
                    },
                  }
                : prev,
          );
        } finally {
          setDeployingIds((prev) => {
            const next = new Set(prev);
            next.delete(server.id);
            return next;
          });
          void reload();
        }
      })();
    },
    [reload],
  );

  const finishSession = useCallback(() => {
    setSessionOpen(false);
    setSession(null);
    void reload();
  }, [reload]);

  // Close is blocked while a deployment is running (the dialog enforces this
  // too); here we only ever receive `false` for allowed closes.
  const handleSessionOpenChange = useCallback(
    (open: boolean) => {
      if (!open && session?.phase === "running") return;
      setSessionOpen(open);
      if (!open) setSession(null);
    },
    [session],
  );

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col px-8 py-10">
      {/* Header */}
      <header className="flex items-end justify-between">
        <div>
          <h1 className="text-xl font-semibold tracking-tight">{t("servers.title")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("servers.subtitle")}</p>
        </div>
        <Button onClick={() => setAddOpen(true)}>
          <Plus className="size-4" />
          {t("servers.add")}
        </Button>
      </header>

      {/* Server list */}
      {servers.length > 0 ? (
        <section className="mt-8 flex flex-col gap-4">
          <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
            {t("servers.count", { n: servers.length })}
          </p>
          {servers.map((server) => (
            <ServerCard
              key={server.id}
              server={server}
              status={statuses[server.id]}
              deploying={deployingIds.has(server.id)}
              hasSession={session?.server.id === server.id}
              onDeploy={() => startDeploy(server)}
              onViewProgress={() => setSessionOpen(true)}
              onTested={(status) =>
                setStatuses((prev) => ({ ...prev, [server.id]: status }))
              }
              onChanged={() => void reload()}
              onForgotten={(id) =>
                setStatuses((prev) => {
                  const next = { ...prev };
                  delete next[id];
                  return next;
                })
              }
            />
          ))}
        </section>
      ) : (
        <EmptyState onAdd={() => setAddOpen(true)} visible={loaded} />
      )}

      <AddServerDialog open={addOpen} onOpenChange={setAddOpen} onAdded={() => void reload()} />

      {session ? (
        <DeployProgressDialog
          session={session}
          open={sessionOpen}
          onOpenChange={handleSessionOpenChange}
          onBackground={() => setSessionOpen(false)}
          onRetry={() => startDeploy(session.server)}
          onFinish={finishSession}
        />
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Server card
// ---------------------------------------------------------------------------

interface ServerCardProps {
  server: ServerConfig;
  status?: ServerStatus;
  deploying: boolean;
  hasSession: boolean;
  onDeploy: () => void;
  onViewProgress: () => void;
  onTested: (status: ServerStatus) => void;
  onChanged: () => void;
  /** Called when the local record disappears (delete/uninstall cleanup). */
  onForgotten: (id: string) => void;
}

function ServerCard({
  server,
  status,
  deploying,
  hasSession,
  onDeploy,
  onViewProgress,
  onTested,
  onChanged,
  onForgotten,
}: ServerCardProps) {
  const { t } = useTranslation();
  const [testing, setTesting] = useState(false);
  const [undeployOpen, setUndeployOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [busy, setBusy] = useState(false);

  const address = `${server.username}@${server.host}:${server.port}`;
  const frpsRunning = Boolean(server.deployed && status?.frpsRunning);
  const statusLabel = frpsRunning
    ? t("servers.statusRunning")
    : server.deployed
      ? t("servers.statusDeployed")
      : t("servers.statusNotDeployed");

  async function handleTest() {
    setTesting(true);
    try {
      const result = await api.testServer(server.id);
      onTested(result);
      toast[result.reachable ? "success" : "error"](
        t(result.reachable ? "servers.card.testOk" : "servers.card.testFailed"),
        result.detail ? { description: result.detail } : undefined,
      );
    } catch (error) {
      toast.error(t("servers.card.testFailed"), { description: errorMessage(error) });
    } finally {
      setTesting(false);
    }
  }

  async function handleUndeploy() {
    setBusy(true);
    try {
      await api.undeployServer(server.id);
      setUndeployOpen(false);
      onForgotten(server.id);
      toast.success(t("servers.card.undeploySuccess"));
      onChanged();
    } catch (error) {
      toast.error(t("servers.card.undeployFailed"), { description: errorMessage(error) });
      setUndeployOpen(false);
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete() {
    setBusy(true);
    try {
      await api.removeServer(server.id);
      setDeleteOpen(false);
      onForgotten(server.id);
      toast.success(t("servers.card.deleteSuccess", { name: serverDisplayName(server) }));
      onChanged();
    } catch (error) {
      toast.error(t("servers.card.deleteFailed"), { description: errorMessage(error) });
      setDeleteOpen(false);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Card className="gap-0 py-0 transition-shadow hover:shadow-md">
      <CardContent className="flex flex-col gap-3.5 p-5">
        {/* Header: status + name + badges */}
        <div className="flex items-center gap-2.5">
          <Tooltip>
            <TooltipTrigger asChild>
              <span className="mr-0.5 flex size-2.5 shrink-0">
                <span
                  className={cn(
                    "inline-flex size-2.5 rounded-full",
                    frpsRunning &&
                      "animate-pulse bg-emerald-600 [animation-duration:2.4s] dark:bg-success",
                    !frpsRunning && server.deployed && "bg-sky-500 dark:bg-sky-400",
                    !server.deployed && "bg-muted-foreground/40",
                  )}
                />
              </span>
            </TooltipTrigger>
            <TooltipContent side="top">{statusLabel}</TooltipContent>
          </Tooltip>
          <h3 className="min-w-0 truncate text-[15px] font-medium">
            {serverDisplayName(server)}
          </h3>
          {server.deployed && server.frpsVersion ? (
            <Badge variant="secondary" className="font-mono text-[11px]">
              {t("servers.frpsVersion", { version: server.frpsVersion })}
            </Badge>
          ) : null}
          {server.subdomainHost ? (
            <Badge variant="outline" className="max-w-44 text-[11px] text-muted-foreground">
              <Globe className="size-3" />
              <span className="truncate">*.{server.subdomainHost}</span>
            </Badge>
          ) : null}

          <div className="ml-auto flex shrink-0 items-center gap-2">
            {deploying && hasSession ? (
              <Button variant="outline" size="sm" onClick={onViewProgress}>
                <LoaderCircle className="size-3.5 animate-spin" />
                {t("servers.card.viewProgress")}
              </Button>
            ) : (
              <Button size="sm" onClick={onDeploy}>
                {t("servers.card.deploy")}
              </Button>
            )}
            <Button
              variant="outline"
              size="sm"
              disabled={testing}
              onClick={() => void handleTest()}
            >
              {testing ? (
                <>
                  <LoaderCircle className="size-3.5 animate-spin" />
                  {t("servers.card.testing")}
                </>
              ) : (
                <>
                  <Stethoscope className="size-3.5" />
                  {t("servers.card.test")}
                </>
              )}
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  className="text-muted-foreground hover:text-foreground"
                  aria-label={t("servers.card.more")}
                >
                  <Ellipsis className="size-4" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-40">
                <DropdownMenuItem
                  disabled={!server.deployed}
                  onSelect={() => setUndeployOpen(true)}
                >
                  <TriangleAlert />
                  {t("servers.card.undeploy")}
                </DropdownMenuItem>
                <DropdownMenuItem variant="destructive" onSelect={() => setDeleteOpen(true)}>
                  <Trash2 />
                  {t("servers.card.delete")}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        </div>

        {/* Address row */}
        <div className="flex items-center gap-2 rounded-lg border bg-muted/40 px-3 py-2">
          <ServerIcon className="size-3.5 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate font-mono text-[13px]">{address}</span>
          <span
            className={cn(
              "shrink-0 text-xs",
              frpsRunning
                ? "text-emerald-700 dark:text-success"
                : server.deployed
                  ? "text-sky-600 dark:text-sky-400"
                  : "text-muted-foreground",
            )}
          >
            {statusLabel}
          </span>
        </div>

        {/* Last test detail */}
        {status?.detail ? (
          <p className="line-clamp-2 break-all font-mono text-xs leading-relaxed text-muted-foreground/80">
            {status.detail}
          </p>
        ) : null}
      </CardContent>

      {/* Uninstall confirmation */}
      <Dialog open={undeployOpen} onOpenChange={setUndeployOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("servers.card.undeployTitle")}</DialogTitle>
            <DialogDescription>{t("servers.card.undeployDescription")}</DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setUndeployOpen(false)} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" onClick={() => void handleUndeploy()} disabled={busy}>
              {t("servers.card.undeploy")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Delete confirmation */}
      <Dialog open={deleteOpen} onOpenChange={setDeleteOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t("servers.card.deleteTitle")}</DialogTitle>
            <DialogDescription>
              {t("servers.card.deleteDescription", { name: serverDisplayName(server) })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" onClick={() => setDeleteOpen(false)} disabled={busy}>
              {t("common.cancel")}
            </Button>
            <Button variant="destructive" onClick={() => void handleDelete()} disabled={busy}>
              {t("common.delete")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  );
}

function serverDisplayName(server: ServerConfig): string {
  return server.name || server.host;
}

// ---------------------------------------------------------------------------
// Empty state
// ---------------------------------------------------------------------------

function EmptyState({ onAdd, visible }: { onAdd: () => void; visible: boolean }) {
  const { t } = useTranslation();
  return (
    <section
      className={
        "mt-16 flex flex-col items-center pb-10 text-center transition-opacity duration-300 " +
        (visible ? "opacity-100" : "opacity-0")
      }
    >
      <span className="flex size-14 items-center justify-center rounded-2xl border bg-card shadow-sm">
        <ServerIcon className="size-7 text-muted-foreground" strokeWidth={1.5} />
      </span>
      <h2 className="mt-6 text-base font-medium">{t("servers.emptyTitle")}</h2>
      <p className="mt-2 max-w-sm text-sm leading-relaxed text-muted-foreground">
        {t("servers.emptyDescription")}
      </p>
      <Button onClick={onAdd} className="mt-6">
        <Plus className="size-4" />
        {t("servers.emptyCta")}
      </Button>
    </section>
  );
}
