/// The preset composer's state (ticket 35): a plain module behind a Vitest
/// seam. Add/remove/expand/version-policy/dependency/env/verify logic lives
/// here, tested without the UI; the dialog is a thin shell over it.
///
/// The composer presents Requirements as "Applications" — the word is a UI
/// synonym, the data shape stays `Requirement` end to end (CONTEXT.md).

import type { EnvWiring, Product, Requirement, VerifyCommand, VersionPolicy } from "./types";

/** A row's hidden-value counts, shown as tags on collapsed rows. */
export interface HiddenCounts {
  env: number;
  verify: number;
  deps: number;
}

export class ComposerState {
  /** The rows being composed, in order. A row without a product is a blank
   *  picker row: it is dropped on save. */
  requirements = $state<Requirement[]>([]);
  /** The one expanded row — `null` means all rows are collapsed. */
  expanded = $state<number | null>(null);
  /** The Settings screen's default timeout, honored for new rows while the
   *  component loads it; falls back to the built-in 10. */
  defaultTimeout: number;

  constructor(defaultTimeout: number = 10) {
    this.defaultTimeout = defaultTimeout;
  }

  /** Replaces the whole list (open/close of the dialog, fork, reload) with a
   *  deep clone so edits never touch the source record. */
  load(rows: Requirement[]) {
    this.requirements = rows.map((r) => ({
      ...r,
      product: { ...r.product, default_env: r.product.default_env.map((e) => ({ ...e })) },
      step: { ...r.step },
      version_policy: { ...r.version_policy },
      depends_on: [...r.depends_on],
      env: r.env.map((e) => ({ ...e })),
      verify: r.verify.map((v) => ({ ...v, args: [...v.args] })),
    }));
    this.expanded = null;
  }

  /** Appends a blank picker row. */
  add() {
    this.requirements = [...this.requirements, this.makeRequirement(blankProduct())];
  }

  /** Picks a library Product for row `i`; the row keeps its policy, timeout,
   *  and dependencies across the pick. */
  setProduct(i: number, product: Product) {
    const current = this.requirements[i];
    if (!current) return;
    this.requirements[i] = {
      ...this.makeRequirement(product),
      version_policy: current.version_policy,
      timeout_minutes: current.timeout_minutes,
      depends_on: current.depends_on,
    };
  }

  /** Removes row `i` and the dependencies other rows had on it; keeps the
   *  expanded panel on the same logical row when rows shift up. */
  remove(i: number) {
    const gone = this.requirements[i]?.product.id;
    this.requirements = this.requirements
      .filter((_, idx) => idx !== i)
      .map((r) => ({
        ...r,
        depends_on: gone ? r.depends_on.filter((d) => d !== gone) : r.depends_on,
      }));
    if (this.expanded === i) this.expanded = null;
    else if (this.expanded !== null && this.expanded > i) this.expanded -= 1;
  }

  /** One row's advanced panel open at a time: toggling a row closes the
   *  previously open one. */
  toggleExpand(i: number) {
    this.expanded = this.expanded === i ? null : i;
  }

  setPolicy(i: number, kind: VersionPolicy["kind"]) {
    const row = this.requirements[i];
    if (!row) return;
    row.version_policy =
      kind === "pinned" ? { kind: "pinned", version: "1.0.0" } : { kind };
  }

  setPinnedVersion(i: number, version: string) {
    const row = this.requirements[i];
    if (row?.version_policy.kind === "pinned") {
      row.version_policy = { kind: "pinned", version };
    }
  }

  setTimeoutMinutes(i: number, value: number) {
    const row = this.requirements[i];
    if (!row) return;
    row.timeout_minutes = Math.max(1, Math.floor(value) || 1);
  }

  toggleDep(i: number, productId: string) {
    const row = this.requirements[i];
    if (!row) return;
    const has = row.depends_on.includes(productId);
    row.depends_on = has
      ? row.depends_on.filter((d) => d !== productId)
      : [...row.depends_on, productId];
  }

  setEnv(i: number, j: number, patch: Partial<EnvWiring>) {
    const row = this.requirements[i];
    if (!row) return;
    row.env[j] = { ...row.env[j], ...patch };
  }

  addEnv(i: number) {
    this.requirements[i]?.env.push({ action: "set", name: "", value: "" });
  }

  removeEnv(i: number, j: number) {
    const row = this.requirements[i];
    if (!row) return;
    row.env = row.env.filter((_, idx) => idx !== j);
  }

  setVerify(i: number, j: number, patch: Partial<VerifyCommand>) {
    const row = this.requirements[i];
    if (!row) return;
    row.verify[j] = { ...row.verify[j], ...patch };
  }

  addVerify(i: number) {
    this.requirements[i]?.verify.push({ command: "", args: [], match_text: null });
  }

  removeVerify(i: number, j: number) {
    const row = this.requirements[i];
    if (!row) return;
    row.verify = row.verify.filter((_, idx) => idx !== j);
  }

  /** Meaningful hidden-value counts for collapsed-row tags; `null` when the
   *  row hides nothing. Blank in-progress entries don't count. */
  hiddenCounts(i: number): HiddenCounts | null {
    const row = this.requirements[i];
    if (!row) return null;
    const counts: HiddenCounts = {
      env: row.env.filter((e) => e.name.trim() || e.value.trim()).length,
      verify: row.verify.filter((v) => v.command.trim() || v.match_text).length,
      deps: row.depends_on.length,
    };
    if (counts.env === 0 && counts.verify === 0 && counts.deps === 0) return null;
    return counts;
  }

  /** The first row that fails validation, or null. Blank picker rows are
   *  skipped — they are dropped on save, not errors. */
  firstError(): string | null {
    for (const r of this.requirements) {
      if (!r.product.id) continue;
      const label = r.product.name || r.product.id;
      const envRows = r.env.filter((e) => e.name.trim() || e.value.trim());
      if (envRows.some((e) => !e.name.trim() || !e.value.trim())) {
        return `Application "${label}": every env wiring entry needs both a variable name and a value.`;
      }
      const verifyRows = r.verify.filter((v) => v.command.trim() || v.match_text);
      if (verifyRows.some((v) => !v.command.trim())) {
        return `Application "${label}": every verify command needs a command.`;
      }
    }
    return null;
  }

  /** The save-ready rows: blank picker rows dropped, fields trimmed, blank
   *  env/verify entries filtered, dependencies restricted to surviving rows. */
  clean(): Requirement[] {
    const rows = this.requirements.filter((r) => r.product.id.trim());
    return rows.map((r) => ({
      ...r,
      product: { ...r.product, name: r.product.name.trim(), id: r.product.id.trim() },
      step:
        r.step.type === "winget"
          ? { type: "winget", id: r.step.id.trim(), scope: r.step.scope.trim() || "machine" }
          : { ...r.step },
      version_policy:
        r.version_policy.kind === "pinned"
          ? { kind: "pinned", version: r.version_policy.version.trim() }
          : r.version_policy,
      depends_on: r.depends_on.filter((d) => rows.some((x) => x.product.id === d)),
      env: r.env
        .filter((e) => e.name.trim() || e.value.trim())
        .map((e) => ({ action: e.action, name: e.name.trim(), value: e.value.trim() })),
      verify: r.verify
        .filter((v) => v.command.trim() || v.match_text)
        .map((v) => ({
          command: v.command.trim(),
          args: v.args,
          match_text: v.match_text?.trim() || null,
        })),
    }));
  }

  private makeRequirement(product: Product): Requirement {
    // The preset file shape (spec decision 11) carries the product, never
    // the Library-only timestamps (ticket 13) — the Rust side stores only
    // its own known fields, so nulls here never reach a .sprout.json.
    const { created_at: _created, updated_at: _updated, ...productFields } = product;
    return {
      product: {
        ...productFields,
        default_env: [],
        created_at: null,
        updated_at: null,
      },
      step: product.winget_id
        ? { type: "winget", id: product.winget_id, scope: "machine" }
        : { type: "command", exe: "", args: [], success_codes: [0] },
      version_policy: { kind: "latest" },
      depends_on: [],
      timeout_minutes: this.defaultTimeout,
      env: product.default_env.map((e) => ({ ...e })),
      verify: [],
    };
  }
}

function blankProduct(): Product {
  return {
    id: "",
    name: "",
    winget_id: null,
    install_location_hint: null,
    install_dir: null,
    default_env: [],
    created_at: null,
    updated_at: null,
  };
}