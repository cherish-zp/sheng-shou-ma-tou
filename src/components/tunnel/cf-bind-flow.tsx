// Three-step Cloudflare bind flow for fixed-hostname (named) tunnels:
// 1. paste + verify an API token  2. pick a zone + subdomain  3. provision.
// The token never leaves this component except as a transient argument to the
// cf_* commands (the backend moves it into the OS keychain).

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  CheckCircle2,
  ChevronRight,
  ExternalLink,
  Eye,
  EyeOff,
  Globe,
  Loader2,
  PenLine,
} from "lucide-react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
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
import { cn, errorMessage } from "@/lib/utils";
import { api } from "@/lib/tauri";
import { mergeState, upsertConfig } from "@/store/tunnel-store";
import type { CfAccount, CfZone, TunnelConfig } from "@/types/tunnel";

const TOKEN_HELP_URL = "https://dash.cloudflare.com/profile/api-tokens";

/** Single DNS label: lowercase letters/digits, hyphens inside, 1-63 chars. */
const SUBDOMAIN_RE = /^[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/;

interface CfBindFlowProps {
  localHost: string;
  localPort: number;
  /** Called after the tunnel was provisioned and started; parent closes the wizard. */
  onProvisioned: (tunnel: TunnelConfig) => void;
  /** True when the local target is not ready yet: steps stay usable, but the
      final provision button is disabled with `disabledReason` shown on top. */
  disabled?: boolean;
  disabledReason?: string;
}

type BindStep = 1 | 2 | 3;
/** Provision progress: create the tunnel object, then route it, then run it. */
type ProvisionPhase = "idle" | "create" | "route" | "start";

export function CfBindFlow({
  localHost,
  localPort,
  onProvisioned,
  disabled = false,
  disabledReason,
}: CfBindFlowProps) {
  const { t } = useTranslation();

  // Entry steps only: 1 = token, 2 = zone + subdomain. Step 3 (provision) is
  // derived — shown as soon as both entries are valid, so editing never fights
  // with an auto-advance effect.
  const [step, setStep] = useState<1 | 2>(1);
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [verifying, setVerifying] = useState(false);

  const [accounts, setAccounts] = useState<CfAccount[]>([]);
  const [zones, setZones] = useState<CfZone[]>([]);
  const [zonesLoading, setZonesLoading] = useState(false);
  const [zonesError, setZonesError] = useState<string | null>(null);

  const [zoneId, setZoneId] = useState("");
  const [subdomain, setSubdomain] = useState("");
  const [subdomainTouched, setSubdomainTouched] = useState(false);

  // Auto-connect the provisioned tunnel when the app launches.
  const [autoStart, setAutoStart] = useState(false);

  const [phase, setPhase] = useState<ProvisionPhase>("idle");
  const routeTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Clear the fake-progress timer when the flow unmounts (dialog closed).
  useEffect(() => {
    return () => {
      if (routeTimer.current) clearTimeout(routeTimer.current);
    };
  }, []);

  const zone = zones.find((z) => z.id === zoneId) ?? null;
  const subdomainValid = SUBDOMAIN_RE.test(subdomain);
  const readyToCreate = !disabled && Boolean(zone) && subdomainValid;
  const activeStep: BindStep = step === 1 ? 1 : readyToCreate ? 3 : 2;

  const STEP_KEYS = [
    t("add.cf.stepToken"),
    t("add.cf.stepZone"),
    t("add.cf.stepCreate"),
  ];

  async function handleVerify() {
    const trimmed = token.trim();
    if (!trimmed) {
      toast.error(t("add.cf.tokenRequired"));
      return;
    }
    setVerifying(true);
    try {
      const list = await api.cfVerifyToken(trimmed);
      setAccounts(list);
      // A different token means a different account — drop stale selections.
      setZoneId("");
      setZones([]);
      setStep(2);
      // Zone loading failures surface inside step 2 (with a retry button),
      // so don't let them masquerade as a verification failure.
      void loadZones(trimmed);
    } catch (error) {
      setAccounts([]);
      toast.error(t("add.cf.verifyFailed"), {
        description: errorMessage(error),
      });
    } finally {
      setVerifying(false);
    }
  }

  async function loadZones(currentToken: string) {
    setZonesLoading(true);
    setZonesError(null);
    try {
      const list = await api.cfListZones(currentToken);
      setZones(list);
    } catch (error) {
      setZonesError(errorMessage(error));
    } finally {
      setZonesLoading(false);
    }
  }

  async function handleProvision() {
    if (!zone || !subdomainValid || phase !== "idle") return;
    setPhase("create");
    // The backend does create + DNS + ingress in one invoke; the labels below
    // are a friendly approximation of that pipeline while it runs.
    routeTimer.current = setTimeout(() => {
      setPhase((prev) => (prev === "create" ? "route" : prev));
    }, 4000);
    try {
      const tunnel = await api.cfProvision({
        token: token.trim(),
        zoneId: zone.id,
        subdomain,
        localHost,
        localPort,
        autoStart,
      });
      if (routeTimer.current) clearTimeout(routeTimer.current);
      setPhase("start");
      const state = await api.startTunnel(tunnel.id);
      upsertConfig(tunnel);
      mergeState(state);
      onProvisioned(tunnel);
    } catch (error) {
      if (routeTimer.current) clearTimeout(routeTimer.current);
      setPhase("idle");
      // e.g. invalid token / subdomain already taken — stay on this step.
      toast.error(t("add.cf.provisionFailed"), {
        description: errorMessage(error),
      });
    }
  }

  const accountName = accounts[0]?.name ?? "";
  const accountNames = new Map(accounts.map((a) => [a.id, a.name]));

  return (
    <div className="flex flex-col gap-3 rounded-xl border bg-muted/20 p-4">
      {/* Panel header + step indicator */}
      <div className="flex items-center gap-2">
        <Globe className="size-4 shrink-0 text-muted-foreground" />
        <p className="text-[13px] font-medium">{t("add.cf.title")}</p>
      </div>
      {disabled && disabledReason && (
        <p className="rounded-md border border-warning/40 bg-warning/10 px-2.5 py-1.5 text-xs text-muted-foreground">
          {disabledReason}
        </p>
      )}
      <ol className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[11px]">
        {STEP_KEYS.map((label, index) => {
          const value = (index + 1) as BindStep;
          const done = activeStep > value;
          const current = activeStep === value;
          return (
            <li key={label} className="flex items-center gap-1.5">
              {index > 0 ? (
                <ChevronRight className="size-3 text-muted-foreground/50" />
              ) : null}
              <span
                className={cn(
                  "inline-flex items-center gap-1 rounded-full px-2 py-0.5",
                  current && "bg-secondary text-secondary-foreground",
                  done && "text-muted-foreground",
                  !current && !done && "text-muted-foreground/60",
                )}
              >
                {done ? (
                  <CheckCircle2 className="size-3 text-emerald-600 dark:text-success" />
                ) : (
                  <span
                    className={cn(
                      "inline-flex size-3.5 items-center justify-center rounded-full border text-[9px] leading-none",
                      current && "border-foreground/60",
                    )}
                  >
                    {value}
                  </span>
                )}
                {label}
              </span>
            </li>
          );
        })}
      </ol>

      {/* Completed-step summary (jump back by clicking edit) */}
      {step > 1 && accounts.length > 0 ? (
        <SummaryRow
          label={t("add.cf.stepToken")}
          onEdit={phase === "idle" ? () => setStep(1) : undefined}
        >
          <CheckCircle2 className="size-3.5 shrink-0 text-emerald-600 dark:text-success" />
          {t("add.cf.accountOk", { name: accountName })}
        </SummaryRow>
      ) : null}

      {/* Step 1: paste + verify API token */}
      {step === 1 ? (
        <div className="flex flex-col gap-2.5">
          <div className="flex flex-col gap-2">
            <Label htmlFor="cf-token">{t("add.cf.tokenLabel")}</Label>
            <div className="flex items-center gap-1">
              <Input
                id="cf-token"
                type={showToken ? "text" : "password"}
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder={t("add.cf.tokenPlaceholder")}
                className="font-mono"
                autoComplete="off"
                spellCheck={false}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="shrink-0 text-muted-foreground hover:text-foreground"
                onClick={() => setShowToken((prev) => !prev)}
                aria-label={showToken ? t("add.cf.hideToken") : t("add.cf.showToken")}
                title={showToken ? t("add.cf.hideToken") : t("add.cf.showToken")}
              >
                {showToken ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
              </Button>
            </div>
          </div>
          <div className="flex flex-col gap-1 text-xs leading-relaxed text-muted-foreground">
            <a
              href={TOKEN_HELP_URL}
              target="_blank"
              rel="noreferrer"
              className="inline-flex w-fit items-center gap-1 font-medium text-primary underline underline-offset-2 hover:opacity-80"
            >
              {t("add.cf.tokenHelp")}
              <ExternalLink className="size-3" />
            </a>
            <p>{t("add.cf.tokenPermissions")}</p>
            <ul className="flex flex-col gap-0.5 font-mono text-[11px]">
              <li>{t("add.cf.permAccountTunnel")}</li>
              <li>{t("add.cf.permZoneDns")}</li>
              <li>{t("add.cf.permZoneRead")}</li>
            </ul>
          </div>
          <Button
            type="button"
            size="sm"
            className="w-fit"
            disabled={verifying}
            onClick={() => void handleVerify()}
          >
            {verifying ? (
              <>
                <Loader2 className="size-3.5 animate-spin" />
                {t("add.cf.verifying")}
              </>
            ) : (
              t("add.cf.verify")
            )}
          </Button>
        </div>
      ) : null}

      {/* Step 2: pick zone + subdomain (live preview) */}
      {step === 2 ? (
        <div className="flex flex-col gap-2.5">
          <div className="flex flex-col gap-2">
            <Label htmlFor="cf-zone">{t("add.cf.zoneLabel")}</Label>
            {zonesLoading ? (
              <p className="inline-flex items-center gap-1.5 text-[13px] text-muted-foreground">
                <Loader2 className="size-3.5 animate-spin" />
                {t("add.cf.zonesLoading")}
              </p>
            ) : zonesError ? (
              <div className="flex flex-col gap-1.5">
                <p className="text-[13px] text-destructive">{t("add.cf.zonesFailed")}</p>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="w-fit"
                  onClick={() => void loadZones(token.trim())}
                >
                  {t("add.cf.zonesRetry")}
                </Button>
              </div>
            ) : zones.length === 0 ? (
              <p className="text-[13px] text-muted-foreground">
                {t("add.cf.zonesEmpty")}
              </p>
            ) : (
              <Select
                value={zoneId}
                onValueChange={(value) => setZoneId(value)}
              >
                <SelectTrigger id="cf-zone" className="w-full">
                  <SelectValue placeholder={t("add.cf.zonePlaceholder")} />
                </SelectTrigger>
                <SelectContent>
                  {zones.map((z) => {
                    const zoneAccount = accountNames.get(z.accountId);
                    const showAccount =
                      zoneAccount && accounts.length > 1 ? zoneAccount : null;
                    return (
                      <SelectItem key={z.id} value={z.id}>
                        <span className="flex flex-col">
                          <span>{z.name}</span>
                          {showAccount ? (
                            <span className="text-xs text-muted-foreground">
                              {showAccount}
                            </span>
                          ) : null}
                        </span>
                      </SelectItem>
                    );
                  })}
                </SelectContent>
              </Select>
            )}
          </div>

          {zone ? (
            <>
              <div className="flex flex-col gap-2">
                <Label htmlFor="cf-subdomain">{t("add.cf.subdomainLabel")}</Label>
                <Input
                  id="cf-subdomain"
                  value={subdomain}
                  onChange={(e) => {
                    setSubdomain(e.target.value.toLowerCase().trim());
                    setSubdomainTouched(true);
                  }}
                  placeholder={t("add.cf.subdomainPlaceholder")}
                  className={cn(
                    "font-mono",
                    subdomainTouched && !subdomainValid && "border-destructive",
                  )}
                  aria-invalid={subdomainTouched && !subdomainValid}
                  autoComplete="off"
                  spellCheck={false}
                />
                {subdomainTouched && !subdomainValid ? (
                  <p className="text-xs text-destructive">
                    {t("add.cf.subdomainInvalid")}
                  </p>
                ) : null}
              </div>
              {/* Live hostname preview */}
              <div className="flex items-center gap-2 rounded-lg border bg-muted/40 px-3 py-2 font-mono text-[13px]">
                <Globe className="size-3.5 shrink-0 text-muted-foreground" />
                <span className="min-w-0 truncate">
                  <span className="text-muted-foreground">https://</span>
                  <span className={cn(subdomainValid ? "text-foreground" : "text-muted-foreground/60")}>
                    {subdomain || "mac"}
                  </span>
                  <span className="text-muted-foreground">.{zone.name}</span>
                </span>
              </div>

              {/* Step 3 (derived): summary + provision CTA, ready to fire */}
              {readyToCreate ? (
                <div className="flex flex-col gap-3 pt-1">
                  <div className="flex flex-col gap-1.5 rounded-lg border bg-background px-3 py-2.5 text-[13px]">
                    <div className="flex items-center justify-between gap-3">
                      <span className="shrink-0 text-muted-foreground">
                        {t("add.cf.summaryHostname")}
                      </span>
                      <span className="min-w-0 truncate font-mono">
                        {subdomain}.{zone.name}
                      </span>
                    </div>
                    <div className="flex items-center justify-between gap-3">
                      <span className="shrink-0 text-muted-foreground">
                        {t("add.cf.summaryLocal")}
                      </span>
                      <span className="min-w-0 truncate font-mono">
                        {localHost}:{localPort}
                      </span>
                    </div>
                  </div>
                  {/* Auto-connect the provisioned tunnel on app launch */}
                  <div className="flex items-start justify-between gap-3">
                    <div className="flex flex-col gap-0.5">
                      <Label htmlFor="cf-autostart">
                        {t("add.autoStart")}
                      </Label>
                      <p className="text-xs leading-relaxed text-muted-foreground">
                        {t("add.autoStartHint")}
                      </p>
                    </div>
                    <Switch
                      id="cf-autostart"
                      checked={autoStart}
                      onCheckedChange={setAutoStart}
                    />
                  </div>
                  <Button
                    type="button"
                    size="sm"
                    className="w-fit"
                    disabled={phase !== "idle"}
                    onClick={() => void handleProvision()}
                  >
                    {phase !== "idle" ? (
                      <>
                        <Loader2 className="size-3.5 animate-spin" />
                        {phase === "create"
                          ? t("add.cf.progressCreate")
                          : phase === "route"
                            ? t("add.cf.progressRoute")
                            : t("add.cf.progressStart")}
                      </>
                    ) : (
                      t("add.cf.create")
                    )}
                  </Button>
                </div>
              ) : null}
            </>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

/** Compact one-line recap of a completed bind step, with an edit shortcut. */
function SummaryRow({
  label,
  children,
  onEdit,
}: {
  label: string;
  children: React.ReactNode;
  onEdit?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-2 rounded-lg border bg-background px-3 py-2 text-[13px]">
      <span className="shrink-0 text-xs text-muted-foreground">{label}</span>
      <span className="flex min-w-0 flex-1 items-center gap-1.5">{children}</span>
      {onEdit ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="shrink-0 text-muted-foreground hover:text-foreground"
          onClick={onEdit}
          aria-label={t("common.edit")}
          title={t("common.edit")}
        >
          <PenLine className="size-3.5" />
        </Button>
      ) : null}
    </div>
  );
}
