import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { render } from "svelte/server";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";

const DIALOG_SOURCE = readFileSync(
  new URL("./components/QuickActionFormDialog.svelte", import.meta.url),
  "utf8",
);

/// The AI section lives between its comment marker and the working-directory
/// field that follows it.
function aiSection(): string {
  const start = DIALOG_SOURCE.indexOf("<!-- AI drafting");
  const end = DIALOG_SOURCE.indexOf('id="qa-cwd"');
  expect(start).toBeGreaterThan(-1);
  expect(end).toBeGreaterThan(start);
  return DIALOG_SOURCE.slice(start, end);
}

describe("QuickActionFormDialog AI section", () => {
  it("renders the Add dialog open with the AI section and no throw", () => {
    const { body } = render(QuickActionFormDialog, {
      props: {
        open: true,
        action: null,
        aiReady: true,
        onsave: vi.fn(),
        oncancel: vi.fn(),
      },
    });
    expect(body).toContain("Add a quick action");
    expect(body).toContain("Draft with AI");
    expect(body).toContain("qa-ai-request");
  });

  it("keeps the shared close wiring — the dialog still closes via oncancel", () => {
    expect(DIALOG_SOURCE).toContain("onclose={oncancel}");
    expect(DIALOG_SOURCE).toContain("onclick={oncancel}");
  });

  it("gives every AI button type button so none can submit the form", () => {
    const section = aiSection();
    const buttons = section.match(/<Button\b/g) ?? [];
    expect(buttons.length).toBeGreaterThan(0);
    expect(section).not.toMatch(/<Button(?![^>]*type="button")/);
  });

  it("leaves Escape alone — the AI key handler only reroutes Ctrl/Cmd+Enter", () => {
    // Ticket 167 removed the permanent generate-shortcut teaching line; the
    // grammar itself lives in aiKeydown and must stay Ctrl/Cmd+Enter-only.
    const handlerStart = DIALOG_SOURCE.indexOf("function aiKeydown");
    const handlerEnd = DIALOG_SOURCE.indexOf("async function submit");
    expect(handlerStart).toBeGreaterThan(-1);
    expect(handlerEnd).toBeGreaterThan(handlerStart);
    const handler = DIALOG_SOURCE.slice(handlerStart, handlerEnd);
    expect(handler).toContain("Enter");
    expect(handler).not.toContain("Escape");
    expect(DIALOG_SOURCE).not.toContain("stopPropagation");
  });

  it("adds no overlay mechanics that could swallow close clicks", () => {
    const section = aiSection();
    expect(section).not.toContain("pointer-events");
    expect(section).not.toMatch(/position:\s*(fixed|absolute)/);
  });
});
