import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useTheme } from "next-themes";
import { enable, disable, isEnabled } from "@tauri-apps/plugin-autostart";
import { Cloud, LoaderCircle, Monitor, Moon, Radio, Sun } from "lucide-react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import {
  SUPPORTED_LANGUAGES,
  changeAppLanguage,
  type AppLanguage,
} from "@/i18n";
import { cn, errorMessage } from "@/lib/utils";
import {
  installEngine,
  refreshBinaryStatus,
  useTunnelStore,
} from "@/store/tunnel-store";
import type { Backend, BinaryInfo } from "@/types/tunnel";

const THEME_OPTIONS = [
  { value: "system", icon: Monitor, labelKey: "settings.themeSystem" },
  { value: "light", icon: Sun, labelKey: "settings.themeLight" },
  { value: "dark", icon: Moon, labelKey: "settings.themeDark" },
] as const;

const LANGUAGE_LABELS: Record<AppLanguage, string> = {
  "zh-CN": "中文（简体）",
  en: "English",
};

const ENGINE_META: Record<
  Backend,
  { icon: typeof Cloud; nameKey: string; descKey: string }
> = {
  cloudflare: {
    icon: Cloud,
    nameKey: "settings.engineCloudflare",
    descKey: "settings.engineCloudflareDescription",
  },
  bore: {
    icon: Radio,
    nameKey: "settings.engineBore",
    descKey: "settings.engineBoreDescription",
  },
  frp: {
    icon: Radio,
    nameKey: "settings.engineFrp",
    descKey: "settings.engineFrpDescription",
  },
};

export function SettingsPage() {
  const { t, i18n } = useTranslation();
  const { theme, setTheme } = useTheme();
  const { binaryStatus, installing } = useTunnelStore();

  useEffect(() => {
    void refreshBinaryStatus();
  }, []);

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col px-8 py-10">
      <header>
        <h1 className="text-xl font-semibold tracking-tight">
          {t("settings.title")}
        </h1>
      </header>

      <div className="mt-8 flex flex-col gap-6">
        {/* Appearance */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">
              {t("settings.appearance")}
            </CardTitle>
            <CardDescription>
              {t("settings.appearanceDescription")}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="inline-flex items-center gap-1 rounded-lg border bg-muted/40 p-1">
              {THEME_OPTIONS.map(({ value, icon: Icon, labelKey }) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => setTheme(value)}
                  className={cn(
                    "inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-[13px] transition-all",
                    theme === value
                      ? "bg-background text-foreground shadow-sm"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  <Icon className="size-3.5" strokeWidth={1.8} />
                  {t(labelKey)}
                </button>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* Language */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t("settings.language")}</CardTitle>
            <CardDescription>
              {t("settings.languageDescription")}
            </CardDescription>
          </CardHeader>
          <CardContent>
            <div className="flex items-center justify-between">
              <Label htmlFor="language-select">{t("settings.language")}</Label>
              <Select
                value={i18n.language.startsWith("zh") ? "zh-CN" : "en"}
                onValueChange={(value) => {
                  void changeAppLanguage(value as AppLanguage);
                }}
              >
                <SelectTrigger id="language-select" className="w-44">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {SUPPORTED_LANGUAGES.map((lang) => (
                    <SelectItem key={lang} value={lang}>
                      {LANGUAGE_LABELS[lang]}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </CardContent>
        </Card>

        {/* Autostart */}
        <AutostartCard />

        {/* Engines */}
        <Card>
          <CardHeader>
            <CardTitle className="text-base">
              {t("settings.engines")}
            </CardTitle>
            <CardDescription>
              {t("settings.enginesDescription")}
            </CardDescription>
          </CardHeader>
          <CardContent>
            {(["cloudflare", "bore"] as const).map((backend, index) => (
              <div key={backend}>
                {index > 0 ? <Separator className="my-4" /> : null}
                <EngineRow
                  backend={backend}
                  info={binaryStatus?.[backend]}
                  installing={installing[backend]}
                />
              </div>
            ))}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

function AutostartCard() {
  const { t } = useTranslation();
  const [enabled, setEnabled] = useState(false);
  const [available, setAvailable] = useState(true);

  useEffect(() => {
    isEnabled()
      .then((value) => {
        setEnabled(value);
        setAvailable(true);
      })
      .catch(() => {
        setAvailable(false);
        toast.error(t("settings.autostartFailed"));
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  async function handleToggle(next: boolean) {
    const previous = enabled;
    setEnabled(next);
    try {
      if (next) await enable();
      else await disable();
    } catch (error) {
      setEnabled(previous);
      toast.error(t("settings.autostartChangeFailed"), {
        description: errorMessage(error),
      });
    }
  }

  return (
    <Card>
      <CardContent className="flex items-center justify-between gap-6 py-5">
        <div className="flex flex-col gap-1">
          <p className="text-sm font-medium leading-none">
            {t("settings.autostart")}
          </p>
          <p className="text-sm text-muted-foreground">
            {t("settings.autostartDescription")}
          </p>
        </div>
        <Switch
          checked={enabled}
          disabled={!available}
          onCheckedChange={handleToggle}
          aria-label={t("settings.autostart")}
        />
      </CardContent>
    </Card>
  );
}

function EngineRow({
  backend,
  info,
  installing,
}: {
  backend: Backend;
  info?: BinaryInfo;
  installing: boolean;
}) {
  const { t } = useTranslation();
  const { icon: Icon, nameKey, descKey } = ENGINE_META[backend];
  const installed = info?.installed ?? false;

  return (
    <div className="flex items-center gap-4">
      <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-secondary text-secondary-foreground">
        <Icon className="size-4.5" strokeWidth={1.8} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium leading-none">{t(nameKey)}</p>
          {info ? (
            installed ? (
              <Badge
                variant="outline"
                className="border-emerald-600/30 bg-emerald-600/10 text-[11px] text-emerald-700 dark:border-success/30 dark:bg-success/10 dark:text-success"
              >
                {t("settings.installed")}
              </Badge>
            ) : (
              <Badge
                variant="outline"
                className="text-[11px] text-muted-foreground"
              >
                {t("settings.notInstalled")}
              </Badge>
            )
          ) : null}
        </div>
        <p className="truncate text-[13px] text-muted-foreground">
          {t(descKey)}
          {installed && info?.version ? (
            <span className="ml-2 font-mono text-xs">
              v{info.version}
              {info.path ? (
                <span className="ml-2 hidden text-muted-foreground/60 lg:inline">
                  {info.path}
                </span>
              ) : null}
            </span>
          ) : null}
        </p>
      </div>
      <Button
        variant="outline"
        size="sm"
        disabled={installing || !info}
        onClick={() => void installEngine(backend)}
      >
        {installing ? (
          <>
            <LoaderCircle className="size-3.5 animate-spin" />
            {t("settings.installing")}
          </>
        ) : (
          t(installed ? "settings.reinstall" : "settings.install")
        )}
      </Button>
    </div>
  );
}
