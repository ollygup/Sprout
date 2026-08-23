<script lang="ts">
  import type { QuickAction } from "$lib/types";
  import { createQuickAction, testQuickAction, updateQuickAction } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import InfoTip from "./InfoTip.svelte";
  import TestResult from "./TestResult.svelte";

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
  let stoppable = $state(false);
  let stopCommand = $state("");
  let saving = $state(false);
  let error = $state("");
  let testing = $state(false);

  const editing = $derived(action !== null);

  $effect(() => {
    if (open) {
      name = action?.name ?? "";
      command = action?.command ?? "";
      cwd = action?.cwd ?? "";
      stoppable = action?.stoppable ?? false;
      stopCommand = action?.stop_command ?? "";
      saving = false;
      error = "";
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
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
        });
        await onsave(`${name.trim()} saved.`);
      } else {
        await createQuickAction({
          name: name.trim(),
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
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

    <label class="stoppable">
      <input
        type="checkbox"
        class="stoppable__check"
        checked={stoppable}
        onchange={(e) => (stoppable = (e.target as HTMLInputElement).checked)}
      />
      <span class="stoppable__title">Show Stop button</span>
      <InfoTip label="What the Stop button does">
        <p>
          While the command runs, its Run button becomes Stop. Tracking covers
          foreground commands only — detached commands (e.g.
          <span class="mono">docker compose up -d</span>) report as not running
          because the process exits while the service continues.
        </p>
      </InfoTip>
    </label>

    {#if stoppable}
      <div class="field">
        <div class="field__label-row">
          <label class="field__label" for="qa-stop-command">Stop command</label>
          <InfoTip label="How the stop command works">
            <p>Runs when Stop is clicked. Empty = kills the process tree.</p>
          </InfoTip>
        </div>
        <textarea
          id="qa-stop-command"
          class="field__cmd"
          rows="2"
          placeholder="e.g. docker compose stop"
          autocomplete="off"
          spellcheck="false"
          value={stopCommand}
          oninput={(e) => (stopCommand = (e.target as HTMLTextAreaElement).value)}
        ></textarea>
      </div>
    {/if}

    <TestResult
      {open}
      {command}
      bind:testing
      validate={() => cwdError(cwd)}
      probe={() => testQuickAction(command.trim(), cwd.trim() || null)}
      onerror={(message) => (error = message)}
    />

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

  .stoppable {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    cursor: pointer;
  }

  .stoppable__check {
    margin: 0;
    accent-color: var(--accent);
    width: 14px;
    height: 14px;
  }

  .stoppable__title {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
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
