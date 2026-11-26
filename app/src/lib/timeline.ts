// Timeline model: snapshots (oldest first) turned into points on the time bar.

import type { Snapshot } from "./api";

export interface TimePoint {
  /** Snapshot shown when this point is selected (the first of a run). */
  snapshot: Snapshot;
  /** Last snapshot of the run of identical content (same as `snapshot` if not grouped). */
  last: Snapshot;
  /** Number of snapshots represented by this point. */
  count: number;
  /** Start time in ms. */
  time: number;
  /** Content differs from the previous point (false only when showing every snapshot). */
  changed: boolean;
}

/**
 * Groups **consecutive** snapshots with the same root into one point.
 *
 * Kopia's web UI groups by root ID globally, which hides a return to earlier content
 * (A → B → A would show two points). On a timeline only consecutive runs make sense, so
 * A → B → A shows three. See docs/API.md §5.
 */
export function buildPoints(snapshots: Snapshot[], group: boolean): TimePoint[] {
  const points: TimePoint[] = [];
  let prevRoot: string | null = null;
  for (const s of snapshots) {
    const time = Date.parse(s.startTime);
    const changed = s.rootId !== prevRoot;
    const last = points[points.length - 1];
    if (group && last && !changed) {
      last.last = s;
      last.count += 1;
    } else {
      points.push({ snapshot: s, last: s, count: 1, time, changed });
    }
    prevRoot = s.rootId;
  }
  return points;
}

/** Index of the point closest in time to `time` (ms); a run covers its first to last snapshot. */
export function indexNearest(points: TimePoint[], time: number): number {
  let best = points.length - 1;
  let bestDiff = Infinity;
  points.forEach((p, i) => {
    const lo = p.time;
    const hi = Date.parse(p.last.startTime);
    const diff = time < lo ? lo - time : time > hi ? time - hi : 0;
    if (diff < bestDiff) {
      best = i;
      bestDiff = diff;
    }
  });
  return best;
}
