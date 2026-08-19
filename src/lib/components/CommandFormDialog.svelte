<script lang="ts">
  import type { LaunchCommandTest, LaunchShell } from "$lib/types";
  import { launchShellLabel } from "$lib/types";
  import { createLaunchEntry, testLaunchCommand } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import Icon from "./Icon.svelte";
  import Select from "./Select.svelte";

  let {
    open,
    onsave,
    oncancel,
  }: {
    open: boolean;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let shell = $state<LaunchShell>("powershell");
  let command = $state("");
  let showWindow = $state(false);
  let saving = $state(false);
  let error = $state("");
  let test: LaunchCommandTest | null = $state(null);
  let testing = $state(false);
  let tested = $state(false);

  // The name follows the command until the user edits it by hand.
  let nameAuto = $state(true);

  $effect(() => {
    if (open) {
      name = "";
      shell = "powershell";
      command = "";
      showWindow = false;
      saving = false;
      error = "";
      test = null;
      testing = false;
      tested = false;
      nameAuto = true;
    }
  });

  /** The entry's name suggestion from the command: the first token (quotes
   * honored), its path basename, without a script/exe extension. */
  function nameFromCommand(value: string): string {
    const trimmed = value.trim();
    if (!trimmed) return "";
    let token = "";
    let inQuotes = false;
    for (const ch of trimmed) {
      if (ch === '"') inQuotes = !inQuotes;
      else if (!inQuotes && /\s/.test(ch)) break;
      else token += ch;
    }
    const base = token.split(/[\\/]/).pop() ?? token;
    return base.replace(/\.(exe|cmd|bat|ps1)$/i, "").trim();
  }

  function onCommandInput(value: string) {
    command = value;
    if (nameAuto) name = nameFromCommand(value);
  }

  function onNameInput(value: string) {
    name = value;
    nameAuto = false;
  }

  async function runTest() {
    if (!command.trim()) {
      error = "Type a command first.";
      return;
    }
    error = "";
    tested = true;
    testing = true;
    test = null;
    try {
      test = await testLaunchCommand(shell, command.trim());
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      testing = false;
    }
  }

  async function submit() {
    if (!name.trim()) {
      error = "Give the entry a name.";
      return;
    }
    if (!command.trim()) {
      error = "The command must not be empty.";
      return;
    }
    saving = true;
    error = "";
    try {
      await createLaunchEntry({
        name: name.trim(),
        kind: "command",
        target: command.trim(),
        shell,
        show_window: showWindow,
        desktop_id: null,
      });
      await onsave(`${name.trim()} added to Quick Launch.`);
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog {open} title="Add a command" onclose={oncancel} width={560}>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <div class="field">
      <label class="field__label" for="command-shell">Shell</label>
      <Select
        id="command-shell"
        value={shell}
        onchange={(v) => (shell = v as LaunchShell)}
      >
        <option value="powershell">PowerShell</option>
        <option value="cmd">cmd</option>
        <option value="none">Direct exe</option>
      </Select>
      <p class="field__hint">
        {shell === "powershell"
          ? "PowerShell runs the command as a script, so you can chain statements and use its cmdlets."
          : shell === "cmd"
            ? "cmd runs the command through the classic console shell."
            : "Direct exe launches the executable itself — no shell wraps it, so environment syntax like $VAR or %VAR% is not expanded."}
      </p>
    </div>

    <div class="field">
      <label class="field__label" for="command-line">Command</label>
      <textarea
        id="command-line"
        class="field__cmd"
        rows="3"
        placeholder={shell === "none" ? 'e.g. C:\\Tools\\dev-server.exe --port 8080' : shell === "powershell" ? 'e.g. nvm use 22 && node server.js' : 'e.g. start "" http://localhost:3000'}
        autocomplete="off"
        spellcheck="false"
        value={command}
        oninput={(e) => onCommandInput((e.target as HTMLTextAreaElement).value)}
      ></textarea>
      <p class="field__hint">
        {shell === "powershell"
          ? "Multi-line scripts work — each line runs as part of one PowerShell command."
          : shell === "cmd"
            ? "Runs as: cmd /c {command}"
            : "Runs the command line as-is; quote paths that contain spaces."}
      </p>
    </div>

    <label class="showwin">
      <input
        type="checkbox"
        class="showwin__check"
        checked={showWindow}
        onchange={(e) => (showWindow = (e.target as HTMLInputElement).checked)}
      />
      <span class="showwin__body">
        <span class="showwin__title">Show a window</span>
        <span class="showwin__hint">
          Hidden by default — the command runs with no console window. Turn this
          on to see the window the command creates.
        </span>
      </span>
    </label>

    <TextInput
      id="command-name"
      label="Name"
      required
      placeholder="e.g. dev server"
      value={name}
      onchange={onNameInput}
      hint="Suggestions come from the command; edit freely."
    />

    <div class="test">
      <div class="test__row">
        <Button variant="secondary" disabled={testing || !command.trim()} onclick={runTest}>
          <Icon name="play" size={13} />
          {testing ? "Testing…" : "Test"}
        </Button>
        <p class="test__note">
          Runs the command timeboxed (20 s) and reports the exit code and output —
          a command that outlives the box is interactive, not headless-verifiable.
        </p>
      </div>
      {#if tested && !testing}
        {#if test?.timed_out}
          <div class="test__result test__result--timeout" role="status">
            <p class="test__verdict">
              Timed out — not headless-verifiable. The command is interactive
              (or hangs); it was killed after 20 s.
            </p>
          </div>
        {:else}
          <div class="test__result" role="status">
            <p class="test__verdict">
              Exit code: <strong class="mono">{test?.exit_code ?? "—"}</strong>
              {test?.exit_code === 0
                ? " — started cleanly."
                : test?.exit_code === null
                  ? " — could not start."
                  : " — the command reported a failure."}
            </p>
            {#if test?.output.trim()}
              <pre class="test__output">{test?.output}</pre>
            {/if}
          </div>
        {/if}
      {/if}
    </div>

    {#if error}
      <p class="form__error" role="alert">{error}</p>
    {/if}

    <div class="form__actions">
      <Button variant="secondary" onclick={oncancel} disabled={saving || testing}>
        Cancel
      </Button>
      <Button kind="submit" disabled={saving || testing}>
        {saving ? "Adding…" : "Add command"}
      </Button>
    </div>
  </form>
</Dialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .field__label {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .field__cmd {
    width: 100%;
    resize: vertical;
    min-height: 64px;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
    background: var(--bg-page);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 8px 10px;
    transition: border-color var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out);
  }

  .field__cmd:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .field__cmd::placeholder {
    color: var(--text-muted);
    opacity: 0.75;
  }

  .field__hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .showwin {
    display: flex;
    align-items: flex-start;
    gap: var(--space-2);
    cursor: pointer;
  }

  .showwin__check {
    margin: 2px 0 0;
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }

  .showwin__body {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .showwin__title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .showwin__hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .test {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border: 1px dashed var(--border-strong);
    border-radius: var(--radius);
    padding: var(--space-3);
  }

  .test__row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
  }

  .test__note {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .test__result {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-page);
    padding: var(--space-2) var(--space-3);
  }

  .test__result--timeout {
    border-color: var(--danger-tint-border);
    background: var(--danger-tint);
  }

  .test__verdict {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text);
  }

  .test__output {
    margin: 0;
    max-height: 160px;
    overflow: auto;
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    line-height: var(--leading-normal);
    color: var(--text-muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .form__error {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--danger-text);
    overflow-wrap: anywhere;
  }

  .form__actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    margin-top: var(--space-2);
  }

  .mono {
    font-family: var(--font-mono);
  }
</style>