import { useTranslation } from "react-i18next";
import { NavLink } from "react-router-dom";
import { Home, Moon, Settings, Sun } from "lucide-react";
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
  { to: "/settings", icon: Settings, labelKey: "nav.settings" },
] as const;

export function Sidebar() {
  const { t } = useTranslation();
  const { resolvedTheme, setTheme } = useTheme();

  const toggleTheme = () =>
    setTheme(resolvedTheme === "dark" ? "light" : "dark");

  return (
    <aside className="flex h-screen w-[60px] shrink-0 flex-col items-center border-r bg-card/50 py-4">
      <div className="flex size-9 items-center justify-center rounded-lg bg-primary text-primary-foreground shadow-sm">
        <PierLogo className="size-5" />
      </div>

      <nav className="mt-6 flex flex-1 flex-col items-center gap-1.5">
        {NAV_ITEMS.map(({ to, icon: Icon, labelKey }) => (
          <Tooltip key={to}>
            <TooltipTrigger asChild>
              <NavLink
                to={to}
                end={to === "/"}
                className={({ isActive }) =>
                  cn(
                    "flex size-9 items-center justify-center rounded-lg transition-colors",
                    isActive
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                  )
                }
              >
                <Icon className="size-[18px]" strokeWidth={1.8} />
              </NavLink>
            </TooltipTrigger>
            <TooltipContent side="right">{t(labelKey)}</TooltipContent>
          </Tooltip>
        ))}
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
