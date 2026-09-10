import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import ContextMenu from "./components/ContextMenu.svelte";

const cleanups: Array<() => void> = [];

afterEach(() => {
  for (const fn of cleanups.splice(0)) fn();
});

function nextFrame() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => resolve());
  });
}

function mountMenu(onclose: () => void) {
  const host = document.createElement("div");
  document.body.appendChild(host);

  // Trigger mirrors the companion site trigger: the dataset lives on the
  // <button>, while pointer targets land on inner spans / icon svg.
  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.setAttribute("data-ctx-trigger", "");
  const text = document.createElement("span");
  text.textContent = "Work";
  const chevron = document.createElement("span");
  chevron.setAttribute("aria-hidden", "true");
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  chevron.appendChild(svg);
  trigger.appendChild(text);
  trigger.appendChild(chevron);
  host.appendChild(trigger);

  const instance = mount(ContextMenu, {
    target: host,
    props: {
      ctx: {
        open: true,
        items: [{ label: "Work — https://work.example" }],
        label: "Saved Companion sites",
        anchor: trigger,
        returnTo: trigger,
      },
      onclose,
    },
  });
  cleanups.push(() => {
    unmount(instance);
    host.remove();
  });
  return { host, trigger, text, svg };
}

function pointerDown(el: Element) {
  el.dispatchEvent(new Event("pointerdown", { bubbles: true, cancelable: true }));
}

describe("ContextMenu trigger clicks", () => {
  it("treats pointerdown on trigger inner content as a trigger click, not outside", async () => {
    const onclose = vi.fn();
    const { text, svg } = mountMenu(onclose);
    await tick();
    await nextFrame();

    pointerDown(text);
    await tick();
    pointerDown(svg);
    await tick();

    expect(onclose).not.toHaveBeenCalled();
  });

  it("still closes on a genuine outside pointerdown", async () => {
    const onclose = vi.fn();
    mountMenu(onclose);
    await tick();
    await nextFrame();

    pointerDown(document.body);
    await tick();

    expect(onclose).toHaveBeenCalledTimes(1);
  });
});

describe("ContextMenu anchored alignment", () => {
  function rect(overrides: Partial<DOMRect>): DOMRect {
    return {
      x: 0,
      y: 0,
      width: 0,
      height: 0,
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      toJSON: () => {},
      ...overrides,
    } as DOMRect;
  }

  async function mountAlignedMenu(opts: {
    align?: "start" | "end";
    matchAnchorWidth?: boolean;
  }) {
    const host = document.createElement("div");
    document.body.appendChild(host);
    const trigger = document.createElement("button");
    host.appendChild(trigger);
    const instance = mount(ContextMenu, {
      target: host,
      props: {
        ctx: {
          open: true,
          items: [{ label: "Zoom out companion" }],
          label: "More Companion actions",
          anchor: trigger,
          returnTo: trigger,
          placement: "above",
          ...opts,
        },
        onclose: () => {},
      },
    });
    const menu = host.querySelector(".ctx-menu") as HTMLElement;
    vi.spyOn(trigger, "getBoundingClientRect").mockReturnValue(
      rect({ left: 100, right: 400, top: 200, bottom: 230, width: 300, height: 30, x: 100, y: 200 }),
    );
    vi.spyOn(menu, "getBoundingClientRect").mockReturnValue(
      rect({ width: 200, height: 150 }),
    );
    cleanups.push(() => {
      unmount(instance);
      host.remove();
    });
    await tick();
    await nextFrame();
    await nextFrame();
    return menu;
  }

  it("right-aligns to the anchor by default (⋯ row menus unchanged)", async () => {
    const menu = await mountAlignedMenu({});
    expect(menu.style.left).toBe("200px");
    expect(menu.style.minWidth).toBe("");
  });

  it("left-aligns and stretches to the anchor when asked (select-style picker)", async () => {
    const menu = await mountAlignedMenu({ align: "start", matchAnchorWidth: true });
    expect(menu.style.left).toBe("100px");
    expect(menu.style.minWidth).toBe("300px");
  });
});
