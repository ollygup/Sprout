import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { dialogSubmitAction } from "./dialogSubmit";
import type { DialogKeyModifiers } from "./dialogSubmit";

const PLAIN: DialogKeyModifiers = { ctrl: false, meta: false, shift: false, alt: false };

const DIALOG_SOURCE = readFileSync(
  new URL("./components/Dialog.svelte", import.meta.url),
  "utf8",
);

describe("dialogSubmitAction", () => {
  it("submits plain Enter from single-line text inputs", () => {
    for (const type of ["text", "search", "url", "tel", "email", "number", "password"]) {
      expect(dialogSubmitAction({ tagName: "INPUT", type }, "Enter", PLAIN)).toBe("submit");
    }
  });

  it("leaves non-text inputs and other elements to native behavior", () => {
    for (const type of ["checkbox", "radio", "file", "range", "submit", "button"]) {
      expect(dialogSubmitAction({ tagName: "INPUT", type }, "Enter", PLAIN)).toBe("none");
    }
    expect(dialogSubmitAction({ tagName: "BUTTON" }, "Enter", PLAIN)).toBe("none");
    expect(dialogSubmitAction({ tagName: "SELECT" }, "Enter", PLAIN)).toBe("none");
    expect(dialogSubmitAction({ tagName: "A" }, "Enter", PLAIN)).toBe("none");
  });

  it("keeps textarea Enter as newline, submits on Ctrl/Cmd+Enter", () => {
    expect(dialogSubmitAction({ tagName: "TEXTAREA" }, "Enter", PLAIN)).toBe("none");
    expect(
      dialogSubmitAction({ tagName: "TEXTAREA" }, "Enter", { ...PLAIN, ctrl: true }),
    ).toBe("submit");
    expect(
      dialogSubmitAction({ tagName: "TEXTAREA" }, "Enter", { ...PLAIN, meta: true }),
    ).toBe("submit");
    expect(
      dialogSubmitAction({ tagName: "TEXTAREA" }, "Enter", { ...PLAIN, alt: true }),
    ).toBe("none");
  });

  it("leaves modified Enter in text inputs to the platform", () => {
    expect(
      dialogSubmitAction({ tagName: "INPUT", type: "text" }, "Enter", {
        ...PLAIN,
        ctrl: true,
      }),
    ).toBe("none");
    expect(
      dialogSubmitAction({ tagName: "INPUT", type: "text" }, "Enter", {
        ...PLAIN,
        alt: true,
      }),
    ).toBe("none");
  });

  it("ignores non-Enter keys and missing targets", () => {
    expect(
      dialogSubmitAction({ tagName: "INPUT", type: "text" }, "Escape", PLAIN),
    ).toBe("none");
    expect(dialogSubmitAction(null, "Enter", PLAIN)).toBe("none");
  });

  it("is owned once by the shared Dialog", () => {
    expect(DIALOG_SOURCE).toMatch(/dialogSubmitAction/);
    expect(DIALOG_SOURCE).toMatch(/requestSubmit/);
  });
});
