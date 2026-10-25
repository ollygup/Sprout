/// The shared drag & drop controller (tickets 232–233). All three main-app
/// collections start drags through this one controller, so the pointer
/// mechanics — a 4 px threshold, the ghost and insertion line, edge
/// auto-scroll, Esc cancel, and full cleanup — exist once and behave the
/// same on every page.
///
/// The controller deliberately owns only *mechanics* and DOM hit-testing.
/// What a drop means is decided by the pure `dropTargets` module (the
/// page closes over its own item order and group list), and the page commits
/// the resolved drop through the transactional API. Grips are the only
/// drag origin; the ⋯ menu stays the complete keyboard path.
///
/// Pointer events are used throughout, so mouse, pen, and 100–200 % DPI all
/// share one code path (client coordinates are CSS pixels; nothing reads
/// `devicePixelRatio`). The ghost and line are plain fixed-position boxes
/// with no animated transitions, which is the reduced-motion-safe form.

import type {
  DropReason,
  GroupDrop,
  GroupDropSuccess,
  ItemDrop,
  ItemDropSuccess,
} from "./dropTargets";
import { t } from "./copy";

export type DragKind = "item" | "group";

export interface DragPayload {
  kind: DragKind;
  id: number;
  label: string;
}

/** What sits under the pointer, in the pure resolver's vocabulary. */
export interface DragTarget {
  overRow: { id: number; groupId: number | null; after: boolean } | null;
  hasAppendTarget: boolean;
  appendGroupId: number | null;
  overHeaderId: number | null;
  overSlot: { parentId: number | null; index: number } | null;
  overTopLevelArea: boolean;
}

export interface DragLine {
  left: number;
  top: number;
  width: number;
}

export interface DragState {
  kind: DragKind;
  id: number;
  label: string;
  x: number;
  y: number;
  valid: boolean;
  /** The refusal copy shown beside the ghost; null when valid or silent. */
  reason: string | null;
  line: DragLine | null;
}

export interface CollectionDragOptions {
  /** False while a filter is active — drags never reorder through hidden rows. */
  enabled: () => boolean;
  resolveItem: (dragId: number, target: DragTarget) => ItemDrop;
  resolveGroup: (dragId: number, target: DragTarget) => GroupDrop;
  commitItem: (dragId: number, drop: ItemDropSuccess) => void;
  commitGroup: (dragId: number, drop: GroupDropSuccess) => void;
}

const START_THRESHOLD = 4;
const EDGE_BAND = 40;
const MAX_STEP = 18;
/** The thin band at a top-level card's body top and bottom that still means
 *  "insert before/after this group" rather than "nest into it". */
const SLOT_EDGE = 16;

const EMPTY_TARGET: DragTarget = {
  overRow: null,
  hasAppendTarget: false,
  appendGroupId: null,
  overHeaderId: null,
  overSlot: null,
  overTopLevelArea: false,
};

function groupIdOf(raw: string | undefined): number | null {
  return raw === undefined || raw === "" ? null : Number(raw);
}

/** Short, calm refusal copy — the same line is announced to screen readers. */
export function dropReasonText(reason: DropReason): string {
  switch (reason) {
    case "filtering":
      return t("drag.reason.filtering");
    case "depth":
      return t("drag.reason.depth");
    case "cycle":
      return t("drag.reason.cycle");
    case "has-children":
      return t("drag.reason.hasChildren");
    case "cross-collection":
      return t("drag.reason.crossCollection");
    case "collision":
      return t("drag.reason.collision");
    case "no-op":
    case "no-target":
      return "";
  }
}

export function createCollectionDrag(options: CollectionDragOptions) {
  let drag = $state<DragState | null>(null);
  let message = $state("");
  let messageTimer: ReturnType<typeof setTimeout> | null = null;

  // Mechanics that must never trigger reactivity live in plain variables.
  let pending: {
    payload: DragPayload;
    startX: number;
    startY: number;
    pointerId: number;
  } | null = null;
  let scrollFrame = 0;
  let suppressClicksUntil = 0;
  let nestSection: HTMLElement | null = null;

  function announce(text: string) {
    if (!text) return;
    message = text;
    if (messageTimer) clearTimeout(messageTimer);
    messageTimer = setTimeout(() => {
      message = "";
      messageTimer = null;
    }, 4000);
  }

  function siblingIndex(id: number, parentId: number | null): number {
    const sections = Array.from(
      document.querySelectorAll<HTMLElement>("[data-drop-group-section]"),
    ).filter(
      (section) => groupIdOf(section.dataset.groupParent) === parentId,
    );
    const index = sections.findIndex(
      (section) => Number(section.dataset.dropGroupSection) === id,
    );
    return index < 0 ? sections.length : index;
  }

  function lineOf(element: Element, after: boolean): DragLine {
    const rect = element.getBoundingClientRect();
    return {
      left: rect.left,
      top: after ? rect.bottom : rect.top,
      width: rect.width,
    };
  }

  /** The group that would take the drop, marked on the section so the
   *  accordion can show "this becomes the parent". A header hit alone is a
   *  small target with no line to draw, so without this the only feedback is
   *  the ghost (ticket 233). */
  function markNest(section: HTMLElement | null) {
    if (nestSection === section) return;
    if (nestSection) delete nestSection.dataset.dropNest;
    nestSection = section;
    if (nestSection) nestSection.dataset.dropNest = "true";
  }

  function hitTest(
    kind: DragKind,
    x: number,
    y: number,
  ): { target: DragTarget; line: DragLine | null; nest: HTMLElement | null } {
    const element = document.elementFromPoint(x, y);
    if (!element) return { target: EMPTY_TARGET, line: null, nest: null };

    if (kind === "item") {
      const row = element.closest<HTMLElement>("[data-drop-row]");
      if (row) {
        const rect = row.getBoundingClientRect();
        const after = y > rect.top + rect.height / 2;
        return {
          target: {
            ...EMPTY_TARGET,
            overRow: {
              id: Number(row.dataset.dropRow),
              groupId: groupIdOf(row.dataset.dropGroup),
              after,
            },
          },
          line: lineOf(row, after),
          nest: null,
        };
      }
      const append = element.closest<HTMLElement>(
        "[data-drop-group-header],[data-drop-empty-group]",
      );
      if (append) {
        const raw =
          append.dataset.dropGroupHeader ?? append.dataset.dropEmptyGroup;
        return {
          target: {
            ...EMPTY_TARGET,
            hasAppendTarget: true,
            appendGroupId: groupIdOf(raw),
          },
          line: lineOf(append, true),
          nest: null,
        };
      }
      if (element.closest("[data-drop-top-level]")) {
        return {
          target: { ...EMPTY_TARGET, hasAppendTarget: true, appendGroupId: null },
          line: null,
          nest: null,
        };
      }
      return { target: EMPTY_TARGET, line: null, nest: null };
    }

    // Groups: a header nests, a top-level card's body nests too (dropping
    // "onto a group" is the common gesture — silently reordering beside it was
    // the wrong answer, ticket 233), a child's body reorders at its level, and
    // the page's empty area unnests.
    const header = element.closest<HTMLElement>("[data-drop-group-header]");
    if (header) {
      return {
        target: {
          ...EMPTY_TARGET,
          overHeaderId: Number(header.dataset.dropGroupHeader),
        },
        line: null,
        nest: header.closest<HTMLElement>("[data-drop-group-section]"),
      };
    }
    const section = element.closest<HTMLElement>("[data-drop-group-section]");
    if (section) {
      const id = Number(section.dataset.dropGroupSection);
      const parentId = groupIdOf(section.dataset.groupParent);
      const rect = section.getBoundingClientRect();

      if (parentId !== null) {
        // A child is never a parent, so its body keeps the sibling slot.
        const after = y > rect.top + rect.height / 2;
        return {
          target: {
            ...EMPTY_TARGET,
            overSlot: {
              parentId,
              index: siblingIndex(id, parentId) + (after ? 1 : 0),
            },
          },
          line: lineOf(section, after),
          nest: null,
        };
      }

      // A top-level card nests everywhere except the two thin bands that sit
      // just under its header and at its bottom edge: those are the insertion
      // slots that reorder the top level.
      const headerEl = section.querySelector<HTMLElement>(
        "[data-drop-group-header]",
      );
      const bodyTop = headerEl
        ? headerEl.getBoundingClientRect().bottom
        : rect.top;
      if (y <= bodyTop + SLOT_EDGE) {
        return {
          target: {
            ...EMPTY_TARGET,
            overSlot: { parentId: null, index: siblingIndex(id, null) },
          },
          line: lineOf(section, false),
          nest: null,
        };
      }
      if (y >= rect.bottom - SLOT_EDGE) {
        return {
          target: {
            ...EMPTY_TARGET,
            overSlot: { parentId: null, index: siblingIndex(id, null) + 1 },
          },
          line: lineOf(section, true),
          nest: null,
        };
      }
      return {
        target: { ...EMPTY_TARGET, overHeaderId: id },
        line: null,
        nest: section,
      };
    }
    if (element.closest("[data-drop-top-level]")) {
      return {
        target: { ...EMPTY_TARGET, overTopLevelArea: true },
        line: null,
        nest: null,
      };
    }
    return { target: EMPTY_TARGET, line: null, nest: null };
  }

  function resolve(payload: DragPayload, target: DragTarget) {
    return payload.kind === "item"
      ? options.resolveItem(payload.id, target)
      : options.resolveGroup(payload.id, target);
  }

  function updateTarget(x: number, y: number) {
    if (!drag) return;
    const { target, line, nest } = hitTest(drag.kind, x, y);
    const result = resolve(
      { kind: drag.kind, id: drag.id, label: drag.label },
      target,
    );
    drag.valid = result.ok;
    drag.reason = result.ok ? null : dropReasonText(result.reason) || null;
    drag.line = result.ok ? line : null;
    markNest(result.ok && target.overHeaderId != null ? nest : null);
    document.body.classList.toggle("sprout-drag-invalid", !result.ok);
  }

  function stepScroll() {
    scrollFrame = 0;
    if (!drag) return;
    const scroller = document.scrollingElement ?? document.documentElement;
    const before = scroller.scrollTop;
    let step = 0;
    if (drag.y < EDGE_BAND) {
      step = -Math.ceil((EDGE_BAND - drag.y) / 3);
    } else if (drag.y > window.innerHeight - EDGE_BAND) {
      step = Math.ceil((drag.y - (window.innerHeight - EDGE_BAND)) / 3);
    }
    if (step !== 0) {
      scroller.scrollTop = before + Math.max(-MAX_STEP, Math.min(MAX_STEP, step));
    }
    // Keep scrolling while the pointer rests in the band, and re-test the
    // target so the line follows the content that moved under it.
    if (step !== 0 && scroller.scrollTop !== before) {
      updateTarget(drag.x, drag.y);
      scrollFrame = requestAnimationFrame(stepScroll);
    }
  }

  function startScroll() {
    if (!scrollFrame && drag) scrollFrame = requestAnimationFrame(stepScroll);
  }

  function onMove(event: PointerEvent) {
    if (!pending) return;
    if (!drag) {
      const dx = event.clientX - pending.startX;
      const dy = event.clientY - pending.startY;
      if (Math.hypot(dx, dy) < START_THRESHOLD) return;
      drag = {
        kind: pending.payload.kind,
        id: pending.payload.id,
        label: pending.payload.label,
        x: event.clientX,
        y: event.clientY,
        valid: true,
        reason: null,
        line: null,
      };
      document.body.classList.add("sprout-dragging");
      document.body.dataset.dragKind = pending.payload.kind;
      if (pending.payload.kind === "group") {
        // The source header must not advertise itself as a target.
        document
          .querySelector<HTMLElement>(
            `[data-drop-group-section="${pending.payload.id}"]`,
          )
          ?.setAttribute("data-drop-source", "true");
      }
    }
    drag.x = event.clientX;
    drag.y = event.clientY;
    updateTarget(event.clientX, event.clientY);
    startScroll();
  }

  function onUp() {
    if (!pending) return;
    const payload = pending.payload;
    const wasActive = drag !== null;
    if (wasActive && drag) {
      // Re-resolve at release: the tree may have re-rendered under the pointer.
      const { target } = hitTest(drag.kind, drag.x, drag.y);
      const result = resolve(payload, target);
      if (result.ok) {
        if (payload.kind === "item") {
          options.commitItem(payload.id, result as ItemDropSuccess);
        } else {
          options.commitGroup(payload.id, result as GroupDropSuccess);
        }
      } else if (result.reason !== "no-op" && result.reason !== "no-target") {
        announce(dropReasonText(result.reason));
      }
      // A drag that ends over a row must not also run that row.
      suppressClicksUntil = performance.now() + 250;
    }
    cleanup();
  }

  function onKey(event: KeyboardEvent) {
    if (event.key === "Escape" && (drag || pending)) {
      event.preventDefault();
      event.stopPropagation();
      cleanup();
    }
  }

  function cleanup() {
    pending = null;
    drag = null;
    if (scrollFrame) cancelAnimationFrame(scrollFrame);
    scrollFrame = 0;
    markNest(null);
    for (const source of document.querySelectorAll<HTMLElement>(
      "[data-drop-source]",
    )) {
      delete source.dataset.dropSource;
    }
    document.body.classList.remove("sprout-dragging", "sprout-drag-invalid");
    delete document.body.dataset.dragKind;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", cleanup);
    window.removeEventListener("keydown", onKey, true);
    window.removeEventListener("blur", cleanup);
  }

  function start(event: PointerEvent, payload: DragPayload) {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    if (!options.enabled()) {
      announce(t("drag.reason.filtering"));
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    pending = {
      payload,
      startX: event.clientX,
      startY: event.clientY,
      pointerId: event.pointerId,
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", cleanup);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", cleanup);
  }

  // One capture-phase guard per controller: a completed drag swallows the
  // click it would otherwise deliver to the row underneath.
  document.addEventListener(
    "click",
    (event) => {
      if (performance.now() < suppressClicksUntil) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
    true,
  );

  return {
    get drag() {
      return drag;
    },
    get message() {
      return message;
    },
    start,
    cancel: cleanup,
  };
}
