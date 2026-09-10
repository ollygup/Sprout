import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick } from "svelte";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";
import ConfirmProbe from "./confirmProbe.svelte";

const hosts: HTMLElement[] = [];

afterEach(() => {
  for (const host of hosts.splice(0)) host.remove();
});

function mountForm() {
  const onsave = vi.fn();
  const oncancel = vi.fn();
  const host = document.createElement("div");
  document.body.appendChild(host);
  hosts.push(host);
  mount(QuickActionFormDialog, {
    target: host,
    props: { open: true, action: null, onsave, oncancel },
  });
  return { onsave, oncancel, host };
}

function mountConfirm() {
  const onconfirm = vi.fn();
  const oncancel = vi.fn();
  const host = document.createElement("div");
  document.body.appendChild(host);
  hosts.push(host);
  mount(ConfirmProbe, {
    target: host,
    props: { onconfirm, oncancel },
  });
  return { onconfirm, oncancel, host };
}

async function click(el: Element | null) {
  expect(el).not.toBeNull();
  el!.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  await tick();
}

describe("Add Quick Action dialog closes", () => {
  it("mounts quiet with no interaction", async () => {
    mountForm();
    await tick();
    await tick();
  }, 15000);

  it("Cancel closes it", async () => {
    const { oncancel, host } = mountForm();
    await tick();
    const buttons = [...host.querySelectorAll("button")];
    const cancel = buttons.find((b) => b.textContent?.trim() === "Cancel");
    await click(cancel ?? null);
    expect(oncancel).toHaveBeenCalled();
  });

  it("the header X closes it", async () => {
    const { oncancel, host } = mountForm();
    await tick();
    await click(host.querySelector('[aria-label="Close dialog"]'));
    expect(oncancel).toHaveBeenCalled();
  });

  it("Escape closes it", async () => {
    const { oncancel, host } = mountForm();
    await tick();
    const dialog = host.querySelector("dialog");
    expect(dialog).not.toBeNull();
    dialog!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
    await tick();
    expect(oncancel).toHaveBeenCalled();
  });
});

describe("Dialog base control (other popups)", () => {
  it("ConfirmDialog Cancel closes it", async () => {
    const { oncancel, host } = mountConfirm();
    await tick();
    const buttons = [...host.querySelectorAll("button")];
    const cancel = buttons.find((b) => b.textContent?.trim() === "Cancel");
    await click(cancel ?? null);
    expect(oncancel).toHaveBeenCalled();
  });

  it("ConfirmDialog header X closes it", async () => {
    const { oncancel, host } = mountConfirm();
    await tick();
    await click(host.querySelector('[aria-label="Close dialog"]'));
    expect(oncancel).toHaveBeenCalled();
  });
});
