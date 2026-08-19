import { describe, expect, it } from "vitest";
import { ComposerState } from "./composerState.svelte";
import type { Product, Requirement } from "./types";

function product(id: string, name = id): Product {
  return {
    id,
    name,
    winget_id: id,
    install_location_hint: null,
    install_dir: null,
    default_env: [],
    created_at: 1,
    updated_at: 2,
  };
}

function filledRow(id: string): Requirement {
  const state = new ComposerState();
  state.add();
  state.setProduct(0, product(id));
  return state.requirements[0];
}

describe("ComposerState", () => {
  describe("add", () => {
    it("appends a blank picker row", () => {
      const state = new ComposerState();
      state.add();
      expect(state.requirements).toHaveLength(1);
      expect(state.requirements[0].product.id).toBe("");
      expect(state.requirements[0].version_policy).toEqual({ kind: "latest" });
      expect(state.requirements[0].timeout_minutes).toBe(10);
      expect(state.requirements[0].env).toEqual([]);
      expect(state.requirements[0].verify).toEqual([]);
      expect(state.requirements[0].depends_on).toEqual([]);
    });

    it("uses the settings default timeout for new rows", () => {
      const state = new ComposerState(25);
      state.add();
      expect(state.requirements[0].timeout_minutes).toBe(25);
    });

    it("honors a default timeout updated after construction", () => {
      const state = new ComposerState();
      state.defaultTimeout = 30;
      state.add();
      expect(state.requirements[0].timeout_minutes).toBe(30);
    });
  });

  describe("load", () => {
    it("replaces the list and collapses all rows", () => {
      const state = new ComposerState();
      state.add();
      state.toggleExpand(0);
      state.load([filledRow("A"), filledRow("B")]);
      expect(state.requirements.map((r) => r.product.id)).toEqual(["A", "B"]);
      expect(state.expanded).toBeNull();
    });

    it("deep-clones so edits never touch the source", () => {
      const source = [filledRow("A")];
      source[0].env = [{ action: "set", name: "X", value: "1" }];
      source[0].verify = [{ command: "cmd", args: [], match_text: null }];
      source[0].depends_on = ["B"];
      const state = new ComposerState();
      state.load(source);
      state.requirements[0].env[0].name = "Y";
      state.requirements[0].verify[0].command = "other";
      state.requirements[0].depends_on.push("C");
      state.requirements[0].product.default_env.push({ action: "set", name: "Z", value: "2" });
      expect(source[0].env[0].name).toBe("X");
      expect(source[0].verify[0].command).toBe("cmd");
      expect(source[0].depends_on).toEqual(["B"]);
      expect(source[0].product.default_env).toEqual([]);
    });
  });

  describe("setProduct", () => {
    it("fills the row from the library product and strips library timestamps", () => {
      const state = new ComposerState();
      state.add();
      state.setProduct(0, product("A", "Alpha"));
      const row = state.requirements[0];
      expect(row.product.id).toBe("A");
      expect(row.product.name).toBe("Alpha");
      expect(row.product.created_at).toBeNull();
      expect(row.product.updated_at).toBeNull();
      expect(row.step).toEqual({ type: "winget", id: "A", scope: "machine" });
      expect(row.version_policy).toEqual({ kind: "latest" });
    });

    it("keeps the row's policy, timeout, and dependencies across a re-pick", () => {
      const state = new ComposerState();
      state.add();
      state.setPolicy(0, "pinned");
      state.setPinnedVersion(0, "2.0.0");
      state.setTimeoutMinutes(0, 42);
      state.add();
      state.setProduct(1, product("B"));
      state.toggleDep(0, "B");
      state.setProduct(0, product("A"));
      expect(state.requirements[0].version_policy).toEqual({ kind: "pinned", version: "2.0.0" });
      expect(state.requirements[0].timeout_minutes).toBe(42);
      expect(state.requirements[0].depends_on).toEqual(["B"]);
    });

    it("carries the product's default env wiring", () => {
      const p = product("A");
      p.default_env = [{ action: "prepend", name: "PATH", value: "<InstallLocation:hint>" }];
      const state = new ComposerState();
      state.add();
      state.setProduct(0, p);
      expect(state.requirements[0].env).toEqual(p.default_env);
    });

    it("does nothing when the row index is out of range", () => {
      const state = new ComposerState();
      state.setProduct(3, product("A"));
      expect(state.requirements).toHaveLength(0);
    });
  });

  describe("remove", () => {
    it("removes the row and clears other rows' dependencies on it", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B"), filledRow("C")]);
      state.toggleDep(0, "B");
      state.toggleDep(2, "B");
      state.remove(1);
      expect(state.requirements.map((r) => r.product.id)).toEqual(["A", "C"]);
      expect(state.requirements[0].depends_on).toEqual([]);
      expect(state.requirements[1].depends_on).toEqual([]);
    });

    it("collapses when the expanded row itself is removed", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B")]);
      state.toggleExpand(1);
      state.remove(1);
      expect(state.expanded).toBeNull();
    });

    it("keeps the expanded panel on the same logical row when rows above shift", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B"), filledRow("C")]);
      state.toggleExpand(2);
      state.remove(0);
      expect(state.expanded).toBe(1);
      expect(state.requirements[1].product.id).toBe("C");
    });
  });

  describe("toggleExpand", () => {
    it("opens a row and closes any other open row", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B")]);
      state.toggleExpand(0);
      expect(state.expanded).toBe(0);
      state.toggleExpand(1);
      expect(state.expanded).toBe(1);
    });

    it("closes when toggled again", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.toggleExpand(0);
      state.toggleExpand(0);
      expect(state.expanded).toBeNull();
    });
  });

  describe("version policy", () => {
    it("sets latest and present", () => {
      const state = new ComposerState();
      state.add();
      state.setPolicy(0, "present");
      expect(state.requirements[0].version_policy).toEqual({ kind: "present" });
      state.setPolicy(0, "latest");
      expect(state.requirements[0].version_policy).toEqual({ kind: "latest" });
    });

    it("seeds a default version when pinning", () => {
      const state = new ComposerState();
      state.add();
      state.setPolicy(0, "pinned");
      expect(state.requirements[0].version_policy).toEqual({ kind: "pinned", version: "1.0.0" });
    });

    it("sets the pinned version and ignores it for other policies", () => {
      const state = new ComposerState();
      state.add();
      state.setPolicy(0, "pinned");
      state.setPinnedVersion(0, "21.0.5");
      expect(state.requirements[0].version_policy).toEqual({ kind: "pinned", version: "21.0.5" });
      state.setPolicy(0, "latest");
      state.setPinnedVersion(0, "9.9.9");
      expect(state.requirements[0].version_policy).toEqual({ kind: "latest" });
    });
  });

  describe("setTimeoutMinutes", () => {
    it("floors and clamps to at least 1", () => {
      const state = new ComposerState();
      state.add();
      state.setTimeoutMinutes(0, 7.9);
      expect(state.requirements[0].timeout_minutes).toBe(7);
      state.setTimeoutMinutes(0, 0);
      expect(state.requirements[0].timeout_minutes).toBe(1);
      state.setTimeoutMinutes(0, -3);
      expect(state.requirements[0].timeout_minutes).toBe(1);
      state.setTimeoutMinutes(0, Number.NaN);
      expect(state.requirements[0].timeout_minutes).toBe(1);
    });
  });

  describe("dependencies", () => {
    it("toggles a dependency on and off", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B")]);
      state.toggleDep(0, "B");
      expect(state.requirements[0].depends_on).toEqual(["B"]);
      state.toggleDep(0, "B");
      expect(state.requirements[0].depends_on).toEqual([]);
    });
  });

  describe("env wiring", () => {
    it("adds, patches, and removes entries", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.addEnv(0);
      state.addEnv(0);
      expect(state.requirements[0].env).toHaveLength(2);
      state.setEnv(0, 0, { action: "prepend", name: "PATH", value: "C:\\bin" });
      expect(state.requirements[0].env[0]).toEqual({
        action: "prepend",
        name: "PATH",
        value: "C:\\bin",
      });
      state.removeEnv(0, 1);
      expect(state.requirements[0].env).toHaveLength(1);
    });
  });

  describe("verify commands", () => {
    it("adds, patches, and removes entries", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.addVerify(0);
      state.addVerify(0);
      expect(state.requirements[0].verify).toHaveLength(2);
      state.setVerify(0, 0, { command: "java -version", match_text: "build" });
      expect(state.requirements[0].verify[0]).toEqual({
        command: "java -version",
        args: [],
        match_text: "build",
      });
      state.removeVerify(0, 1);
      expect(state.requirements[0].verify).toHaveLength(1);
    });
  });

  describe("hiddenCounts", () => {
    it("counts meaningful env, verify, and dependency entries", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B"), filledRow("C")]);
      state.toggleDep(0, "B");
      state.addEnv(0);
      state.setEnv(0, 0, { name: "X", value: "1" });
      state.addEnv(0); // blank in-progress entry — not counted
      state.addVerify(0);
      state.setVerify(0, 0, { command: "cmd" });
      expect(state.hiddenCounts(0)).toEqual({ env: 1, verify: 1, deps: 1 });
    });

    it("returns null when the row hides nothing", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      expect(state.hiddenCounts(0)).toBeNull();
    });

    it("returns null for a blank picker row", () => {
      const state = new ComposerState();
      state.add();
      expect(state.hiddenCounts(0)).toBeNull();
    });
  });

  describe("firstError", () => {
    it("flags an env entry missing a name or value", () => {
      const state = new ComposerState();
      state.load([filledRow("Alpha")]);
      state.addEnv(0);
      state.setEnv(0, 0, { name: "", value: "1" });
      expect(state.firstError()).toContain("Application \"Alpha\"");
      expect(state.firstError()).toContain("variable name and a value");
    });

    it("flags a verify command missing a command", () => {
      const state = new ComposerState();
      state.load([filledRow("Alpha")]);
      state.addVerify(0);
      state.setVerify(0, 0, { command: "", match_text: "anything" });
      expect(state.firstError()).toContain("every verify command needs a command");
    });

    it("ignores blank picker rows and reports the first bad row", () => {
      const state = new ComposerState();
      state.add();
      state.load([filledRow("A"), filledRow("B")]);
      state.addVerify(0);
      state.addVerify(1);
      state.setVerify(1, 0, { command: "", match_text: "anything" });
      expect(state.firstError()).toContain("Application \"B\"");
    });

    it("returns null when everything is fine", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.add();
      expect(state.firstError()).toBeNull();
    });
  });

  describe("clean", () => {
    it("drops blank rows and trims fields", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.add();
      state.requirements[0].product.name = "  Alpha  ";
      state.requirements[0].step = { type: "winget", id: " A ", scope: " machine " };
      const rows = state.clean();
      expect(rows).toHaveLength(1);
      expect(rows[0].product.name).toBe("Alpha");
      expect(rows[0].step).toEqual({ type: "winget", id: "A", scope: "machine" });
    });

    it("drops blank env and verify entries and trims the rest", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.addEnv(0);
      state.setEnv(0, 0, { name: "X", value: " 1 " });
      state.addEnv(0);
      state.addVerify(0);
      state.setVerify(0, 0, { command: " cmd ", match_text: " out " });
      state.addVerify(0);
      const rows = state.clean();
      expect(rows[0].env).toEqual([{ action: "set", name: "X", value: "1" }]);
      expect(rows[0].verify).toEqual([{ command: "cmd", args: [], match_text: "out" }]);
    });

    it("restricts dependencies to surviving rows", () => {
      const state = new ComposerState();
      state.load([filledRow("A"), filledRow("B")]);
      state.toggleDep(0, "B");
      state.toggleDep(0, "gone");
      const rows = state.clean();
      expect(rows[0].depends_on).toEqual(["B"]);
    });

    it("trims the pinned version", () => {
      const state = new ComposerState();
      state.load([filledRow("A")]);
      state.setPolicy(0, "pinned");
      state.setPinnedVersion(0, " 21.0.5 ");
      expect(state.clean()[0].version_policy).toEqual({ kind: "pinned", version: "21.0.5" });
    });
  });
});