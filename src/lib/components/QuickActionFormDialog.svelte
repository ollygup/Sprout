<script lang="ts">
  import type { LaunchCommandTest, QuickAction } from "$lib/types";
  import { createQuickAction, testQuickAction, updateQuickAction } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import InfoTip from "./InfoTip.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    action,
    onsave,
    oncancel,
  }: {
    open: boolean;
    /** The action being edited; null = adding a new action. */
    action: QuickAction | null;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let command = $state("");
  let cwd = $state("");
  let saving = $state(false);
  let error = $state("");
  let test: LaunchCommandTest | null = $state(null);
  let testing = $state(false);
  let tested = $state(false);

  const editing = $derived(action !== null);

  $effect(() => {
    if (open) {
      name = action?.name ?? "";
      command = action?.command ?? "";
      cwd = action?.cwd ?? "";
      saving = false;
      error = "";
      test = null;
      testing = false;
      tested = false;
    }
  });

  /** The working-directory rule, mirroring the backend (ticket 50): when set,
   *  it must be an absolute path — a relative one would silently mean
   *  different things per machine. */
  function cwdError(value: string): string | null {
    const trimmed = value.trim();
    if (!trimmed) return null;
    if (!/^[A-Za-z]:[\\/]/.test(trimmed) && !/^\\\\/.test(trimmed)) {
      return `'${trimmed}' is not an absolute path — the working directory must be a full path like D:\Work`;
    }
    return null;
  }

  async function runTest() {
    if (!command.trim()) {
      error = "Type a command first.";
      return;
    }
    const badCwd = cwdError(cwd);
    if (badCwd) {
      error = badCwd;
      return;
    }
    error = "";
    tested = true;
    testing = true;
    test = null;
    try {
      test = await testQuickAction(command.trim(), cwd.trim() || null);
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      testing = false;
    }
  }

  async function submit() {
    if (!name.trim()) {
      error = "Give the action a name.";
      return;
    }
    if (!command.trim()) {
      error = "The command must not be empty.";
      return;
    }
    const badCwd = cwdError(cwd);
    if (badCwd) {
      error = badCwd;
      return;
    }
    saving = true;
    error = "";
    try {
      if (editing && action) {
        await updateQuickAction({
          ...action,
          name: name.trim(),
          command: command.trim(),
          cwd: cwd.trim() || null,
        });
        await onsave(`${name.trim()} saved.`);
      } else {
        await createQuickAction({
          name: name.trim(),
          command: command.trim(),
          cwd: cwd.trim() || null,
        });
        await onsave(`${name.trim()} added to Quick Actions.`);
      }
    } catch (e) {
      console.error(e);
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog
  {open}
  title={editing ? "Edit quick action" : "Add a quick action"}
  onclose={oncancel}
  width={560}
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    <TextInput
      id="qa-name"
      label="Name"
      required
      autofocus
      placeholder="e.g. docker start"
      value={name}
      onchange={(v) => (name = v)}
      info="Where the name is shown"
    >
      {#snippet infobody()}
        <p>Shown in the Quick Actions tab.</p>
      {/snippet}
    </TextInput>

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-command">Command</label>
        <InfoTip label="How the command runs">
          <p>PowerShell script; runs with -NoProfile -NonInteractive. Multi-line is fine.</p>
        </InfoTip>
      </div>
      <textarea
        id="qa-command"
        class="field__cmd"
        rows="6"
        placeholder="e.g. docker compose up -d"
        autocomplete="off"
        spellcheck="false"
        value={command}
        oninput={(e) => (command = (e.target as HTMLTextAreaElement).value)}
      ></textarea>
    </div>

    <TextInput
      id="qa-cwd"
      label="Working directory"
      placeholder="e.g. D:\Work"
      value={cwd}
      onchange={(v) => (cwd = v)}
      info="How the working directory works"
    >
      {#snippet infobody()}
        <p>Working directory; empty = the app's folder.</p>
      {/snippet}
    </TextInput>

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
        {saving
          ? editing
            ? "Saving…"
            : "Adding…"
          : editing
            ? "Save changes"
            : "Add action"}
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

  .field__label-row {
    display: flex;
    align-items: center;
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
