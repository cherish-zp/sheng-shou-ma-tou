import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ArrowLeft,
  ChevronDown,
  Copy,
  Dices,
  Globe,
  Network,
  Waypoints,
} from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import { cn, copyText, errorMessage, randomId, randomPassword } from "@/lib/utils";
import { api } from "@/lib/tauri";
import { mergeState, upsertConfig } from "@/store/tunnel-store";
import { CfBindFlow } from "@/components/tunnel/cf-bind-flow";
import { CfHostnameEditor } from "@/components/tunnel/cf-hostname-editor";
import type {
  ServerConfig,
  TunnelAuth,
  TunnelConfig,
  TunnelType,
} from "@/types/tunnel";

interface AddTunnelDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** When set, the dialog edits an existing tunnel instead of creating one. */
  editTunnel?: TunnelConfig | null;
}

const TOTAL_STEPS = 2;

const TYPE_OPTIONS: Array<{
  type: TunnelType;
  icon: typeof Globe;
  titleKey: string;
  descKey: string;
}> = [
  {
    type: "http",
    icon: Globe,
    titleKey: "add.httpTitle",
    descKey: "add.httpDescription",
  },
  {
    type: "tcp",
    icon: Network,
    titleKey: "add.tcpTitle",
    descKey: "add.tcpDescription",
  },
];

const CHANNEL_OPTIONS = [
  { value: "quick", labelKey: "add.channelQuick" },
  { value: "selfhosted", labelKey: "add.channelSelfHosted" },
] as const;

type Channel = (typeof CHANNEL_OPTIONS)[number]["value"];

/**
 * Cloudflare quick-channel tiers (HTTP only): temporary trycloudflare URLs
 * (default, zero setup) vs a permanent fixed hostname via a bind flow.
 */
const CF_TIER_OPTIONS = [
  {
    value: "temporary",
    titleKey: "add.hostnameTemporary",
    descKey: "add.hostnameTemporaryHint",
  },
  {
    value: "fixed",
    titleKey: "add.hostnameFixed",
    descKey: "add.hostnameFixedHint",
  },
] as const;

type CfTier = (typeof CF_TIER_OPTIONS)[number]["value"];

function parsePort(value: string): number | null {
  if (!/^\d+$/.test(value.trim())) return null;
  const port = Number.parseInt(value, 10);
  return port >= 1 && port <= 65535 ? port : null;
}

/**
 * Parse the IP allowlist textarea: one IP or CIDR per line, trimmed, empty
 * lines dropped, duplicates removed (case-insensitive, first spelling wins).
 */
export function parseAllowlist(text: string): string[] {
  const seen = new Set<string>();
  const rules: string[] = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const key = line.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    rules.push(line);
  }
  return rules;
}

export function AddTunnelDialog({
  open,
  onOpenChange,
  editTunnel,
}: AddTunnelDialogProps) {
  const { t } = useTranslation();
  const isEdit = Boolean(editTunnel);
  // Only tunnels with pre-existing auth may keep their keychain password by
  // leaving the password field empty.
  const hasExistingAuth = isEdit && Boolean(editTunnel?.auth);

  const [step, setStep] = useState(1);
  const [tunnelType, setTunnelType] = useState<TunnelType>("http");
  const [name, setName] = useState("");
  const [localHost, setLocalHost] = useState("127.0.0.1");
  const [localPort, setLocalPort] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [portError, setPortError] = useState(false);

  // M2: self-hosted channel state. Only offered in create mode when at least
  // one deployed server exists.
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [channel, setChannel] = useState<Channel>("quick");
  const [serverId, setServerId] = useState("");
  const [subdomain, setSubdomain] = useState("");
  const [remotePort, setRemotePort] = useState("");
  const [remotePortError, setRemotePortError] = useState(false);

  // v0.2.0: Cloudflare quick-channel tier (HTTP only) — temporary by default.
  const [cfTier, setCfTier] = useState<CfTier>("temporary");

  // M3: advanced options — Basic Auth + IP allowlist.
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [authEnabled, setAuthEnabled] = useState(false);
  const [authUsername, setAuthUsername] = useState("admin");
  const [authPassword, setAuthPassword] = useState("");
  const [passwordError, setPasswordError] = useState(false);
  const [allowlistText, setAllowlistText] = useState("");

  // Reset (create) or prefill (edit) whenever the dialog opens.
  useEffect(() => {
    if (!open) return;
    setSubmitting(false);
    setPortError(false);
    setServers([]);
    setChannel("quick");
    setCfTier("temporary");
    setServerId("");
    setSubdomain("");
    setRemotePort("");
    setRemotePortError(false);
    setAdvancedOpen(false);
    setPasswordError(false);
    if (editTunnel) {
      setStep(TOTAL_STEPS);
      setTunnelType(editTunnel.tunnelType);
      setName(editTunnel.name);
      setLocalHost(editTunnel.localHost);
      setLocalPort(String(editTunnel.localPort));
      setAuthEnabled(Boolean(editTunnel.auth));
      setAuthUsername(editTunnel.auth?.username || "admin");
      // Never prefill the stored password; empty means "keep it".
      setAuthPassword("");
      setAllowlistText((editTunnel.ipAllowlist ?? []).join("\n"));
    } else {
      setStep(1);
      setTunnelType("http");
      setName("");
      setLocalHost("127.0.0.1");
      setLocalPort("");
      setAuthEnabled(false);
      setAuthUsername("admin");
      setAuthPassword("");
      setAllowlistText("");
      // Load deployed servers to offer the self-hosted channel.
      api
        .listServers()
        .then((list) => setServers(list.filter((s) => s.deployed)))
        .catch(() => setServers([]));
    }
  }, [open, editTunnel]);

  const parsedPort = Number.parseInt(localPort, 10);
  const portValid =
    Number.isInteger(parsedPort) &&
    parsedPort >= 1 &&
    parsedPort <= 65535 &&
    /^\d+$/.test(localPort.trim());
  const hostValid = localHost.trim().length > 0;

  const selectedServer = servers.find((s) => s.id === serverId) ?? null;
  const useFrp = !isEdit && channel === "selfhosted" && servers.length > 0;
  const parsedRemotePort = parsePort(remotePort);

  // v0.2.0: fixed-hostname tier (Cloudflare named tunnel) — HTTP, quick
  // channel, create mode only. TCP never sees the tier (backend limitation).
  const useCfNamed =
    !isEdit && tunnelType === "http" && !useFrp && cfTier === "fixed";
  const showCfTierPicker = !isEdit && !useFrp && tunnelType === "http";
  const showTcpCfHint = !isEdit && !useFrp && tunnelType === "tcp";
  const localTargetReady = portValid && hostValid;

  const authAvailable = tunnelType === "http";
  const authActive = authEnabled && authAvailable;
  const parsedAllowlist = parseAllowlist(allowlistText);

  function pickType(type: TunnelType) {
    setTunnelType(type);
    setStep(2);
  }

  function pickChannel(next: Channel) {
    setChannel(next);
    // Preselect the only deployed server for convenience.
    if (next === "selfhosted" && !serverId && servers.length === 1) {
      setServerId(servers[0].id);
    }
  }

  /** The bind flow finished: tunnel provisioned + started — close the wizard. */
  function handleProvisioned(tunnel: TunnelConfig) {
    toast.success(t("add.cf.success", { name: tunnel.name }));
    onOpenChange(false);
  }

  function toggleAuthEnabled(on: boolean) {
    setAuthEnabled(on);
    setPasswordError(false);
    // Prefill a random password so enabling auth is always submission-ready;
    // the user can overwrite or clear it (clear = keep existing on edit).
    if (on && !authPassword) setAuthPassword(randomPassword());
  }

  async function copyPassword() {
    if (!authPassword) return;
    const ok = await copyText(authPassword);
    if (ok) toast.success(t("common.copied"));
    else toast.error(t("common.copyFailed"));
  }

  async function handleSubmit() {
    if (!portValid) {
      setPortError(true);
      return;
    }
    if (!hostValid) {
      toast.error(t("add.hostRequired"));
      return;
    }
    if (useFrp && !serverId) {
      toast.error(t("add.serverRequired"));
      return;
    }
    // frpc TCP proxies require an explicit remotePort, so make it mandatory
    // on the self-hosted channel.
    if (useFrp && tunnelType === "tcp" && parsedRemotePort === null) {
      setRemotePortError(true);
      return;
    }
    // Without a stored password there is nothing to keep — require one.
    if (authActive && !authPassword && !hasExistingAuth) {
      setPasswordError(true);
      return;
    }
    const nextAuth: TunnelAuth | null = authActive
      ? { kind: "basic", username: authUsername.trim() || "admin" }
      : null;
    setSubmitting(true);
    try {
      if (isEdit && editTunnel) {
        const updated: TunnelConfig = {
          ...editTunnel,
          name: name.trim() || `port-${parsedPort}`,
          localHost: localHost.trim(),
          localPort: parsedPort,
          auth: nextAuth,
          ipAllowlist: parsedAllowlist,
        };
        const saved = await api.updateTunnel(updated);
        upsertConfig(saved);
        // Password lives in the OS keychain, managed separately from config.
        if (nextAuth) {
          // Empty field = keep the stored password untouched.
          if (authPassword) {
            try {
              await api.setTunnelAuth(saved.id, authPassword);
            } catch (authError) {
              toast.error(t("add.auth.saveFailed"), {
                description: errorMessage(authError),
              });
            }
          }
        } else if (editTunnel.auth) {
          // Auth was turned off: clear the keychain entry too.
          try {
            await api.setTunnelAuth(saved.id, null);
          } catch (authError) {
            toast.error(t("add.auth.saveFailed"), {
              description: errorMessage(authError),
            });
          }
        }
        toast.success(t("add.updateSuccess"));
        onOpenChange(false);
      } else {
        const created: TunnelConfig = {
          id: randomId(),
          name: name.trim() || `port-${parsedPort}`,
          tunnelType,
          backend: useFrp
            ? "frp"
            : tunnelType === "http"
              ? "cloudflare"
              : "bore",
          localHost: localHost.trim() || "127.0.0.1",
          localPort: parsedPort,
          autoStart: false,
          createdAt: new Date().toISOString(),
          ...(useFrp
            ? {
                serverId,
                subdomain: tunnelType === "http" ? subdomain.trim() || null : null,
                remotePort: tunnelType === "tcp" ? parsedRemotePort : null,
              }
            : {}),
          ...(authActive ? { auth: nextAuth } : {}),
          ipAllowlist: parsedAllowlist,
        };
        const saved = await api.createTunnel(created);
        upsertConfig(saved);
        if (nextAuth) {
          try {
            await api.setTunnelAuth(saved.id, authPassword);
          } catch (authError) {
            toast.error(t("add.auth.saveFailed"), {
              description: errorMessage(authError),
            });
          }
        }
        const state = await api.startTunnel(saved.id);
        mergeState(state);
        toast.success(t("add.createSuccess", { name: saved.name }));
        onOpenChange(false);
      }
    } catch (error) {
      toast.error(t(isEdit ? "add.updateFailed" : "add.createFailed"), {
        description: errorMessage(error),
      });
    } finally {
      setSubmitting(false);
    }
  }

  const backendNote = isEdit
    ? editTunnel?.backend === "cloudflareNamed"
      ? t("add.backendMappingCfNamedEdit", {
          hostname: editTunnel.cfHostname || "",
        })
      : editTunnel?.backend === "frp"
        ? t("add.backendMappingFrpGeneric")
        : tunnelType === "http"
          ? t("add.backendMappingHttp")
          : t("add.backendMappingTcp")
    : useFrp
      ? t("add.backendMappingFrp", {
          name: selectedServer?.name || selectedServer?.host || "",
        })
      : useCfNamed
        ? t("add.backendMappingCfNamed")
        : tunnelType === "http"
          ? t("add.backendMappingHttp")
          : t("add.backendMappingTcp");

  const showChannelPicker = !isEdit && servers.length > 0;
  const advancedActive = authActive || parsedAllowlist.length > 0;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>
            {isEdit ? t("add.editTitle") : t("add.createTitle")}
          </DialogTitle>
          <DialogDescription>
            {isEdit
              ? t("add.formTitle")
              : t("add.step", { current: step, total: TOTAL_STEPS })}
          </DialogDescription>
        </DialogHeader>

        {step === 1 ? (
          <div className="flex flex-col gap-3 pt-1">
            {TYPE_OPTIONS.map(({ type, icon: Icon, titleKey, descKey }) => (
              <button
                key={type}
                type="button"
                onClick={() => pickType(type)}
                className={cn(
                  "group flex items-start gap-3.5 rounded-xl border p-4 text-left transition-all",
                  "hover:border-ring hover:bg-accent/50",
                  tunnelType === type && "border-ring bg-accent/50",
                )}
              >
                <span className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-secondary text-secondary-foreground group-hover:bg-primary group-hover:text-primary-foreground">
                  <Icon className="size-5" strokeWidth={1.8} />
                </span>
                <span className="flex flex-col gap-1">
                  <span className="text-sm font-medium">{t(titleKey)}</span>
                  <span className="text-[13px] leading-relaxed text-muted-foreground">
                    {t(descKey)}
                  </span>
                </span>
              </button>
            ))}
          </div>
        ) : (
          <div className="flex flex-col gap-4 pt-1">
            {/* Channel picker: quick (public relays) vs self-hosted (frp) */}
            {showChannelPicker ? (
              <div className="flex flex-col gap-2">
                <Label>{t("add.channel")}</Label>
                <div className="inline-flex w-fit items-center gap-1 rounded-lg border bg-muted/40 p-1">
                  {CHANNEL_OPTIONS.map(({ value, labelKey }) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => pickChannel(value)}
                      className={cn(
                        "inline-flex items-center rounded-md px-3 py-1.5 text-[13px] transition-all",
                        channel === value
                          ? "bg-background text-foreground shadow-sm"
                          : "text-muted-foreground hover:text-foreground",
                      )}
                    >
                      {t(labelKey)}
                    </button>
                  ))}
                </div>
              </div>
            ) : null}

            {/* Cloudflare hostname tier: temporary (default) vs fixed (bind flow).
                TCP never shows the tier — fixed hostnames are HTTP-only. */}
            {showCfTierPicker ? (
              <div className="flex flex-col gap-2">
                <Label>{t("add.hostnameMode")}</Label>
                <div className="grid grid-cols-2 gap-2">
                  {CF_TIER_OPTIONS.map(({ value, titleKey, descKey }) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => setCfTier(value)}
                      aria-pressed={cfTier === value}
                      className={cn(
                        "flex flex-col gap-1 rounded-lg border p-3 text-left transition-all",
                        "hover:border-ring hover:bg-accent/50",
                        cfTier === value && "border-ring bg-accent/50",
                      )}
                    >
                      <span className="text-[13px] font-medium">
                        {t(titleKey)}
                      </span>
                      <span className="text-xs leading-relaxed text-muted-foreground">
                        {t(descKey)}
                      </span>
                    </button>
                  ))}
                </div>
              </div>
            ) : showTcpCfHint ? (
              <p className="text-xs leading-relaxed text-muted-foreground/70">
                {t("add.hostnameTcpHint")}
              </p>
            ) : null}

            {/* Self-hosted: pick the deployed server */}
            {useFrp ? (
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-server">{t("add.server")}</Label>
                <Select
                  value={serverId}
                  onValueChange={(value) => setServerId(value)}
                >
                  <SelectTrigger id="tunnel-server" className="w-full">
                    <SelectValue placeholder={t("add.serverPlaceholder")} />
                  </SelectTrigger>
                  <SelectContent>
                    {servers.map((server) => (
                      <SelectItem key={server.id} value={server.id}>
                        {server.name || server.host}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            ) : null}

            {/* Name: hidden for fixed-hostname tunnels — the backend names
                them after the provisioned hostname. */}
            {!useCfNamed ? (
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-name">{t("add.name")}</Label>
                <Input
                  id="tunnel-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder={t("add.namePlaceholder", {
                    port: localPort || parsedPort || "",
                  })}
                />
              </div>
            ) : null}
            <div className="grid grid-cols-[1fr_120px] gap-3">
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-host">{t("add.localHost")}</Label>
                <Input
                  id="tunnel-host"
                  value={localHost}
                  onChange={(e) => setLocalHost(e.target.value)}
                  className="font-mono"
                  spellCheck={false}
                />
              </div>
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-port">{t("add.localPort")}</Label>
                <Input
                  id="tunnel-port"
                  type="number"
                  min={1}
                  max={65535}
                  value={localPort}
                  onChange={(e) => {
                    setLocalPort(e.target.value);
                    setPortError(false);
                  }}
                  placeholder={t("add.portPlaceholder")}
                  className={cn("font-mono", portError && "border-destructive")}
                  aria-invalid={portError}
                />
              </div>
            </div>
            {portError ? (
              <p className="text-xs text-destructive">{t("add.portRequired")}</p>
            ) : null}

            {/* Self-hosted HTTP: optional subdomain */}
            {useFrp && tunnelType === "http" ? (
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-subdomain">{t("add.subdomain")}</Label>
                <Input
                  id="tunnel-subdomain"
                  value={subdomain}
                  onChange={(e) => setSubdomain(e.target.value)}
                  placeholder={
                    selectedServer?.subdomainHost
                      ? t("add.subdomainPlaceholder", {
                          name: "myapp",
                          host: selectedServer.subdomainHost,
                        })
                      : t("add.subdomainPlaceholderNoHost")
                  }
                  className="font-mono"
                  spellCheck={false}
                />
              </div>
            ) : null}

            {/* Self-hosted TCP: required remote port */}
            {useFrp && tunnelType === "tcp" ? (
              <div className="flex flex-col gap-2">
                <Label htmlFor="tunnel-remote-port">{t("add.remotePort")}</Label>
                <Input
                  id="tunnel-remote-port"
                  type="number"
                  min={1}
                  max={65535}
                  value={remotePort}
                  onChange={(e) => {
                    setRemotePort(e.target.value);
                    setRemotePortError(false);
                  }}
                  placeholder={t("add.remotePortPlaceholder")}
                  className={cn(
                    "font-mono",
                    remotePortError && "border-destructive",
                  )}
                  aria-invalid={remotePortError}
                />
                <p
                  className={cn(
                    "text-xs leading-relaxed",
                    remotePortError ? "text-destructive" : "text-muted-foreground",
                  )}
                >
                  {remotePortError
                    ? t("add.remotePortRequired")
                    : t("add.remotePortHint")}
                </p>
              </div>
            ) : null}

            {/* Backend auto-mapping note */}
            {backendNote === "__CF_HOSTNAME_EDITOR__" && editTunnel ? (
              <CfHostnameEditor
                tunnel={editTunnel}
                onUpdated={(saved: TunnelConfig) => upsertConfig(saved)}
              />
            ) : (
              <div className="flex items-start gap-2.5 rounded-lg border bg-muted/40 px-3 py-2.5">
                <Waypoints className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                <p className="text-[13px] leading-relaxed text-muted-foreground">
                  {backendNote}
                </p>
              </div>
            )}

            {/* Fixed hostname: the 3-step Cloudflare bind flow (replaces the
                plain submit button). Appears once the local target is valid. */}
            {useCfNamed ? (
              localTargetReady ? (
                <CfBindFlow
                  localHost={localHost.trim() || "127.0.0.1"}
                  localPort={parsedPort}
                  onProvisioned={handleProvisioned}
                />
              ) : (
                <p className="text-xs leading-relaxed text-muted-foreground/70">
                  {t("add.cf.needLocalFirst")}
                </p>
              )
            ) : null}

            {/* Advanced: access auth + IP allowlist. Hidden for fixed-hostname
                tunnels — cfProvision carries no auth/allowlist fields. */}
            {!useCfNamed ? (
              <div className="rounded-lg border">
                <button
                  type="button"
                  onClick={() => setAdvancedOpen((prev) => !prev)}
                  aria-expanded={advancedOpen}
                  className="flex w-full items-center gap-2 px-3 py-2.5 text-left text-[13px] font-medium transition-colors hover:bg-accent/40"
                >
                  <ChevronDown
                    className={cn(
                      "size-4 shrink-0 text-muted-foreground transition-transform",
                      advancedOpen && "rotate-180",
                    )}
                  />
                  {t("add.advanced")}
                  {advancedActive ? (
                    <Badge variant="secondary" className="ml-auto text-[11px]">
                      {t("add.advancedActive")}
                    </Badge>
                  ) : null}
                </button>

                {advancedOpen ? (
                  <div className="flex flex-col gap-4 border-t px-3 py-3.5">
                    {/* Basic Auth */}
                    <div className="flex flex-col gap-3">
                      <div className="flex items-start justify-between gap-3">
                        <div className="flex flex-col gap-0.5">
                          <Label htmlFor="tunnel-auth-switch">
                            {t("add.auth.title")}
                          </Label>
                          <p className="text-xs leading-relaxed text-muted-foreground">
                            {authAvailable
                              ? t("add.auth.description")
                              : t("add.auth.tcpDisabled")}
                          </p>
                        </div>
                        <Switch
                          id="tunnel-auth-switch"
                          checked={authActive}
                          disabled={!authAvailable}
                          onCheckedChange={toggleAuthEnabled}
                        />
                      </div>

                      {authActive ? (
                        <div className="flex flex-col gap-2.5">
                          <div className="grid grid-cols-2 gap-3">
                            <div className="flex flex-col gap-2">
                              <Label htmlFor="tunnel-auth-username">
                                {t("add.auth.username")}
                              </Label>
                              <Input
                                id="tunnel-auth-username"
                                value={authUsername}
                                onChange={(e) => setAuthUsername(e.target.value)}
                                placeholder="admin"
                                className="font-mono"
                                autoComplete="off"
                                spellCheck={false}
                              />
                            </div>
                            <div className="flex flex-col gap-2">
                              <Label htmlFor="tunnel-auth-password">
                                {t("add.auth.password")}
                              </Label>
                              <div className="flex items-center gap-1">
                                <Input
                                  id="tunnel-auth-password"
                                  type="password"
                                  value={authPassword}
                                  onChange={(e) => {
                                    setAuthPassword(e.target.value);
                                    setPasswordError(false);
                                  }}
                                  className={cn(
                                    "font-mono",
                                    passwordError && "border-destructive",
                                  )}
                                  aria-invalid={passwordError}
                                  autoComplete="new-password"
                                  spellCheck={false}
                                />
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="icon-sm"
                                  className="shrink-0 text-muted-foreground hover:text-foreground"
                                  onClick={() => {
                                    setAuthPassword(randomPassword());
                                    setPasswordError(false);
                                  }}
                                  aria-label={t("add.auth.generate")}
                                  title={t("add.auth.generate")}
                                >
                                  <Dices className="size-4" />
                                </Button>
                                <Button
                                  type="button"
                                  variant="ghost"
                                  size="icon-sm"
                                  className="shrink-0 text-muted-foreground hover:text-foreground"
                                  disabled={!authPassword}
                                  onClick={() => void copyPassword()}
                                  aria-label={t("add.auth.copyPassword")}
                                  title={t("add.auth.copyPassword")}
                                >
                                  <Copy className="size-4" />
                                </Button>
                              </div>
                            </div>
                          </div>
                          {passwordError ? (
                            <p className="text-xs text-destructive">
                              {t("add.auth.passwordRequired")}
                            </p>
                          ) : null}
                          <p className="text-xs leading-relaxed text-muted-foreground">
                            {hasExistingAuth
                              ? t("add.auth.passwordKeep")
                              : t("add.auth.storedHint")}
                          </p>
                        </div>
                      ) : null}
                    </div>

                    {/* IP allowlist */}
                    <div className="flex flex-col gap-2">
                      <Label htmlFor="tunnel-allowlist">
                        {t("add.allowlist.title")}
                      </Label>
                      <Textarea
                        id="tunnel-allowlist"
                        value={allowlistText}
                        onChange={(e) => setAllowlistText(e.target.value)}
                        placeholder={t("add.allowlist.placeholder")}
                        rows={3}
                        className="resize-y font-mono text-[13px]"
                        spellCheck={false}
                      />
                      <p className="text-xs leading-relaxed text-muted-foreground">
                        {parsedAllowlist.length > 0
                          ? t("add.allowlist.count", { n: parsedAllowlist.length })
                          : t("add.allowlist.description")}
                      </p>
                    </div>
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        )}

        <div className="flex items-center justify-between">
          {step === 2 && !isEdit ? (
            <Button
              variant="ghost"
              size="sm"
              className="-ml-2 text-muted-foreground"
              onClick={() => setStep(1)}
            >
              <ArrowLeft className="size-4" />
              {t("common.back")}
            </Button>
          ) : (
            <span />
          )}
          {step === 2 ? (
            // Fixed-hostname flow drives its own "Create" CTA inside the
            // bind panel, so the plain submit button is suppressed.
            useCfNamed ? (
              <span />
            ) : (
              <Button onClick={handleSubmit} disabled={submitting}>
                {isEdit ? t("add.submitEdit") : t("add.submitCreate")}
              </Button>
            )
          ) : (
            <span />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
