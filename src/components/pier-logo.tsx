import { cn } from "@/lib/utils";

/** Pier logo: a simple stone pier / bridge arch drawn with strokes. */
export function PierLogo({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={cn("size-6", className)}
    >
      <path d="M3.5 19.5v-7a8.5 8.5 0 0 1 17 0v7" />
      <path d="M8.5 19.5v-5.2a3.5 3.5 0 0 1 7 0v5.2" />
      <path d="M12 4.2v2.3" />
      <path d="M2.5 19.5h19" />
    </svg>
  );
}

/** Decorative empty-state illustration: an arch bridge over water. */
export function BridgeIllustration({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 200 96"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={cn("text-muted-foreground/50", className)}
    >
      {/* deck */}
      <path d="M14 44h172" />
      {/* main arch */}
      <path d="M40 44a60 60 0 0 1 120 0" />
      {/* inner arch */}
      <path d="M64 44a36 36 0 0 1 72 0" />
      {/* suspension struts */}
      <path d="M52 20.5v23.5M76 10v34M100 6v38M124 10v34M148 20.5v23.5" />
      {/* piers */}
      <path d="M40 44v34M160 44v34" />
      {/* water */}
      <path d="M22 84c10-5 20-5 30 0s20 5 30 0 20-5 30 0 20 5 30 0 20-5 30 0" opacity="0.6" />
      <path d="M40 90c8-4 16-4 24 0s16 4 24 0 16-4 24 0 16 4 24 0" opacity="0.35" />
    </svg>
  );
}
