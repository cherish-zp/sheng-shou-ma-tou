// Import dialog: paste an frpc config (TOML or INI), preview the parsed
// tunnels, then create them one by one.

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { FileUp, LoaderCircle } from "lucide-react";
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
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { api } from "@/lib/tauri";
import { errorMessage } from "@/lib/utils";
import { upsertConfig } from "@/store/tunnel-store";
import type { ServerConfig, TunnelConfig } from "@/types/tunnel";

interface ImportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Called after tunnels were imported so the list refreshes. */
  onImported: () => void;
}

export function ImportDialog({ open, onOpenChange, onImported }: ImportDialogProps) {
  const { t } = useTranslation();

  const [text, setText] = useState("");
  const [servers, setServers] = useState<ServerConfig[]>([]);
  const [serverId, setServerId] = useState("");
  const [preview, setPreview] = useState<TunnelConfig[] | null>(null);
  const [parsing, setParsing] = useState(false);
  const [importing, setImporting] = useState(false);

  // Fresh state + server list every time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setText("");
    setServerId("");
    setPreview(null);
    setParsing(false);
    setImporting(false);
    api
      .listServers()
      .then((list) => {
        setServers(list);
        // Keep only one preselectable target: a single deployed server.
        const deployed = list.filter((s) => s.deployed);
        if (deployed.length === 1) setServerId(deployed[0].id);
      })
      .catch(() => setServers([]));
  }, [open]);

  async function handleParse() {
    if (!text.trim()) {
      toast.error(t("import.emptyText"));
      return;
    }
    setParsing(true);
    try {
      const tunnels = await api.importFrpcConfig(
        text,
        serverId || undefined,
      );
      if (tunnels.length === 0) {
        setPreview(null);
        toast.error(t("import.parseFailed"), {
          description: t("import.parseFailedDescription"),
        });
        return;
      }
      setPreview(tunnels);
    } catch (error) {
      setPreview(null);
      toast.error(t("import.parseFailed"), {
        description: errorMessage(error) || t("import.parseFailedDescription"),
      });
    } finally {
      setParsing(false);
    }
  }

  async function handleImport() {
    if (!preview) return;
    setImporting(true);
    let ok = 0;
    let failed = 0;
    for (const config of preview) {
      try {
        const created = await api.createTunnel(config);
        upsertConfig(created);
        ok += 1;
      } catch {
        failed += 1;
      }
    }
    setImporting(false);
    if (failed === 0) {
      toast.success(t("import.success", { n: ok }));
    } else if (ok > 0) {
      toast.warning(t("import.partial", { ok, failed }));
    } else {
      toast.error(t("import.importFailed"));
    }
    if (ok > 0) {
      onOpenChange(false);
      onImported();
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>{t("import.title")}</DialogTitle>
          <DialogDescription>{t("import.description")}</DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-4">
          <textarea
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setPreview(null);
            }}
            placeholder={t("import.placeholder")}
            spellCheck={false}
            rows={8}
            className="pier-scroll w-full resize-y rounded-lg border bg-transparent px-3 py-2.5 font-mono text-[13px] leading-relaxed shadow-xs outline-none transition-[color,box-shadow] placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30"
          />

          {servers.length > 0 ? (
            <div className="flex flex-col gap-2">
              <Label htmlFor="import-server">{t("import.targetServer")}</Label>
              <Select
                value={serverId}
                onValueChange={(value) => {
                  setServerId(value === "none" ? "" : value);
                  setPreview(null);
                }}
              >
                <SelectTrigger id="import-server" className="w-full">
                  <SelectValue placeholder={t("import.noServer")} />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="none">{t("import.noServer")}</SelectItem>
                  {servers.map((server) => (
                    <SelectItem key={server.id} value={server.id}>
                      {server.name || server.host}
                      {server.deployed
                        ? ""
                        : ` (${t("servers.statusNotDeployed")})`}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              {serverId ? null : (
                <p className="text-xs text-muted-foreground">{t("import.noServerHint")}</p>
              )}
            </div>
          ) : null}

          {preview ? (
            <div className="flex flex-col gap-2">
              <p className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
                {t("import.previewTitle", { n: preview.length })}
              </p>
              <div className="pier-scroll max-h-48 overflow-y-auto rounded-lg border">
                {preview.map((tunnel, index) => (
                  <div
                    key={tunnel.id || index}
                    className="flex items-center gap-2.5 border-b px-3 py-2 text-[13px] last:border-b-0"
                  >
                    <span className="min-w-0 flex-1 truncate font-medium">
                      {tunnel.name}
                    </span>
                    <Badge variant="secondary" className="font-mono text-[11px]">
                      {tunnel.tunnelType.toUpperCase()}
                    </Badge>
                    <span className="shrink-0 font-mono text-xs text-muted-foreground">
                      {tunnel.localPort}
                    </span>
                    <span className="w-28 shrink-0 truncate text-right font-mono text-xs text-muted-foreground">
                      {tunnel.subdomain ?? tunnel.remotePort ?? tunnel.backend}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          ) : null}
        </div>

        <div className="flex items-center justify-end gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={importing}>
            {t("common.cancel")}
          </Button>
          {preview ? (
            <Button onClick={() => void handleImport()} disabled={importing}>
              {importing ? (
                <>
                  <LoaderCircle className="size-4 animate-spin" />
                  {t("import.importing")}
                </>
              ) : (
                <>
                  <FileUp className="size-4" />
                  {t("import.submit", { n: preview.length })}
                </>
              )}
            </Button>
          ) : (
            <Button
              onClick={() => void handleParse()}
              disabled={parsing || !text.trim()}
            >
              {parsing ? (
                <>
                  <LoaderCircle className="size-4 animate-spin" />
                  {t("import.parseButton")}
                </>
              ) : (
                t("import.parseButton")
              )}
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
