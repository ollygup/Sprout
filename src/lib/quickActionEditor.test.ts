import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import {
  applyCompletion,
  filesCompletion,
  filesHint,
  tokenizeQuickActionCommand,
} from "./quickActionEditor";

const DIALOG_SOURCE = readFileSync(
  new URL("./components/QuickActionFormDialog.svelte", import.meta.url),
  "utf8",
);
const API_SOURCE = readFileSync(new URL("./api.ts", import.meta.url), "utf8");
const TYPES_SOURCE = readFileSync(new URL("./types.ts", import.meta.url), "utf8");
const PACKAGE = JSON.parse(
  readFileSync(new URL("../../package.json", import.meta.url), "utf8"),
) as { dependencies: Record<string, string> };

function kindsOf(command: string, shell: "powershell" | "cmd"): string[] {
  return tokenizeQuickActionCommand(command, shell).map((t) => t.kind);
}

function textsOf(command: string, shell: "powershell" | "cmd"): string[] {
  return tokenizeQuickActionCommand(command, shell).map((t) => t.text);
}

describe("command tokenizer follows the selected shell", () => {
  it("marks PowerShell comments, strings, and commands distinctly", () => {
    expect(kindsOf("# list services", "powershell")).toEqual(["comment"]);
    const tokens = tokenizeQuickActionCommand(
      `Get-Service -Name spooler # check it`,
      "powershell",
    );
    expect(tokens[0]).toEqual({ text: "Get-Service", kind: "command" });
    expect(tokens[tokens.length - 1].kind).toBe("comment");
    expect(kindsOf(`'it''s'`, "powershell")).toEqual(["string"]);
    expect(kindsOf(`"a $b"`, "powershell")).toEqual(["string"]);
  });

  it("marks CMD comments, strings, and commands distinctly", () => {
    expect(kindsOf("REM clean up", "cmd")[0]).toBe("comment");
    expect(kindsOf(":: also a comment", "cmd")[0]).toBe("comment");
    const tokens = tokenizeQuickActionCommand(`dir "C:\\my dir"`, "cmd");
    expect(tokens[0]).toEqual({ text: "dir", kind: "command" });
    expect(tokens.some((t) => t.kind === "string")).toBe(true);
  });

  it("reads the same text differently per shell", () => {
    // `#` comments PowerShell but is plain text in CMD.
    expect(kindsOf("# x", "powershell")).toEqual(["comment"]);
    expect(kindsOf("# x", "cmd")).not.toContain("comment");
    // `REM` comments CMD but is a plain command word in PowerShell.
    expect(kindsOf("REM x", "cmd")[0]).toBe("comment");
    expect(kindsOf("REM x", "powershell")[0]).toBe("command");
  });

  it("marks the files placeholder in both shells", () => {
    for (const shell of ["powershell", "cmd"] as const) {
      const tokens = tokenizeQuickActionCommand(
        `copy <FilesDir>\\a.txt .`,
        shell,
      );
      expect(tokens.some((t) => t.kind === "placeholder")).toBe(true);
      expect(tokens.find((t) => t.kind === "placeholder")?.text).toBe(
        "<FilesDir>",
      );
    }
  });

  it("keeps every character so the overlay never drops text", () => {
    const corpus = [
      "",
      "echo hi",
      "Get-ChildItem -Recurse | Where-Object { $_.Length -gt 1MB } # big ones",
      "'don''t' + \"stop `believin`\"",
      "copy <FilesDir>\\a.txt <FilesDir>\\b.txt",
      "@echo off\r\ndir %TEMP% && REM done || :: tail",
      "set X=1&echo %X%",
      "# trailing comment without newline",
      "echo unterminated 'string here",
      "<FilesDir>",
      "sort < input.txt",
      'notepad "<FilesDir>\\a.txt"',
    ];
    for (const shell of ["powershell", "cmd"] as const) {
      for (const command of corpus) {
        const joined = textsOf(command, shell).join("");
        expect({ shell, command, joined }).toEqual({
          shell,
          command,
          joined: command,
        });
      }
    }
  });

  it("highlights an attached filename as part of the placeholder", () => {
    const names = ["a.txt"];
    for (const shell of ["powershell", "cmd"] as const) {
      expect(tokenizeQuickActionCommand("run <FilesDir>\\a.txt", shell, names)).toEqual([
        { text: "run", kind: "command" },
        { text: " ", kind: "text" },
        { text: "<FilesDir>\\a.txt", kind: "placeholder" },
      ]);
      // A name that is not attached stays plain text after the tag.
      expect(
        tokenizeQuickActionCommand("run <FilesDir>\\b.txt", shell, names).map((t) => t.kind),
      ).toEqual(["command", "text", "placeholder", "text"]);
      // Matching ignores case like the stored uniqueness rule.
      expect(
        tokenizeQuickActionCommand("run <FilesDir>\\A.TXT", shell, names),
      ).toEqual([
        { text: "run", kind: "command" },
        { text: " ", kind: "text" },
        { text: "<FilesDir>\\A.TXT", kind: "placeholder" },
      ]);
    }
    // Without the filename list the tag alone still highlights (and old
    // two-argument calls keep working).
    expect(tokenizeQuickActionCommand("run <FilesDir>\\a.txt", "powershell")).toEqual([
      { text: "run", kind: "command" },
      { text: " ", kind: "text" },
      { text: "<FilesDir>", kind: "placeholder" },
      { text: "\\a.txt", kind: "text" },
    ]);
  });

  it("keeps the placeholder highlight inside quotes", () => {
    const names = ["a.txt"];
    for (const shell of ["powershell", "cmd"] as const) {
      expect(
        tokenizeQuickActionCommand('notepad "<FilesDir>\\a.txt"', shell, names),
      ).toEqual([
        { text: "notepad", kind: "command" },
        { text: " ", kind: "text" },
        { text: '"', kind: "string" },
        { text: "<FilesDir>\\a.txt", kind: "placeholder" },
        { text: '"', kind: "string" },
      ]);
      // A name that is not attached leaves the tag highlighted alone.
      expect(
        tokenizeQuickActionCommand('run "<FilesDir>\\b.txt"', shell, names).map(
          (t) => t.kind,
        ),
      ).toEqual(["command", "text", "string", "placeholder", "string"]);
    }
    // Single quotes split the same way under PowerShell.
    expect(
      tokenizeQuickActionCommand("cat '<FilesDir>\\a.txt'", "powershell", names),
    ).toEqual([
      { text: "cat", kind: "command" },
      { text: " ", kind: "text" },
      { text: "'", kind: "string" },
      { text: "<FilesDir>\\a.txt", kind: "placeholder" },
      { text: "'", kind: "string" },
    ]);
  });

  it("highlights a spaced filename as one placeholder unit", () => {
    const names = ["my file.txt", "a.txt"];
    for (const shell of ["powershell", "cmd"] as const) {
      expect(
        tokenizeQuickActionCommand("run <FilesDir>\\my file.txt", shell, names),
      ).toEqual([
        { text: "run", kind: "command" },
        { text: " ", kind: "text" },
        { text: "<FilesDir>\\my file.txt", kind: "placeholder" },
      ]);
      // A longer name that could still be forming never matches short.
      expect(
        tokenizeQuickActionCommand("run <FilesDir>\\a.txt2", shell, names).map(
          (t) => t.kind,
        ),
      ).toEqual(["command", "text", "placeholder", "text"]);
    }
  });
});

describe("files autocomplete", () => {
  it("offers the placeholder while a tag is being typed", () => {
    expect(filesCompletion("echo <", 6, [])).toEqual({
      start: 5,
      end: 6,
      items: ["<FilesDir>"],
    });
    expect(filesCompletion("echo <Fil", 9, [])).toEqual({
      start: 5,
      end: 9,
      items: ["<FilesDir>"],
    });
    // A partial that cannot become the placeholder offers nothing.
    expect(filesCompletion("echo <x", 7, [])).toBeNull();
  });

  it("offers file paths directly while the tag is being typed", () => {
    const names = ["foo.txt", "bar.txt"];
    // `<f` matches the placeholder and foo.txt by name prefix, so a file is
    // picked without typing the whole placeholder first.
    expect(filesCompletion("run <f", 6, names)).toEqual({
      start: 4,
      end: 6,
      items: ["<FilesDir>", "<FilesDir>\\foo.txt"],
    });
    // Bare `<` offers everything: the folder plus every file path.
    expect(filesCompletion("run <", 5, names)).toEqual({
      start: 4,
      end: 5,
      items: ["<FilesDir>", "<FilesDir>\\foo.txt", "<FilesDir>\\bar.txt"],
    });
    // A partial matching neither offers nothing.
    expect(filesCompletion("run <x", 6, names)).toBeNull();
    // No files keeps the folder-only offer.
    expect(filesCompletion("run <f", 6, [])).toEqual({
      start: 4,
      end: 6,
      items: ["<FilesDir>"],
    });
  });

  it("offers attached filenames after the placeholder separator", () => {
    const names = ["alpha.txt", "notes.md"];
    expect(filesCompletion("run <FilesDir>\\", 15, names)).toEqual({
      start: 15,
      end: 15,
      items: names,
    });
    expect(filesCompletion("run <FilesDir>\\a", 16, names)).toEqual({
      start: 15,
      end: 16,
      items: ["alpha.txt"],
    });
    expect(filesCompletion("run <FilesDir>\\N", 16, names)).toEqual({
      start: 15,
      end: 16,
      items: ["notes.md"],
    });
    // A forward slash stages the same folder, so it offers the same files.
    expect(filesCompletion("run <FilesDir>/", 15, names)).toEqual({
      start: 15,
      end: 15,
      items: names,
    });
    expect(filesCompletion("run <FilesDir>/a", 16, names)).toEqual({
      start: 15,
      end: 16,
      items: ["alpha.txt"],
    });
    // No attached files means no offer, never an empty list.
    expect(filesCompletion("run <FilesDir>\\", 15, [])).toBeNull();
    expect(filesCompletion("run <FilesDir>\\zzz", 18, names)).toBeNull();
  });

  it("stays closed away from a trigger", () => {
    expect(filesCompletion("echo hi", 7, ["a.txt"])).toBeNull();
    // A finished placeholder with the caret after it is not a trigger.
    expect(filesCompletion("echo <FilesDir>", 15, ["a.txt"])).toBeNull();
    // CMD redirection spacing never triggers: `< ` ends the tag match.
    expect(filesCompletion("sort < ", 7, [])).toBeNull();
  });

  it("splices the item over the trigger range and lands the caret after it", () => {
    expect(applyCompletion("echo <Fil", 5, 9, "<FilesDir>")).toEqual({
      text: "echo <FilesDir>",
      caret: 15,
    });
    expect(
      applyCompletion("run <FilesDir>\\a", 15, 16, "alpha.txt"),
    ).toEqual({ text: "run <FilesDir>\\alpha.txt", caret: 24 });
    expect(applyCompletion("run <f", 4, 6, "<FilesDir>\\foo.txt")).toEqual({
      text: "run <FilesDir>\\foo.txt",
      caret: 22,
    });
    expect(applyCompletion("", 0, 0, "<FilesDir>")).toEqual({
      text: "<FilesDir>",
      caret: 10,
    });
  });
});

describe("files hints lifecycle", () => {
  it("asks for the placeholder exactly while files sit unused", () => {
    expect(filesHint("echo hi", 2)).toBe("unused");
    expect(filesHint("echo <FilesDir>", 2)).toBeNull();
    expect(filesHint("echo hi", 0)).toBeNull();
  });

  it("asks for files exactly while the placeholder has none", () => {
    expect(filesHint("echo <FilesDir>", 0)).toBe("missing");
    expect(filesHint("echo <FilesDir>", 1)).toBeNull();
  });

  it("stays quiet for a self-quoted placeholder while files are attached", () => {
    // The run normalizes the author's pair to the shell's quoting, so
    // wrapping the reference itself needs no flag. With no files yet,
    // "missing" stays the step.
    expect(filesHint('copy "<FilesDir>\\a.txt" .', 1)).toBeNull();
    expect(filesHint("copy '<FilesDir>\\a.txt' .", 1)).toBeNull();
    expect(filesHint('copy "<FilesDir>\\a.txt" .', 0)).toBe("missing");
    expect(filesHint("copy <FilesDir>\\a.txt .", 1)).toBeNull();
    expect(filesHint("don't <FilesDir> .", 1)).toBeNull();
  });
});

describe("files seam and dialog wiring", () => {
  it("exposes the files commands through the api seam only", () => {
    for (const name of [
      "list_quick_action_files",
      "attach_quick_action_file",
      "remove_quick_action_file",
    ]) {
      expect(API_SOURCE).toContain(`"${name}"`);
    }
    expect(API_SOURCE).toContain("QUICK_ACTION_FILE_MAX_BYTES");
    expect(API_SOURCE).toContain("formatActionFileBytes");
    expect(TYPES_SOURCE).toMatch(/interface QuickActionFileMeta/);
  });

  it("never redeclares the placeholder outside the backend owner", () => {
    expect(DIALOG_SOURCE).not.toContain("FILES_DIR_PLACEHOLDER");
    expect(API_SOURCE).not.toContain("FILES_DIR_PLACEHOLDER");
    expect(TYPES_SOURCE).not.toContain("FILES_DIR_PLACEHOLDER");
  });

  it("keeps the textarea the sole edit surface with a visual-only highlight", () => {
    expect(DIALOG_SOURCE).toContain("cmdwrap__hl");
    expect(DIALOG_SOURCE).toContain('aria-hidden="true"');
    expect(DIALOG_SOURCE).toContain(
      "tokenizeQuickActionCommand(command, shell, fileNames)",
    );
    expect(DIALOG_SOURCE).toContain('id="qa-command"');
  });

  it("paints one field edge: chrome lives on the wrap, never on both layers", () => {
    // The highlight copy and the textarea paint text only — a second border
    // down there reads as a seam under the textarea's bottom edge.
    expect(DIALOG_SOURCE).toContain(".cmdwrap:focus-within");
    expect(DIALOG_SOURCE).not.toContain("border: 1px solid transparent");
  });

  it("announces suggestions without moving focus", () => {
    expect(DIALOG_SOURCE).toContain('role="listbox"');
    expect(DIALOG_SOURCE).toContain('role="option"');
    expect(DIALOG_SOURCE).toContain("aria-activedescendant");
    expect(DIALOG_SOURCE).toContain('role="status"');
  });

  it("renders attach/remove/size plus hints inline", () => {
    expect(DIALOG_SOURCE).toContain("Attach files");
    expect(DIALOG_SOURCE).toContain("formatActionFileBytes");
    expect(DIALOG_SOURCE).toContain("removeFile");
    expect(DIALOG_SOURCE).toContain("filesHintState");
    expect(DIALOG_SOURCE).toContain("Attach files…");
    expect(DIALOG_SOURCE).not.toContain('filesHintState === "quoted"');
    expect(DIALOG_SOURCE).toContain("wrapping the reference itself in quotes works too");
  });

  it("keeps Insert a real button that waits for attached files", () => {
    expect(DIALOG_SOURCE).toContain("onclick={insertFilesDir}");
    expect(DIALOG_SOURCE).toContain(
      "disabled={filesBusy || fileRows.length === 0}",
    );
    expect(DIALOG_SOURCE).not.toContain('Insert {"<FilesDir>"}');
  });

  it("adds no editor dependency to the bundle", () => {
    expect(Object.keys(PACKAGE.dependencies).sort()).toEqual([
      "@tauri-apps/api",
      "@tauri-apps/plugin-dialog",
      "@tauri-apps/plugin-notification",
    ]);
  });
});
