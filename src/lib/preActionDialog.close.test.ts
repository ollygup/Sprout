import { afterEach, describe, expect, it } from "vitest";
import { mount, tick } from "svelte";
import QuickActionFormDialog from "./components/QuickActionFormDialog.svelte";
import type { QuickAction } from "./types";

const hosts: HTMLElement[] = [];

afterEach(() => {
  for (const host of hosts.splice(0)) host.remove();
});

function baseAction(
  overrides: Partial<Omit<QuickAction, "id" | "group_id">> = {}
): QuickAction {
  return {
    id: 7,
    group_id: null,
    name: "docker start",
    shell: "powershell",
    command: "docker compose up -d",
    cwd: null,
    stoppable: false,
    stop_command: null,
    note: null,
    auto_run: false,
    show_in_dock: true,
    ...overrides,
  };
}

function mountForm(action: QuickAction | null) {
  const host = document.createElement("div");
  document.body.appendChild(host);
  hosts.push(host);
  mount(QuickActionFormDialog, {
    target: host,
    props: { open: true, action, onsave: () => {}, oncancel: () => {} },
  });
  return host;
}

async function click(el: Element | null) {
  expect(el).not.toBeNull();
  el!.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  await tick();
}

function disclosure(host: HTMLElement, label: string): Element | null {
  const buttons = [...host.querySelectorAll("button")];
  return (
    buttons.find((b) => b.textContent?.trim() === label && b.hasAttribute("aria-expanded")) ??
    null
  );
}

function typeInto(host: HTMLElement, selector: string, text: string) {
  const el = host.querySelector(selector) as
    | HTMLTextAreaElement
    | HTMLInputElement
    | null;
  expect(el).not.toBeNull();
  el!.value = text;
  el!.dispatchEvent(new Event("input", { bubbles: true }));
}

async function openPreAction(host: HTMLElement) {
  await click(disclosure(host, "Details"));
  await click(disclosure(host, "Pre-action"));
}

/** The Pre-action gate lives behind Details, collapsed until needed. */
describe("Pre-action section", () => {
  it("stays collapsed inside Details on Add", async () => {
    const host = mountForm(null);
    await tick();
    const detailsBody = host.querySelector("#qa-details-body");
    expect(detailsBody?.hasAttribute("hidden")).toBe(true);
    await click(disclosure(host, "Details"));
    const preBody = host.querySelector("#qa-preaction-body");
    expect(preBody).not.toBeNull();
    expect(preBody!.hasAttribute("hidden")).toBe(true);
  });

  it("expands to check, fix, and a Check affordance", async () => {
    const host = mountForm(null);
    await tick();
    await openPreAction(host);
    expect(host.querySelector("#qa-pre-check")).not.toBeNull();
    expect(host.querySelector("#qa-pre-fix")).not.toBeNull();
    const buttons = [...host.querySelectorAll("button")];
    expect(buttons.some((b) => b.textContent?.trim() === "Check")).toBe(true);
  });

  it("prefills check-only and check+fix on Edit", async () => {
    const host = mountForm(
      baseAction({ pre_check: "node --version", pre_fix: "npm install" })
    );
    await tick();
    await openPreAction(host);
    expect((host.querySelector("#qa-pre-check") as HTMLTextAreaElement).value).toBe(
      "node --version"
    );
    expect((host.querySelector("#qa-pre-fix") as HTMLTextAreaElement).value).toBe(
      "npm install"
    );
  });

  it("refuses a fix without a check with a plain error", async () => {
    const host = mountForm(null);
    await tick();
    typeInto(host, "#qa-name", "docker start");
    typeInto(host, "#qa-command", "docker compose up -d");
    await openPreAction(host);
    typeInto(host, "#qa-pre-fix", "npm install");
    await tick();
    host
      .querySelector("form")!
      .dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await tick();
    expect(host.querySelector(".form__error")?.textContent).toBe(
      "A pre-action fix needs a pre-action check — add a check command or clear the fix."
    );
  });

  it("keeps Check disabled while the check is empty", async () => {
    const host = mountForm(null);
    await tick();
    await openPreAction(host);
    const check = [...host.querySelectorAll("button")].find(
      (b) => b.textContent?.trim() === "Check"
    );
    // Empty check keeps the affordance disabled — nothing to run.
    expect(check?.hasAttribute("disabled")).toBe(true);
  });
});
