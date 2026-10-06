// Inline diagnosis result card. The diagnosis `code` is a stable slug that
// maps to `diagnosis.<code>.{title,detail,suggestions}` i18n keys; the raw
// `detail` from the backend (English log evidence) is shown in muted mono.

import { useTranslation } from "react-i18next";
import {
  CircleAlert,
  Info,
  TriangleAlert,
} from "lucide-react";

import { cn } from "@/lib/utils";
import type { Diagnosis, DiagnosisLevel } from "@/types/tunnel";

/** Codes with full i18n coverage; anything else falls back to genericError. */
const KNOWN_CODES = new Set([
  "tokenMismatch",
  "authFailed",
  "versionMismatch",
  "portConflict",
  "connectionRefused",
  "dnsFailed",
  "localServiceDown",
  "binaryMissing",
  "remoteServerUnreachable",
  "genericError",
  "allHealthy",
]);

const LEVEL_STYLES: Record<
  DiagnosisLevel,
  { box: string; icon: typeof Info; iconClass: string; titleClass: string }
> = {
  error: {
    box: "border-destructive/30 bg-destructive/10",
    icon: CircleAlert,
    iconClass: "text-destructive",
    titleClass: "text-destructive",
  },
  warn: {
    box: "border-amber-500/40 bg-amber-500/10",
    icon: TriangleAlert,
    iconClass: "text-amber-600 dark:text-warning",
    titleClass: "text-amber-700 dark:text-warning",
  },
  info: {
    box: "border-sky-500/40 bg-sky-500/10",
    icon: Info,
    iconClass: "text-sky-600 dark:text-sky-400",
    titleClass: "text-sky-700 dark:text-sky-400",
  },
};

interface DiagnosisCardProps {
  diagnosis: Diagnosis;
}

export function DiagnosisCard({ diagnosis }: DiagnosisCardProps) {
  const { t } = useTranslation();
  const code = KNOWN_CODES.has(diagnosis.code) ? diagnosis.code : "genericError";
  const level = diagnosis.level in LEVEL_STYLES ? diagnosis.level : "info";
  const { box, icon: Icon, iconClass, titleClass } = LEVEL_STYLES[level];

  const title = t(`diagnosis.code.${code}.title`);
  const explanation = t(`diagnosis.code.${code}.detail`);
  const suggestions = t(`diagnosis.code.${code}.suggestions`, {
    returnObjects: true,
  }) as string[];

  return (
    <div className={cn("rounded-lg border px-3.5 py-3", box)}>
      <div className="flex items-center gap-2">
        <Icon className={cn("size-4 shrink-0", iconClass)} />
        <p className={cn("text-[13px] font-medium", titleClass)}>{title}</p>
      </div>

      <p className="mt-1.5 text-[13px] leading-relaxed text-muted-foreground">
        {explanation}
      </p>

      {diagnosis.detail ? (
        <div className="mt-2.5">
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70">
            {t("diagnosis.detailLabel")}
          </p>
          <pre className="pier-scroll mt-1 max-h-32 overflow-y-auto rounded bg-muted/50 px-2.5 py-2 font-mono text-xs leading-relaxed break-all whitespace-pre-wrap text-muted-foreground/80">
            {diagnosis.detail}
          </pre>
        </div>
      ) : null}

      {Array.isArray(suggestions) && suggestions.length > 0 ? (
        <div className="mt-2.5">
          <p className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70">
            {t("diagnosis.suggestionsLabel")}
          </p>
          <ul className="mt-1 flex flex-col gap-1">
            {suggestions.map((suggestion, index) => (
              <li
                key={index}
                className="flex items-start gap-2 text-[13px] leading-relaxed"
              >
                <span
                  className={cn(
                    "mt-[7px] size-1 shrink-0 rounded-full",
                    level === "error"
                      ? "bg-destructive/70"
                      : level === "warn"
                        ? "bg-amber-500/70 dark:bg-warning/70"
                        : "bg-sky-500/70 dark:bg-sky-400/70",
                  )}
                />
                <span>{suggestion}</span>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
