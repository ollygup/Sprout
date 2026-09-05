// ADR-0029 ownership gate: every Sprout-owned Windows command/API must be
// invoked from exactly one owning module. A Windows change must mean fixing
// one implementation, never hunting copies across install, authoring,
// Quick Launch, and Quick Actions.
// Run: node tools/ownership-gate.mjs [root]   (expect "ownership gate: pass")

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = process.argv[2] ?? join(fileURLToPath(import.meta.url), "..", "..");
const SRC = join(ROOT, "src-tauri", "src");

// Each entry: the native operation's symbols and import paths, and the only
// files (repo-root-relative) allowed to reference them. Callers elsewhere
// must go through the owner's interface; a genuinely new operation gets a
// new row (AGENTS.md), never a silent second invocation site.
const OWNERS = [
  {
    operation: "Quick Launch window/process inspection (EnumWindows, Toolhelp, image query)",
    owners: ["src-tauri/src/engine/windows/inspection.rs"],
    symbols: [
      "EnumWindows",
      "GetWindowThreadProcessId",
      "IsWindowVisible",
      "QueryFullProcessImageNameW",
      "CreateToolhelp32Snapshot",
      "Process32FirstW",
      "Process32NextW",
      "PROCESSENTRY32W",
      "TH32CS_SNAPPROCESS",
      "GetExitCodeProcess",
      "STILL_ACTIVE",
    ],
    modules: ["Diagnostics::ToolHelp"],
  },
  {
    operation: "display-config probing (QueryDisplayConfig, EDID identity)",
    owners: ["src-tauri/src/appbar/display.rs"],
    symbols: [
      "QueryDisplayConfig",
      "GetDisplayConfigBufferSizes",
      "DisplayConfigGetDeviceInfo",
      "DISPLAYCONFIG_PATH_INFO",
      "DISPLAYCONFIG_MODE_INFO",
      "DISPLAYCONFIG_SOURCE_DEVICE_NAME",
      "DISPLAYCONFIG_TARGET_DEVICE_NAME",
      "DISPLAYCONFIG_DEVICE_INFO_HEADER",
      "DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME",
      "DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME",
      "QDC_ONLY_ACTIVE_PATHS",
    ],
    modules: ["Devices::Display"],
  },
  {
    operation: "visible shell open (ShellExecuteW)",
    owners: ["src-tauri/src/windows_execution/shell.rs"],
    symbols: ["ShellExecuteW"],
    modules: [],
  },
  {
    operation: "handle-returning app launch (ShellExecuteExW, distinct from ShellExecuteW)",
    owners: ["src-tauri/src/engine/windows.rs"],
    symbols: ["ShellExecuteExW"],
    modules: [],
  },
];

const escapeRegExp = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function rustFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...rustFiles(full));
    } else if (entry.endsWith(".rs")) {
      out.push(full);
    }
  }
  return out;
}

function stripComment(line) {
  const trimmed = line.trimStart();
  if (trimmed.startsWith("//")) {
    return "";
  }
  const cut = line.search(/(^|\s)\/\//);
  return cut === -1 ? line : line.slice(0, cut);
}

// A `use` statement may span lines (nested brace groups); join until braces
// balance so aliases and module paths are seen whole.
function joinUseStatements(lines) {
  const out = [];
  let pending = null;
  for (const line of lines) {
    if (pending === null) {
      if (line.trimStart().startsWith("use ")) {
        pending = line;
      } else {
        out.push({ text: line, isUse: false });
        continue;
      }
    } else {
      pending += "\n" + line;
    }
    const open = (pending.match(/{/g) ?? []).length;
    const close = (pending.match(/}/g) ?? []).length;
    if (open === close && pending.trimEnd().endsWith(";")) {
      out.push({ text: pending, isUse: true });
      pending = null;
    }
  }
  if (pending !== null) {
    out.push({ text: pending, isUse: true });
  }
  return out;
}

const violations = [];
let references = 0;

for (const file of rustFiles(SRC)) {
  const rel = relative(ROOT, file).replace(/\\/g, "/");
  const lines = readFileSync(file, "utf8").split("\n");
  const aliased = new Map(); // local alias -> owner entry (from `X as Y` imports)
  const statements = joinUseStatements(lines);
  for (const { text, isUse } of statements) {
    if (!isUse) {
      continue;
    }
    const code = stripComment(text);
    for (const entry of OWNERS) {
      for (const symbol of entry.symbols) {
        const alias = new RegExp(`\\b${escapeRegExp(symbol)}\\s+as\\s+(\\w+)`).exec(code);
        if (alias) {
          aliased.set(alias[1], entry);
        }
      }
    }
  }
  lines.forEach((raw, index) => {
    const code = stripComment(raw);
    if (code.trim() === "") {
      return;
    }
    const lineno = index + 1;
    for (const entry of OWNERS) {
      const owned = entry.owners.includes(rel);
      for (const symbol of entry.symbols) {
        if (!new RegExp(`\\b${escapeRegExp(symbol)}\\b`).test(code)) {
          continue;
        }
        references += 1;
        if (!owned) {
          violations.push(`${rel}:${lineno}: '${symbol}' must live in ${entry.owners.join(", ")} (${entry.operation})`);
        }
      }
      for (const module of entry.modules) {
        if (code.includes(module)) {
          references += 1;
          if (!owned) {
            violations.push(`${rel}:${lineno}: '${module}' import must live in ${entry.owners.join(", ")} (${entry.operation})`);
          }
        }
      }
    }
    for (const [alias, entry] of aliased) {
      if (!new RegExp(`\\b${escapeRegExp(alias)}\\b`).test(code)) {
        continue;
      }
      references += 1;
      if (!entry.owners.includes(rel)) {
        violations.push(`${rel}:${lineno}: alias '${alias}' of an owned native API must live in ${entry.owners.join(", ")} (${entry.operation})`);
      }
    }
  });
}

if (violations.length > 0) {
  console.error("ownership gate: FAIL — owned Windows operations referenced outside their owners:");
  for (const violation of violations) {
    console.error(`  ${violation}`);
  }
  console.error("See ADR-0029 (one source of truth per Windows command) and the operation inventory.");
  process.exit(1);
}

console.log(`ownership gate: pass (${references} owned references, all in their owner files)`);
