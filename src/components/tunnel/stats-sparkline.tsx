// Minimal inline SVG sparkline for a tunnel's recent traffic: two thin
// polylines (in = success color, out = weakened foreground), no chart lib.

import { useTranslation } from "react-i18next";

import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { sparklineGeometry, type StatsSample } from "@/lib/stats-history";
import { cn } from "@/lib/utils";

const WIDTH = 120;
const HEIGHT = 28;

interface StatsSparklineProps {
  history: readonly StatsSample[];
  className?: string;
}

export function StatsSparkline({ history, className }: StatsSparklineProps) {
  const { t } = useTranslation();
  const geometry = sparklineGeometry(history, WIDTH, HEIGHT);
  if (!geometry) return null;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <svg
          width={WIDTH}
          height={HEIGHT}
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          className={cn("shrink-0 overflow-visible", className)}
          role="img"
          aria-label={t("stats.sparklineTitle")}
        >
          {/* Outbound first: weakened, sits underneath the inbound line. */}
          <polyline
            points={geometry.outPoints}
            fill="none"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeLinejoin="round"
            className="stroke-foreground/30"
          />
          <polyline
            points={geometry.inPoints}
            fill="none"
            strokeWidth={1.5}
            strokeLinecap="round"
            strokeLinejoin="round"
            className="stroke-emerald-600 dark:stroke-success"
          />
        </svg>
      </TooltipTrigger>
      <TooltipContent side="top">{t("stats.sparklineTitle")}</TooltipContent>
    </Tooltip>
  );
}
