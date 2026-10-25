import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { mount, unmount } from "svelte";
import DragGrip from "./components/DragGrip.svelte";
import DragOverlay from "./components/DragOverlay.svelte";
import { createCollectionDrag } from "./collectionDrag.svelte";
import { resolveGroupDrop } from "./dropTargets";
import type { Group } from "./types";

const source = (relative: string) =>
  readFileSync(new URL(relative, import.meta.url), "utf8");

const LAUNCH = source("../routes/+page.svelte");
const CLIPS = source("../routes/clips/+page.svelte");
const ACTIONS = source("../routes/quick-actions/+page.svelte");
const DOCK = source("../routes/quick-launch-window/+page.svelte");
const ACCORDION = source("./components/GroupAccordion.svelte");
const CONTROLLER = source("./collectionDrag.svelte.ts");
const ICON = source("./components/Icon.svelte");
const GROUPS = source("./collectionGroups.svelte.ts");

describe("drag grip component", () => {
  it("is a labelled button with the six-dot glyph", () => {
    const host = document.createElement("div");
    const instance = mount(DragGrip, {
      target: host,
      props: { label: "Drag Work to move it.", onstart: () => {} },
    });
    const button = host.querySelector("button");
    expect(button).toBeTruthy();
    expect(button?.getAttribute("type")).toBe("button");
    expect(button?.getAttribute("aria-label")).toBe("Drag Work to move it.");
    expect(button?.querySelector("svg circle")).toBeTruthy();
    unmount(instance);
    host.remove();
  });

  it("starts a pointer drag and opens the menu only from a keyboard click", () => {
    const host = document.createElement("div");
    let started = 0;
    let menu = 0;
    const instance = mount(DragGrip, {
      target: host,
      props: {
        label: "Drag Work to move it.",
        onstart: () => started++,
        onmenu: () => menu++,
      },
    });
    const button = host.querySelector("button") as HTMLButtonElement;
    button.dispatchEvent(
      new PointerEvent("pointerdown", {
        bubbles: true,
        pointerType: "mouse",
        button: 0,
      }),
    );
    expect(started).toBe(1);
    // Enter/Space produce a click with detail 0.
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 0 }));
    expect(menu).toBe(1);
    // The click that ends a pointer drag must not open the menu.
    button.dispatchEvent(new MouseEvent("click", { bubbles: true, detail: 1 }));
    expect(menu).toBe(1);
    unmount(instance);
    host.remove();
  });
});

describe("drag overlay", () => {
  it("shows the ghost, the insertion line, and the refusal copy", () => {
    const host = document.createElement("div");
    const instance = mount(DragOverlay, {
      target: host,
      props: {
        drag: {
          kind: "group" as const,
          id: 2,
          label: "Builds",
          x: 40,
          y: 60,
          valid: false,
          reason: "Groups only go two levels deep.",
          line: { left: 10, top: 20, width: 120 },
        },
        message: "Groups only go two levels deep.",
      },
    });
    expect(host.textContent).toContain("Builds");
    expect(host.textContent).toContain("two levels deep");
    expect(host.querySelector(".drag-line")).toBeTruthy();
    expect(
      host.querySelector('[aria-live="polite"]')?.textContent,
    ).toContain("two levels deep");
    unmount(instance);
    host.remove();
  });
});

describe("drag wiring across the three collections", () => {
  it("gives every page a grip, a top-level drop area, and empty-group targets", () => {
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain("createCollectionDrag");
      expect(page).toContain("<DragGrip");
      expect(page).toContain("<DragOverlay");
      expect(page).toContain('t("drag.grip")');
      expect(page).toContain("data-drop-row");
      expect(page).toContain("data-drop-group=");
      expect(page).toContain("data-drop-top-level");
      expect(page).toContain("data-drop-empty-group");
    }
  });

  it("commits each collection through its own transactional place_item call", () => {
    expect(LAUNCH).toContain('placeItem("launch"');
    expect(CLIPS).toContain('placeItem("clip"');
    expect(ACTIONS).toContain('placeItem("action"');
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain("resolveItemDrop");
      expect(page).toContain("resolveGroupDrop");
      expect(page).toContain("reorderGroup");
    }
  });

  it("publishes group identity for nesting and sibling slots", () => {
    expect(ACCORDION).toContain("data-drop-group-section");
    expect(ACCORDION).toContain("data-drop-group-header");
    expect(ACCORDION).toContain("data-group-parent");
  });

  it("keeps drag out of the dock and leaves the menu as the keyboard path", () => {
    expect(DOCK).not.toContain("DragGrip");
    expect(DOCK).not.toContain("createCollectionDrag");
    expect(GROUPS).toContain('t("menu.moveInto")');
    expect(GROUPS).toContain("moveInto(");
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain('t("menu.moveInto")');
    }
  });

  it("refuses drags while filtering and keeps the grip disabled there", () => {
    expect(CONTROLLER).toContain("filtering");
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain("disabled={reorderBlocked}");
    }
  });
});

describe("drag mechanics", () => {
  it("uses a 4 px threshold, pointer events, Esc, and edge auto-scroll", () => {
    expect(CONTROLLER).toContain("START_THRESHOLD = 4");
    expect(CONTROLLER).toContain('addEventListener("pointermove"');
    expect(CONTROLLER).toContain('addEventListener("pointerup"');
    expect(CONTROLLER).toContain('addEventListener("pointercancel"');
    expect(CONTROLLER).toContain('event.key === "Escape"');
    expect(CONTROLLER).toContain("requestAnimationFrame");
    expect(CONTROLLER).toContain("EDGE_BAND");
    expect(CONTROLLER).toContain("cleanup()");
  });

  it("honors reduced motion and keeps the grip reachable on pen/touch", () => {
    expect(source("./components/DragGrip.svelte")).toContain(
      "prefers-reduced-motion",
    );
    expect(source("./components/DragGrip.svelte")).toContain(
      "(hover: none)",
    );
    expect(source("./components/DragGrip.svelte")).toContain("touch-action: none");
    expect(source("./components/DragOverlay.svelte")).not.toContain("transition:");
  });

  it("treats the grip strip as part of the card's hover area", () => {
    const grip = source("./components/DragGrip.svelte");
    // The strip spans the host's full height and stays hit-testable while
    // invisible, so moving from the card to the handle never drops the hover.
    expect(grip).toContain("left: -24px");
    expect(grip).toContain("bottom: 0");
    expect(grip).not.toContain("pointer-events: none");
    // No second highlight: the card's own hover plus the grab cursor is it.
    expect(grip).not.toContain("background: var(--bg-hover)");
    for (const host of [LAUNCH, CLIPS, ACTIONS, ACCORDION]) {
      expect(host).not.toContain("pointer-events: auto");
    }
  });

  it("ships a six-dot grip glyph in the shared icon set", () => {
    expect(ICON).toContain("grip:");
    expect(ICON).toContain('cx="9" cy="6"');
    expect(ICON).toContain('cx="15" cy="18"');
  });
});

describe("group drop wiring (ticket 233)", () => {
  const GROUPS: Group[] = [
    { id: 1, collection: "launch", name: "AFWS", color: null, parent_id: null },
    { id: 2, collection: "launch", name: "TST", color: null, parent_id: null },
  ];

  function harness() {
    const area = document.createElement("div");
    area.dataset.dropTopLevel = "";
    const source = document.createElement("section");
    source.dataset.dropGroupSection = "1";
    const sourceHeader = document.createElement("div");
    sourceHeader.dataset.dropGroupHeader = "1";
    source.append(sourceHeader);
    const target = document.createElement("section");
    target.dataset.dropGroupSection = "2";
    const targetHeader = document.createElement("div");
    targetHeader.dataset.dropGroupHeader = "2";
    target.append(targetHeader);
    area.append(source, target);
    document.body.append(area);

    const point = document.createElement("span");
    targetHeader.append(point);
    const original = document.elementFromPoint;
    document.elementFromPoint = () => point;

    const groupCommits: unknown[] = [];
    const controller = createCollectionDrag({
      enabled: () => true,
      resolveItem: () => ({ ok: false, reason: "no-target" }),
      resolveGroup: (dragId, target) =>
        resolveGroupDrop({
          dragId,
          groups: GROUPS,
          filtering: false,
          overHeaderId: target.overHeaderId,
          overSlot: target.overSlot,
          overTopLevelArea: target.overTopLevelArea,
        }),
      commitItem: () => {},
      commitGroup: (_id, drop) => groupCommits.push(drop),
    });

    return {
      controller,
      groupCommits,
      restore() {
        document.elementFromPoint = original;
        area.remove();
      },
    };
  }

  it("commits a nest when a group is released over another group's header", () => {
    const { controller, groupCommits, restore } = harness();
    controller.start(
      new PointerEvent("pointerdown", {
        pointerType: "mouse",
        button: 0,
        clientX: 10,
        clientY: 10,
      }),
      { kind: "group", id: 1, label: "AFWS" },
    );
    window.dispatchEvent(new PointerEvent("pointermove", { clientX: 30, clientY: 40 }));
    window.dispatchEvent(new PointerEvent("pointerup", { clientX: 30, clientY: 40 }));
    expect(groupCommits).toEqual([{ ok: true, parentId: 2, position: 0 }]);
    restore();
  });

  function rectAt(top: number, height: number): DOMRect {
    return {
      top,
      bottom: top + height,
      left: 0,
      right: 400,
      width: 400,
      height,
      x: 0,
      y: top,
      toJSON: () => ({}),
    } as DOMRect;
  }

  /** A top-level target whose card body spans y 128–300, header above it. */
  function bodyHarness() {
    const area = document.createElement("div");
    area.dataset.dropTopLevel = "";
    const source = document.createElement("section");
    source.dataset.dropGroupSection = "1";
    source.append(document.createElement("div"));
    const target = document.createElement("section");
    target.dataset.dropGroupSection = "2";
    const targetHeader = document.createElement("div");
    targetHeader.dataset.dropGroupHeader = "2";
    targetHeader.getBoundingClientRect = () => rectAt(100, 28);
    const body = document.createElement("div");
    body.className = "card-body";
    target.append(targetHeader, body);
    target.getBoundingClientRect = () => rectAt(100, 200);
    area.append(source, target);
    document.body.append(area);

    const original = document.elementFromPoint;
    document.elementFromPoint = () => body;

    const groupCommits: unknown[] = [];
    const controller = createCollectionDrag({
      enabled: () => true,
      resolveItem: () => ({ ok: false, reason: "no-target" }),
      resolveGroup: (dragId, target) =>
        resolveGroupDrop({
          dragId,
          groups: GROUPS,
          filtering: false,
          overHeaderId: target.overHeaderId,
          overSlot: target.overSlot,
          overTopLevelArea: target.overTopLevelArea,
        }),
      commitItem: () => {},
      commitGroup: (_id, drop) => groupCommits.push(drop),
    });

    return {
      controller,
      groupCommits,
      restore() {
        document.elementFromPoint = original;
        area.remove();
      },
    };
  }

  function dragTo(controller: ReturnType<typeof createCollectionDrag>, y: number) {
    controller.start(
      new PointerEvent("pointerdown", {
        pointerType: "mouse",
        button: 0,
        clientX: 10,
        clientY: 10,
      }),
      { kind: "group", id: 1, label: "AFWS" },
    );
    window.dispatchEvent(new PointerEvent("pointermove", { clientX: 30, clientY: y }));
    window.dispatchEvent(new PointerEvent("pointerup", { clientX: 30, clientY: y }));
  }

  it("nests when a group is released on another group's card body", () => {
    const { controller, groupCommits, restore } = bodyHarness();
    dragTo(controller, 160);
    expect(groupCommits).toEqual([{ ok: true, parentId: 2, position: 0 }]);
    restore();
  });

  it("still reorders on the thin band at the card's bottom edge", () => {
    const { controller, groupCommits, restore } = bodyHarness();
    dragTo(controller, 292);
    expect(groupCommits).toEqual([{ ok: true, parentId: null, position: 1 }]);
    restore();
  });

  it("marks the header under a group drag so the target can show itself", () => {
    const { controller, restore } = harness();
    controller.start(
      new PointerEvent("pointerdown", {
        pointerType: "mouse",
        button: 0,
        clientX: 10,
        clientY: 10,
      }),
      { kind: "group", id: 1, label: "AFWS" },
    );
    window.dispatchEvent(new PointerEvent("pointermove", { clientX: 30, clientY: 40 }));
    const target = document.querySelector<HTMLElement>(
      '[data-drop-group-section="2"]',
    );
    expect(target?.dataset.dropNest).toBe("true");
    window.dispatchEvent(new PointerEvent("pointerup", { clientX: 30, clientY: 40 }));
    expect(target?.dataset.dropNest).toBeUndefined();
    restore();
  });
});

describe("nested group copy and removal (tickets 230–233)", () => {
  it("explains what removing a parent versus a child does", () => {
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain('t("dialog.removeParentBody")');
      expect(page).toContain('t("dialog.removeChildBody")');
      expect(page).toContain("groups.removingHasChildren");
    }
  });

  it("keeps Move into… reachable at both levels", () => {
    for (const page of [LAUNCH, CLIPS, ACTIONS]) {
      expect(page).toContain("openGroupMenu(");
      expect(page).toContain("parentId={section.group.id}");
    }
  });
});
