// Simple markdown-lite for Quick Action notes (ticket 118).
//
// Scope (AC): paragraphs, `-`/`*` bullet lists, `1.` ordered lists;
// everything else escapes verbatim and renders as plain text.
//
// Rendering via escaped HTML — the consumer uses {@html formatNote(note)}.
// All visual styling lives in CSS tokens; this module returns structure only.
//
// Research refs:
// - 0006 pattern 14 — existence glyphs are content-gated; formatter itself is
//   gated by hasNote() so empty notes leave no trace.
// - 0004 rule 3 — compact surfaces show only the glyph, content stays out.

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** True when the note carries visible content — trimmed non-empty. */
export function hasNote(note: string | null | undefined): boolean {
  return typeof note === "string" && note.trim().length > 0;
}

/**
 * Renders the authored plain text to safe HTML.
 *
 * Blocks are separated by blank lines. Within a block:
 * - consecutive `- ` / `* ` lines → `<ul>`
 * - consecutive `1. ` / `2. ` … lines → `<ol>`
 * - otherwise → `<p>` (single-newline lines inside a paragraph join with a space)
 *
 * Every text fragment is HTML-escaped; no markdown beyond the two list
 * grammars is recognized, so headings, bold, code, etc. appear verbatim.
 */
export function formatNote(raw: string | null | undefined): string {
  if (!hasNote(raw)) return "";
  const normalized = raw!.replace(/\r\n/g, "\n").trim();
  const lines = normalized.split("\n");

  let html = "";
  let inList: "ul" | "ol" | null = null;
  let paragraphLines: string[] = [];

  function flushParagraph(): void {
    if (paragraphLines.length === 0) return;
    const joined = paragraphLines.join(" ").trim();
    if (joined) html += `<p>${escapeHtml(joined)}</p>`;
    paragraphLines = [];
  }

  function closeList(): void {
    if (inList) {
      html += `</${inList}>`;
      inList = null;
    }
  }

  for (const rawLine of lines) {
    const trimmed = rawLine.trim();
    if (trimmed === "") {
      flushParagraph();
      closeList();
      continue;
    }

    const bullet = trimmed.match(/^[-*]\s+(.+)$/);
    const ordered = trimmed.match(/^\d+\.\s+(.+)$/);

    if (bullet) {
      flushParagraph();
      if (inList !== "ul") {
        closeList();
        html += "<ul>";
        inList = "ul";
      }
      html += `<li>${escapeHtml(bullet[1])}</li>`;
      continue;
    }

    if (ordered) {
      flushParagraph();
      if (inList !== "ol") {
        closeList();
        html += "<ol>";
        inList = "ol";
      }
      html += `<li>${escapeHtml(ordered[1])}</li>`;
      continue;
    }

    // Plain paragraph line — close any list, accumulate until blank or list.
    closeList();
    paragraphLines.push(trimmed);
  }

  flushParagraph();
  closeList();
  return html;
}
