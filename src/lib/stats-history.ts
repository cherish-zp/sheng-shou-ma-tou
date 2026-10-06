// Pure helpers for the per-tunnel traffic history ring buffer and the
// sparkline geometry rendered on the tunnel card. No React, no Tauri —
// everything here is plain data-in/data-out so it can be reasoned about
// (and tested) without a browser.

/** One sampled point of a tunnel's cumulative traffic counter. */
export interface StatsSample {
  /** Wall-clock ms (Date.now()) at sampling time. */
  t: number;
  /** Cumulative inbound bytes reported by the backend. */
  bytesIn: number;
  /** Cumulative outbound bytes reported by the backend. */
  bytesOut: number;
}

/** Ring buffer capacity: keep the last minute of per-second samples. */
export const STATS_HISTORY_LIMIT = 60;

/**
 * Append a sample to the history ring buffer (immutably):
 * - replaces the trailing sample when it carries the same timestamp,
 * - otherwise appends and trims to the last `limit` entries.
 * Always returns a new array so useSyncExternalStore snapshots stay stable
 * between real changes.
 */
export function pushSample(
  history: readonly StatsSample[],
  sample: StatsSample,
  limit: number = STATS_HISTORY_LIMIT,
): StatsSample[] {
  const last = history[history.length - 1];
  if (last && last.t === sample.t) {
    return [...history.slice(0, -1), sample];
  }
  const next = [...history, sample];
  return next.length > limit ? next.slice(next.length - limit) : next;
}

export interface SparklineGeometry {
  /** SVG polyline points for the inbound series. */
  inPoints: string;
  /** SVG polyline points for the outbound series. */
  outPoints: string;
  /** Max per-interval value both series were scaled against. */
  max: number;
}

/**
 * Turn cumulative-counter samples into two normalized polylines that fit a
 * `width` x `height` box. Consecutive samples are differenced into
 * per-interval throughput (counter resets are clamped to zero), then both
 * series share one scale so they stay comparable.
 *
 * Returns null when there are fewer than two samples (nothing to draw yet).
 */
export function sparklineGeometry(
  history: readonly StatsSample[],
  width: number,
  height: number,
): SparklineGeometry | null {
  if (history.length < 2 || width <= 0 || height <= 0) return null;

  // First point has no delta; pin it to the baseline so the line spans the
  // full width.
  const inValues: number[] = [0];
  const outValues: number[] = [0];
  for (let i = 1; i < history.length; i += 1) {
    inValues.push(Math.max(0, history[i].bytesIn - history[i - 1].bytesIn));
    outValues.push(Math.max(0, history[i].bytesOut - history[i - 1].bytesOut));
  }

  let max = 0;
  for (const value of inValues) if (value > max) max = value;
  for (const value of outValues) if (value > max) max = value;
  if (max <= 0) max = 1; // idle traffic: draw a flat baseline

  const step = width / (history.length - 1);
  // Leave room for the stroke so peaks are not clipped.
  const top = 1.5;
  const bottom = height - 1.5;
  const toY = (value: number) => bottom - (value / max) * (bottom - top);

  const toPoints = (values: number[]) =>
    values
      .map((value, i) => `${(i * step).toFixed(1)},${toY(value).toFixed(1)}`)
      .join(" ");

  return {
    inPoints: toPoints(inValues),
    outPoints: toPoints(outValues),
    max,
  };
}
