/**
 * Lightweight command-editor intelligence for the Quick Action dialog: shell
 * coloring plus `<FilesDir>` autocomplete plus unused/missing hints.
 *
 * A small hand scanner covers the two authoring shells with token classes only
 * (comments/strings/commands distinct) — no editor dependency, so the bundle
 * is unchanged. The textarea stays the sole edit surface (native
 * paste/undo/IME); the highlight copy behind it is visual only, which is why
 * this module never touches the DOM and every function here is pure.
 *
 * The files placeholder is always the backend contract's literal, reused
 * inline and never redeclared: the single Windows execution owner expands and
 * shell-quotes it at run time (ADR-0029), so completions insert it raw and let
 * the owner quote.
 */

export type QuickActionTokenKind =
  | "text"
  | "comment"
  | "string"
  | "command"
  | "placeholder";

export interface QuickActionToken {
  text: string;
  kind: QuickActionTokenKind;
}

export type QuickActionShellKind = "powershell" | "cmd";

export function tokenizeQuickActionCommand(
  command: string,
  shell: QuickActionShellKind,
  filenames: string[] = [],
): QuickActionToken[] {
  return shell === "cmd"
    ? tokenizeCmdCommand(command, filenames)
    : tokenizePowerShellCommand(command, filenames);
}

type Emitter = (text: string, kind: QuickActionTokenKind) => void;

function makeEmitter(tokens: QuickActionToken[]): Emitter {
  return (text, kind) => {
    if (!text) return;
    const last = tokens[tokens.length - 1];
    if (last && last.kind === kind) last.text += text;
    else tokens.push({ text, kind });
  };
}

function isWordChar(value: string): boolean {
  return /[A-Za-z0-9_.\-]/.test(value);
}

/**
 * After a `<FilesDir>` placeholder, a `\` (or `/` — both stage the same
 * folder) plus an attached filename belongs to the same reference
 * (`<FilesDir>\a.txt` stages that file): highlight it with the placeholder so
 * a file reference reads as one unit instead of a colored tag plus plain
 * text. Matching runs against the attached names so a spaced file still
 * resolves; a longer name that could still be forming never matches short.
 * Matching is case-insensitive like the stored uniqueness rule; the emitted
 * text keeps the user's own casing.
 */
function filesDirNameLength(
  command: string,
  separatorAt: number,
  filenames: string[],
): number {
  const sep = command[separatorAt];
  if ((sep !== "\\" && sep !== "/") || filenames.length === 0) return 0;
  const rest = command.slice(separatorAt + 1);
  let best = 0;
  for (const name of filenames) {
    if (
      rest.length >= name.length &&
      rest.slice(0, name.length).toLowerCase() === name.toLowerCase() &&
      name.length > best
    ) {
      const next = rest[name.length];
      if (next === undefined || !/[A-Za-z0-9.\-_~]/.test(next)) {
        best = name.length;
      }
    }
  }
  return best > 0 ? 1 + best : 0;
}

/**
 * Emits one scanned string span, splitting out any `<FilesDir>` references
 * it encloses so a quoted file reference keeps its placeholder highlight
 * (`"<FilesDir>\a.txt"` reads as string, placeholder, string). The run
 * expands the placeholder textually even inside quotes, so the overlay must
 * show it wherever the backend will act on it. Character-preserving: the
 * pieces rejoin to the span exactly.
 */
function emitStringWithPlaceholders(
  command: string,
  start: number,
  end: number,
  filenames: string[],
  emit: Emitter,
) {
  let k = start;
  while (k < end) {
    const at = command.indexOf("<FilesDir>", k);
    if (at === -1 || at + "<FilesDir>".length > end) break;
    if (at > k) emit(command.slice(k, at), "string");
    emit("<FilesDir>", "placeholder");
    k = at + "<FilesDir>".length;
    const nameLength = filesDirNameLength(command, k, filenames);
    if (nameLength > 0) {
      emit(command.slice(k, k + nameLength), "placeholder");
      k += nameLength;
    }
  }
  emit(command.slice(k, end), "string");
}

function tokenizePowerShellCommand(
  command: string,
  filenames: string[],
): QuickActionToken[] {
  const tokens: QuickActionToken[] = [];
  const emit = makeEmitter(tokens);
  let i = 0;
  const n = command.length;
  // A statement's first bare word is its command: line starts and the
  // chaining/block operators that open a new statement.
  let expectCommand = true;
  while (i < n) {
    if (command.startsWith("<FilesDir>", i)) {
      emit("<FilesDir>", "placeholder");
      i += "<FilesDir>".length;
      const nameLength = filesDirNameLength(command, i, filenames);
      if (nameLength > 0) {
        emit(command.slice(i, i + nameLength), "placeholder");
        i += nameLength;
      }
      expectCommand = false;
      continue;
    }
    const c = command[i];
    if (c === "#") {
      let j = command.indexOf("\n", i);
      if (j === -1) j = n;
      emit(command.slice(i, j), "comment");
      i = j;
      continue;
    }
    if (c === "'") {
      let j = i + 1;
      while (j < n) {
        if (command.startsWith("''", j)) {
          j += 2;
          continue;
        }
        if (command[j] === "'") {
          j += 1;
          break;
        }
        j += 1;
      }
      emitStringWithPlaceholders(command, i, j, filenames, emit);
      i = j;
      expectCommand = false;
      continue;
    }
    if (c === '"') {
      let j = i + 1;
      while (j < n) {
        if (command[j] === "`" && j + 1 < n) {
          j += 2;
          continue;
        }
        if (command[j] === '"') {
          j += 1;
          break;
        }
        j += 1;
      }
      emitStringWithPlaceholders(command, i, j, filenames, emit);
      i = j;
      expectCommand = false;
      continue;
    }
    if (c === "\n" || c === ";" || c === "{" || c === "(") {
      emit(c, "text");
      i += 1;
      expectCommand = true;
      continue;
    }
    if (c === "|" || c === "&") {
      emit(c, "text");
      i += 1;
      if (command[i] === c) {
        emit(c, "text");
        i += 1;
      }
      expectCommand = true;
      continue;
    }
    if (/\s/.test(c)) {
      emit(c, "text");
      i += 1;
      continue;
    }
    if (expectCommand && /[A-Za-z]/.test(c)) {
      let j = i + 1;
      while (j < n && isWordChar(command[j])) j += 1;
      emit(command.slice(i, j), "command");
      i = j;
      expectCommand = false;
      continue;
    }
    emit(c, "text");
    i += 1;
    expectCommand = false;
  }
  return tokens;
}

function tokenizeCmdCommand(
  command: string,
  filenames: string[],
): QuickActionToken[] {
  const tokens: QuickActionToken[] = [];
  const emit = makeEmitter(tokens);
  let i = 0;
  const n = command.length;
  let expectCommand = true;
  while (i < n) {
    if (command.startsWith("<FilesDir>", i)) {
      emit("<FilesDir>", "placeholder");
      i += "<FilesDir>".length;
      const nameLength = filesDirNameLength(command, i, filenames);
      if (nameLength > 0) {
        emit(command.slice(i, i + nameLength), "placeholder");
        i += nameLength;
      }
      expectCommand = false;
      continue;
    }
    const c = command[i];
    if (c === "\n") {
      emit(c, "text");
      i += 1;
      expectCommand = true;
      continue;
    }
    if (c === " " || c === "\t" || c === "\r") {
      emit(c, "text");
      i += 1;
      continue;
    }
    // A `::` line opens a comment; anything else keeps the statement grammar.
    if (c === ":" && command[i + 1] === ":") {
      let j = command.indexOf("\n", i);
      if (j === -1) j = n;
      emit(command.slice(i, j), "comment");
      i = j;
      continue;
    }
    if (c === '"' && !expectCommand) {
      let j = i + 1;
      while (j < n && command[j] !== '"' && command[j] !== "\n") j += 1;
      if (command[j] === '"') j += 1;
      emitStringWithPlaceholders(command, i, j, filenames, emit);
      i = j;
      continue;
    }
    if (c === '"' && expectCommand) {
      emit(c, "text");
      i += 1;
      expectCommand = false;
      continue;
    }
    if (c === "&" || c === "|" || c === "(") {
      emit(c, "text");
      i += 1;
      if ((c === "&" || c === "|") && command[i] === c) {
        emit(c, "text");
        i += 1;
      }
      expectCommand = true;
      continue;
    }
    if (expectCommand) {
      // `REM ...` to end of line is the word comment; `@` quiets echo and
      // stays plain text in front of the command word.
      if (
        (c === "R" || c === "r") &&
        (command[i + 1] === "E" || command[i + 1] === "e") &&
        (command[i + 2] === "M" || command[i + 2] === "m") &&
        (i + 3 >= n || /[\s&|();,=]/.test(command[i + 3]))
      ) {
        let j = command.indexOf("\n", i);
        if (j === -1) j = n;
        emit(command.slice(i, j), "comment");
        i = j;
        continue;
      }
      let j = i;
      if (command[j] === "@") {
        emit("@", "text");
        j += 1;
      }
      if (j < n && /[A-Za-z]/.test(command[j])) {
        let k = j + 1;
        while (k < n && /[A-Za-z0-9_.\-\\:$%~?]/.test(command[k])) k += 1;
        emit(command.slice(j, k), "command");
        i = k;
        expectCommand = false;
        continue;
      }
      emit(c, "text");
      i += 1;
      expectCommand = false;
      continue;
    }
    emit(c, "text");
    i += 1;
  }
  return tokens;
}

/** One open completion: the range it replaces plus the items to offer. */
export interface FilesCompletion {
  start: number;
  end: number;
  items: string[];
}

/**
 * What the `<` key offers at `caret`: the files placeholder while a tag is
 * being typed — plus `<FilesDir>\<name>` for every attached file whose name
 * starts with the typed partial, so a file is picked directly without typing
 * the whole placeholder first — attached filenames after `<FilesDir>\` or
 * `<FilesDir>/` (both separators stage the same folder), nothing otherwise.
 * Case-insensitive prefix filtering; an empty offer (or none) reads as closed
 * so the list never opens empty.
 */
export function filesCompletion(
  command: string,
  caret: number,
  filenames: string[],
): FilesCompletion | null {
  const safe = Math.max(0, Math.min(caret, command.length));
  const before = command.slice(0, safe);
  const fileMatch = before.match(/<FilesDir>[\\/]([^<>\r\n]*)$/);
  if (fileMatch) {
    const partial = fileMatch[1];
    const items = filenames.filter((name) =>
      name.toLowerCase().startsWith(partial.toLowerCase()),
    );
    if (items.length === 0) return null;
    return { start: safe - partial.length, end: safe, items };
  }
  const tagMatch = before.match(/<([A-Za-z]*)$/);
  if (tagMatch) {
    const partial = tagMatch[1];
    const items: string[] = [];
    if ("<FilesDir>".toLowerCase().startsWith(`<${partial}`.toLowerCase())) {
      items.push("<FilesDir>");
    }
    const lowerPartial = partial.toLowerCase();
    for (const name of filenames) {
      if (name.toLowerCase().startsWith(lowerPartial)) {
        items.push(`<FilesDir>\\${name}`);
      }
    }
    if (items.length === 0) return null;
    return {
      start: safe - partial.length - 1,
      end: safe,
      items,
    };
  }
  return null;
}

/** Splices one accepted item over the completion range; the caret lands after it. */
export function applyCompletion(
  command: string,
  start: number,
  end: number,
  insert: string,
): { text: string; caret: number } {
  const safeStart = Math.max(0, Math.min(start, command.length));
  const safeEnd = Math.max(safeStart, Math.min(end, command.length));
  const text = command.slice(0, safeStart) + insert + command.slice(safeEnd);
  return { text, caret: safeStart + insert.length };
}

export type FilesHint = "unused" | "missing" | null;

/**
 * Which files hint the dialog owes right now, if any: attached files with no
 * placeholder stay inert at run, and a placeholder with no files stages
 * nothing. Wrapping the reference itself in quotes needs no flag — the run
 * normalizes the author's pair to the shell's quoting. With no files yet,
 * "missing" stays the single next step. Both clear the moment the command or
 * the file list fixes them.
 */
export function filesHint(command: string, fileCount: number): FilesHint {
  const hasPlaceholder = command.includes("<FilesDir>");
  if (hasPlaceholder && fileCount === 0) return "missing";
  if (fileCount > 0 && !hasPlaceholder) return "unused";
  return null;
}
