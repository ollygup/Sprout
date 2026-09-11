import { afterEach, describe, expect, it, vi } from "vitest";
import { mount, tick, unmount } from "svelte";
import CommandFormDialog from "./components/CommandFormDialog.svelte";
import { createLaunchEntry, testLaunchCommand } from "./api";

vi.mock("./api", () => ({
  createLaunchEntry: vi.fn().mockResolvedValue({}),
  testLaunchCommand: vi.fn(),
}));

const mounted: ReturnType<typeof mount>[] = [];
afterEach(async () => {
  for (const component of mounted.splice(0)) await unmount(component);
  document.body.replaceChildren();
  vi.clearAllMocks();
});

async function openForm() {
  const host = document.createElement("div");
  document.body.append(host);
  const oncancel = vi.fn();
  const onsave = vi.fn();
  mounted.push(mount(CommandFormDialog, {
    target: host,
    props: { open: true, onsave, oncancel },
  }));
  await tick();
  return { host, oncancel, onsave };
}

describe("Quick Launch command disclosure", () => {
  it("starts with the command fields and hides optional settings and execution help", async () => {
    const { host } = await openForm();
    expect(host.querySelector("#command-line")?.closest("[hidden]")).toBeNull();
    expect(host.querySelector("#command-name")?.closest("[hidden]")).toBeNull();
    const showWindow = [...host.querySelectorAll("label")].find((label) => label.textContent?.includes("Show a window"));
    expect(showWindow?.closest("[hidden]")).toBeTruthy();
    const disclosure = [...host.querySelectorAll("button")].find((button) => button.textContent?.trim() === "Details");
    expect(disclosure?.getAttribute("aria-expanded")).toBe("false");
    disclosure!.click();
    await tick();
    expect(showWindow?.closest("[hidden]")).toBeNull();
    expect(disclosure?.getAttribute("aria-expanded")).toBe("true");
  });

  it("keeps entered values through disclosure changes and saves without testing", async () => {
    const { host, onsave } = await openForm();
    const command = host.querySelector("#command-line") as HTMLTextAreaElement;
    command.value = "Get-Process";
    command.dispatchEvent(new Event("input", { bubbles: true }));
    await tick();
    expect((host.querySelector("#command-name") as HTMLInputElement).value).toBe("Get-Process");
    const details = [...host.querySelectorAll("button")].find((button) => button.textContent?.trim() === "Details")!;
    details.click();
    await tick();
    const flags = [...host.querySelectorAll<HTMLInputElement>('input[type="checkbox"]')];
    flags[0].checked = true;
    flags[0].dispatchEvent(new Event("change", { bubbles: true }));
    details.click();
    await tick();
    host.querySelector("form")!.dispatchEvent(new Event("submit", { bubbles: true, cancelable: true }));
    await vi.waitFor(() => expect(onsave).toHaveBeenCalled());
    expect(createLaunchEntry).toHaveBeenCalledExactlyOnceWith({
      name: "Get-Process", kind: "command", target: "Get-Process", shell: "powershell",
      show_window: true, desktop_id: null, show_in_dock: true,
    });
    expect(testLaunchCommand).not.toHaveBeenCalled();
  });
});
