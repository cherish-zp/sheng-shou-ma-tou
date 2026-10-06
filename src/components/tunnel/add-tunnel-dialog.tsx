import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowLeft, Globe, Network, Waypoints } from "lucide-react";
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
import { cn, errorMessage, randomId } from "@/lib/utils";
import { api } from "@/lib/tauri";
import { mergeState, upsertConfig } from "@/store/tunnel-store";
import type { TunnelConfig, TunnelType } from "@/types/tunnel";

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

export function AddTunnelDialog({
  open,
  onOpenChange,
  editTunnel,
}: AddTunnelDialogProps) {
  const { t } = useTranslation();
  const isEdit = Boolean(editTunnel);

  const [step, setStep] = useState(1);
  const [tunnelType, setTunnelType] = useState<TunnelType>("http");
  const [name, setName] = useState("");
  const [localHost, setLocalHost] = useState("127.0.0.1");
  const [localPort, setLocalPort] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [portError, setPortError] = useState(false);

  // Reset (create) or prefill (edit) whenever the dialog opens.
  useEffect(() => {
    if (!open) return;
    setSubmitting(false);
    setPortError(false);
    if (editTunnel) {
      setStep(TOTAL_STEPS);
      setTunnelType(editTunnel.tunnelType);
      setName(editTunnel.name);
      setLocalHost(editTunnel.localHost);
      setLocalPort(String(editTunnel.localPort));
    } else {
      setStep(1);
      setTunnelType("http");
      setName("");
      setLocalHost("127.0.0.1");
      setLocalPort("");
    }
  }, [open, editTunnel]);

  const parsedPort = Number.parseInt(localPort, 10);
  const portValid =
    Number.isInteger(parsedPort) &&
    parsedPort >= 1 &&
    parsedPort <= 65535 &&
    /^\d+$/.test(localPort.trim());
  const hostValid = localHost.trim().length > 0;

  function pickType(type: TunnelType) {
    setTunnelType(type);
    setStep(2);
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
    setSubmitting(true);
    try {
      if (isEdit && editTunnel) {
        const updated: TunnelConfig = {
          ...editTunnel,
          name: name.trim() || `port-${parsedPort}`,
          localHost: localHost.trim(),
          localPort: parsedPort,
        };
        const saved = await api.updateTunnel(updated);
        upsertConfig(saved);
        toast.success(t("add.updateSuccess"));
        onOpenChange(false);
      } else {
        const created: TunnelConfig = {
          id: randomId(),
          name: name.trim() || `port-${parsedPort}`,
          tunnelType,
          backend: tunnelType === "http" ? "cloudflare" : "bore",
          localHost: localHost.trim() || "127.0.0.1",
          localPort: parsedPort,
          autoStart: false,
          createdAt: new Date().toISOString(),
        };
        await api.createTunnel(created);
        upsertConfig(created);
        const state = await api.startTunnel(created.id);
        mergeState(state);
        toast.success(t("add.createSuccess", { name: created.name }));
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

  const backendNote =
    tunnelType === "http"
      ? t("add.backendMappingHttp")
      : t("add.backendMappingTcp");

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

            {/* Backend auto-mapping note */}
            <div className="flex items-start gap-2.5 rounded-lg border bg-muted/40 px-3 py-2.5">
              <Waypoints className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
              <p className="text-[13px] leading-relaxed text-muted-foreground">
                {backendNote}
              </p>
            </div>
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
            <Button onClick={handleSubmit} disabled={submitting}>
              {isEdit ? t("add.submitEdit") : t("add.submitCreate")}
            </Button>
          ) : (
            <span />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
