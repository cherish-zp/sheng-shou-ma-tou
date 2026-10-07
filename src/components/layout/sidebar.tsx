import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { CircleHelp, Home, Moon, Server, Settings, Sun } from "lucide-react";
import { useTheme } from "next-themes";

import { PierLogo } from "@/components/pier-logo";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

const NAV_ITEMS = [
  { to: "/", icon: Home, labelKey: "nav.tunnels" },
  { to: "/servers", icon: Server, labelKey: "nav.servers" },
  { to: "/help", icon: CircleHelp, labelKey: "nav.help" },
  { to: "/settings", icon: Settings, labelKey: "nav.settings" },
] as const;

export function Sidebar() {
  const { t } = useTranslation();
  const { resolvedTheme, setTheme } = useTheme();
  const location = useLocation();
  const navigate = useNavigate();

  const toggleTheme = () =>
    setTheme(resolvedTheme === "dark" ? "light" : "dark");

  return (
    <aside className="flex h-screen w-[60px] shrink-0 flex-col items-center border-r bg-card/50 py-4">
      <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
        <PierLogo className="size-5" />
      </div>

      <nav className="mt-6 flex flex-1 flex-col items-center gap-1.5">
        {NAV_ITEMS.map(({ to, icon: Icon, labelKey }) => {
          // TooltipTrigger asChild 会把函数 className 直接 toString（Radix
          // mergeProps 的字符串拼接），导致所有导航同时"选中"。改用
          // useLocation 判定 + 字符串 className 的普通按钮。
          const active =
            to === "/" ? location.pathname === "/" : location.pathname.startsWith(to);
          return (
            <Tooltip key={to}>
              <TooltipTrigger asChild>
                <button
                  type="button"
                  onClick={() => navigate(to)}
                  aria-current={active ? "page" : undefined}
                  className={cn(
                    "flex size-9 items-center justify-center rounded-lg transition-colors",
                    active
                      ? "bg-accent text-accent-foreground shadow-sm"
                      : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                  )}
                >
                  <Icon className="size-[18px]" strokeWidth={1.8} />
                </button>
              </TooltipTrigger>
              <TooltipContent side="right">{t(labelKey)}</TooltipContent>
            </Tooltip>
          );
        })}
      </nav>

      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="size-9 text-muted-foreground hover:text-foreground"
            onClick={toggleTheme}
            aria-label={t("settings.appearance")}
          >
            {resolvedTheme === "dark" ? (
              <Sun className="size-[18px]" strokeWidth={1.8} />
            ) : (
              <Moon className="size-[18px]" strokeWidth={1.8} />
            )}
          </Button>
        </TooltipTrigger>
        <TooltipContent side="right">{t("settings.appearance")}</TooltipContent>
      </Tooltip>
    </aside>
  );
}
