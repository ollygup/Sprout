/** The run-status display vocabulary (ticket 68): one home for the status
 * ordering, badge-tone mapping, result grouping, and the install-directory
 * mismatch-note check that the Plan summary and the History detail both
 * render. Pure — no state, no runes. */

import type { RequirementOutcome, RunStatus } from "$lib/types";

/** Display order of the per-status groups in a run's results: good news
 * first, failures last. */
export const statusOrder: RunStatus[] = [
  "installed",
  "upgraded",
  "already_ok",
  "satisfied_by_newer",
  "skipped_unmanaged",
  "failed",
  "timed_out",
];

/** Badge tones for run statuses — shared by Plan and History. */
export type Tone = "accent" | "warm" | "muted" | "info" | "faint" | "danger" | "warn";

export function runStatusTone(status: RunStatus): Tone {
  switch (status) {
    case "installed":
      return "accent";
    case "upgraded":
      return "warm";
    case "already_ok":
      return "muted";
    case "satisfied_by_newer":
      return "info";
    case "skipped_unmanaged":
      return "warn";
    case "failed":
    case "timed_out":
      return "danger";
  }
}

/** Groups a run's outcomes by status in one pass; absent statuses simply
 * have no key. */
export function groupResultsByStatus(
  results: RequirementOutcome[]
): Partial<Record<RunStatus, RequirementOutcome[]>> {
  const groups: Partial<Record<RunStatus, RequirementOutcome[]>> = {};
  for (const result of results) {
    (groups[result.status] ??= []).push(result);
  }
  return groups;
}

/** The worker flags an installer that ignored the requested directory with
 * a phrase in the outcome detail (ticket 34, ADR-0009). */
export function hasMismatchNote(detail: string): boolean {
  return detail.includes("ignored the requested directory");
}
