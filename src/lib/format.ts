/** Shared display helpers for dates, durations, and byte sizes — used by the
 * History and Logs screens (ticket 09). */

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** i;
  return `${value >= 100 || i === 0 ? Math.round(value) : value.toFixed(1)} ${units[i]}`;
}

/** Epoch seconds → "14 Aug 2026, 14:32" in the machine's locale. */
export function formatTimestamp(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Epoch seconds → "14 Aug, 14:32" (year dropped — runs are rarely years
 * old). */
export function formatTimestampShort(epochSeconds: number): string {
  return new Date(epochSeconds * 1000).toLocaleString(undefined, {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Seconds → "3m 12s", "45s", or "2h 5m". */
export function formatDuration(seconds: number): string {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  if (hours > 0) return `${hours}h ${minutes}m`;
  if (minutes > 0) return `${minutes}m ${secs}s`;
  return `${secs}s`;
}

/** A Clip's display title (ticket 78): the stored name when titled, else the
 *  content's first non-blank line — untitled clips stay readable without
 *  inventing names. Empty only for content that is blank overall. */
export function clipTitle(name: string, content: string): string {
  const titled = name.trim();
  if (titled) return titled;
  return (
    content.split(/\r?\n/).map((line) => line.trim()).find((line) => line) ??
    ""
  );
}

/** The end-of-run wording shared by the Quick Launch page's flash notice and
 *  the Quick Launch window's status line (ticket 42): started / skipped /
 *  failed counts, skipped entries with their reasons (ticket 48), and the
 *  desktop-assignment notes (ticket 44) — a no-op run is never silent. */
export function launchReportSummary(report: {
  started: string[];
  skipped: string[];
  failed: string[];
  notes: string[];
}): string {
  const counts = [
    `started ${report.started.length}`,
    `skipped ${report.skipped.length}`,
    `failed ${report.failed.length}`,
  ];
  const skipped =
    report.skipped.length > 0 ? ` Skipped: ${report.skipped.join(", ")}.` : "";
  const failed =
    report.failed.length > 0 ? ` Failed: ${report.failed.join(", ")}.` : "";
  const notes = report.notes.length > 0 ? ` ${report.notes.join(". ")}.` : "";
  return `Quick Launch done — ${counts.join(", ")}.${skipped}${failed}${notes}`;
}