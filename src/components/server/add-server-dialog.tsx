// Dialog for registering a self-hosted frps server (SSH credentials are moved
// into the OS keychain by the backend). Advanced section exposes frps ports.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, LoaderCircle } from "lucide-react";
import { toast } from "sonner";

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
import { api } from "@/lib/tauri";
import { cn, errorMessage } from "@/lib/utils";
import type { AuthKind, ServerInput } from "@/types/tunnel";

interface AddServerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Called after a server was added successfully. */
  onAdded: (server: Awaited<ReturnType<typeof api.addServer>>) => void;
}

interface PortField {
  key: "frpsBindPort" | "frpsVhostHttpPort" | "frpsVhostHttpsPort" | "frpsDashboardPort";
  labelKey: string;
  defaultValue: number;
}

const PORT_FIELDS: PortField[] = [
  { key: "frpsBindPort", labelKey: "servers.form.frpsBindPort", defaultValue: 7000 },
  { key: "frpsVhostHttpPort", labelKey: "servers.form.frpsVhostHttpPort", defaultValue: 8080 },
  { key: "frpsVhostHttpsPort", labelKey: "servers.form.frpsVhostHttpsPort", defaultValue: 8443 },
  { key: "frpsDashboardPort", labelKey: "servers.form.frpsDashboardPort", defaultValue: 7500 },
];

function parsePort(value: string): number | null {
  if (!/^\d+$/.test(value.trim())) return null;
  const port = Number.parseInt(value, 10);
  return port >= 1 && port <= 65535 ? port : null;
}

export function AddServerDialog({ open, onOpenChange, onAdded }: AddServerDialogProps) {
  const { t } = useTranslation();

  const [name, setName] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [username, setUsername] = useState("");
  const [authKind, setAuthKind] = useState<AuthKind>("password");
  const [secret, setSecret] = useState("");
  const [subdomainHost, setSubdomainHost] = useState("");
  const [proxyPortStart, setProxyPortStart] = useState("");
  const [proxyPortEnd, setProxyPortEnd] = useState("");
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [ports, setPorts] = useState<Record<PortField["key"], string>>({
    frpsBindPort: "7000",
    frpsVhostHttpPort: "8080",
    frpsVhostHttpsPort: "8443",
    frpsDashboardPort: "7500",
  });
  const [submitting, setSubmitting] = useState(false);
  const [errors, setErrors] = useState<Set<"host" | "port" | "username" | "secret">>(new Set());

  // Fresh form every time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setName("");
    setHost("");
    setPort("22");
    setUsername("");
    setAuthKind("password");
    setSecret("");
    setSubdomainHost("");
    setProxyPortStart("");
    setProxyPortEnd("");
    setAdvancedOpen(false);
    setPorts({ frpsBindPort: "7000", frpsVhostHttpPort: "8080", frpsVhostHttpsPort: "8443", frpsDashboardPort: "7500" });
    setSubmitting(false);
    setErrors(new Set());
  }, [open]);

  function validate(): boolean {
    const next = new Set<"host" | "port" | "username" | "secret">();
    if (!host.trim()) next.add("host");
    if (parsePort(port) === null) next.add("port");
    if (!username.trim()) next.add("username");
    if (!secret.trim()) next.add("secret");
    setErrors(next);
    return next.size === 0;
  }

  async function handleSubmit() {
    if (!validate()) return;
    setSubmitting(true);
    try {
      const sshPort = parsePort(port);
      const input: ServerInput = {
        name: name.trim(),
        host: host.trim(),
        port: sshPort as number,
        username: username.trim(),
        authKind,
        secret: secret.trim(),
      };
      const bindPort = parsePort(ports.frpsBindPort);
      const httpPort = parsePort(ports.frpsVhostHttpPort);
      const httpsPort = parsePort(ports.frpsVhostHttpsPort);
      const dashboardPort = parsePort(ports.frpsDashboardPort);
      if (bindPort !== null) input.frpsBindPort = bindPort;
      if (httpPort !== null) input.frpsVhostHttpPort = httpPort;
      if (httpsPort !== null) input.frpsVhostHttpsPort = httpsPort;
      if (dashboardPort !== null) input.frpsDashboardPort = dashboardPort;
      if (subdomainHost.trim()) input.subdomainHost = subdomainHost.trim();
      // Optional TCP/UDP forwarding port range: blank = unrestricted.
      const proxyStart = parsePort(proxyPortStart);
      const proxyEnd = parsePort(proxyPortEnd);
      if (proxyStart !== null && proxyEnd !== null) {
        if (proxyStart > proxyEnd) {
          toast.error(t("servers.form.proxyPortInvalid"));
          setSubmitting(false);
          return;
        }
        input.frpsProxyPortStart = proxyStart;
        input.frpsProxyPortEnd = proxyEnd;
      }

      const server = await api.addServer(input);
      toast.success(t("servers.form.success"));
      onOpenChange(false);
      onAdded(server);
    } catch (error) {
      toast.error(t("servers.form.failed"), { description: errorMessage(error) });
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>{t("servers.form.title")}</DialogTitle>
          <DialogDescription>{t("servers.form.description")}</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-[1fr_88px] gap-3">
            <div className="flex flex-col gap-2">
              <Label htmlFor="server-host">{t("servers.form.host")}</Label>
              <Input
                id="server-host"
                value={host}
                onChange={(e) => {
                  setHost(e.target.value);
                  removeError("host");
                }}
                placeholder={t("servers.form.hostPlaceholder")}
                className={cn("font-mono", errors.has("host") && "border-destructive")}
                aria-invalid={errors.has("host")}
                spellCheck={false}
              />
            </div>
            <div className="flex flex-col gap-2">
              <Label htmlFor="server-port">{t("servers.form.port")}</Label>
              <Input
                id="server-port"
                type="number"
                min={1}
                max={65535}
                value={port}
                onChange={(e) => {
                  setPort(e.target.value);
                  removeError("port");
                }}
                className={cn("font-mono", errors.has("port") && "border-destructive")}
                aria-invalid={errors.has("port")}
              />
            </div>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="server-name">{t("servers.form.name")}</Label>
            <Input
              id="server-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t("servers.form.namePlaceholder")}
            />
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="server-username">{t("servers.form.username")}</Label>
            <Input
              id="server-username"
              value={username}
              onChange={(e) => {
                setUsername(e.target.value);
                removeError("username");
              }}
              placeholder={t("servers.form.usernamePlaceholder")}
              className={cn(errors.has("username") && "border-destructive")}
              aria-invalid={errors.has("username")}
              spellCheck={false}
            />
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="server-auth-kind">{t("servers.form.authKind")}</Label>
            <Select
              value={authKind}
              onValueChange={(value) => setAuthKind(value as AuthKind)}
            >
              <SelectTrigger id="server-auth-kind" className="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="password">{t("servers.form.authPassword")}</SelectItem>
                <SelectItem value="keypath">{t("servers.form.authKeypath")}</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="server-secret">
              {t(authKind === "password" ? "servers.form.secretPassword" : "servers.form.secretKeypath")}
            </Label>
            <Input
              id="server-secret"
              type={authKind === "password" ? "password" : "text"}
              value={secret}
              onChange={(e) => {
                setSecret(e.target.value);
                removeError("secret");
              }}
              placeholder={
                authKind === "password"
                  ? undefined
                  : t("servers.form.secretKeypathPlaceholder")
              }
              className={cn(
                authKind === "keypath" && "font-mono",
                errors.has("secret") && "border-destructive",
              )}
              aria-invalid={errors.has("secret")}
              spellCheck={false}
            />
          </div>

          {/* Advanced: frps ports + wildcard domain */}
          <div className="rounded-lg border">
            <button
              type="button"
              onClick={() => setAdvancedOpen((v) => !v)}
              className="flex w-full items-center justify-between px-3 py-2.5 text-[13px] font-medium"
            >
              {t("servers.form.advanced")}
              <ChevronDown
                className={cn(
                  "size-4 text-muted-foreground transition-transform",
                  advancedOpen && "rotate-180",
                )}
              />
            </button>
            {advancedOpen ? (
              <div className="flex flex-col gap-4 border-t px-3 py-4">
                <div className="grid grid-cols-2 gap-3">
                  {PORT_FIELDS.map(({ key, labelKey, defaultValue }) => (
                    <div key={key} className="flex flex-col gap-1.5">
                      <Label htmlFor={`server-${key}`} className="text-xs text-muted-foreground">
                        {t(labelKey)}
                      </Label>
                      <Input
                        id={`server-${key}`}
                        type="number"
                        min={1}
                        max={65535}
                        value={ports[key]}
                        onChange={(e) =>
                          setPorts((prev) => ({ ...prev, [key]: e.target.value }))
                        }
                        placeholder={String(defaultValue)}
                        className="font-mono"
                      />
                    </div>
                  ))}
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label htmlFor="server-subdomain-host" className="text-xs text-muted-foreground">
                    {t("servers.form.subdomainHost")}
                  </Label>
                  <Input
                    id="server-subdomain-host"
                    value={subdomainHost}
                    onChange={(e) => setSubdomainHost(e.target.value)}
                    placeholder={t("servers.form.subdomainHostPlaceholder")}
                    className="font-mono"
                    spellCheck={false}
                  />
                  {subdomainHost.trim() ? (
                    <p className="text-xs leading-relaxed text-muted-foreground">
                      {t("servers.form.subdomainHostHint", { host: subdomainHost.trim() })}
                    </p>
                  ) : null}
                </div>
                <div className="flex flex-col gap-1.5">
                  <Label className="text-xs text-muted-foreground">
                    {t("servers.form.proxyPortRange")}
                  </Label>
                  <div className="grid grid-cols-2 gap-2">
                    <Input
                      id="server-proxy-port-start"
                      type="number"
                      min={1}
                      max={65535}
                      value={proxyPortStart}
                      onChange={(e) => setProxyPortStart(e.target.value)}
                      placeholder={t("servers.form.proxyPortStart")}
                      className="font-mono"
                    />
                    <Input
                      id="server-proxy-port-end"
                      type="number"
                      min={1}
                      max={65535}
                      value={proxyPortEnd}
                      onChange={(e) => setProxyPortEnd(e.target.value)}
                      placeholder={t("servers.form.proxyPortEnd")}
                      className="font-mono"
                    />
                  </div>
                  <p className="text-xs leading-relaxed text-muted-foreground">
                    {t("servers.form.proxyPortHint")}
                  </p>
                </div>
              </div>
            ) : null}
          </div>
        </div>

        <div className="flex items-center justify-end gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={submitting}>
            {t("common.cancel")}
          </Button>
          <Button onClick={() => void handleSubmit()} disabled={submitting}>
            {submitting ? (
              <>
                <LoaderCircle className="size-4 animate-spin" />
                {t("servers.form.submitting")}
              </>
            ) : (
              t("servers.form.submit")
            )}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );

  function removeError(field: "host" | "port" | "username" | "secret") {
    setErrors((prev) => {
      if (!prev.has(field)) return prev;
      const next = new Set(prev);
      next.delete(field);
      return next;
    });
  }
}
