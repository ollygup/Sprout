<script lang="ts">
  import type { Group, QuickAction } from "$lib/types";
  import {
    assignToGroup,
    createGroup,
    createQuickAction,
    testQuickAction,
    unassignFromGroup,
    updateQuickAction,
  } from "$lib/api";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import InfoTip from "./InfoTip.svelte";
  import Select from "./Select.svelte";
  import TestResult from "./TestResult.svelte";

  let {
    open,
    action,
    groups = [],
    groupsEnabled = false,
    onsave,
    oncancel,
  }: {
    open: boolean;
    /** The action being edited; null = adding a new action. */
    action: QuickAction | null;
    /** Live `action`-collection groups (ticket 131): empty = no group field
     *  at all (research 0004 rule 2, 0006 patterns 2/11); otherwise an
     *  optional picker defaulting to ungrouped, with a create-and-place
     *  `New group…` entry (0006 pattern 10). */
    groups?: Group[];
    /** The collection's Groups switch (ticket 89): off is fully dormant —
     *  stored groups are never shown or touched, so the picker stays absent
     *  even while groups exist (0006 pattern 12). */
    groupsEnabled?: boolean;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let command = $state("");
  let cwd = $state("");
  let note = $state("");
  let stoppable = $state(false);
  let stopCommand = $state("");
  let saving = $state(false);
  let error = $state("");
  let testing = $state(false);
  /** Group picker selection: "" = ungrouped, a group id, or NEW_GROUP. */
  let groupPick = $state("");
  let newGroupName = $state("");

  const NEW_GROUP = "__new__";

  const editing = $derived(action !== null);

  $effect(() => {
    if (open) {
      name = action?.name ?? "";
      command = action?.command ?? "";
      cwd = action?.cwd ?? "";
      note = action?.note ?? "";
      stoppable = action?.stoppable ?? false;
      stopCommand = action?.stop_command ?? "";
      saving = false;
      error = "";
      // Default ungrouped; an edit preselects its current group when that
      // group is still live.
      groupPick =
        action?.group_id != null &&
        groups.some((g) => g.id === action.group_id)
          ? String(action.group_id)
          : "";
      newGroupName = "";
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
    // Group placement (ticket 131): the picker exists only while the Groups
    // switch is on and live groups do, and the New-group entry needs a name
    // — validated inline like the GroupNameDialog's, before anything is
    // created.
    const placing = groupsEnabled && groups.length > 0;
    const creatingGroup = placing && groupPick === NEW_GROUP;
    const trimmedGroupName = newGroupName.trim();
    if (creatingGroup && !trimmedGroupName) {
      error = "Give the new group a name.";
      return;
    }
    saving = true;
    error = "";
    try {
      const trimmedNote = note.trim() || null;
      if (editing && action) {
        await updateQuickAction({
          ...action,
          name: name.trim(),
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
        });
        // Group membership rides outside the edit payload (ticket 89) — the
        // same assign/unassign the row menu uses.
        if (placing) {
          if (creatingGroup) {
            const created = await createGroup("action", trimmedGroupName);
            await assignToGroup("action", action.id, created.id);
          } else if (groupPick === "") {
            if (action.group_id !== null) {
              await unassignFromGroup("action", action.id);
            }
          } else if (Number(groupPick) !== action.group_id) {
            await assignToGroup("action", action.id, Number(groupPick));
          }
        }
        await onsave(`${name.trim()} saved.`);
      } else {
        const created = await createQuickAction({
          name: name.trim(),
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
        });
        if (placing) {
          if (creatingGroup) {
            const group = await createGroup("action", trimmedGroupName);
            await assignToGroup("action", created.id, group.id);
          } else if (groupPick !== "") {
            await assignToGroup("action", created.id, Number(groupPick));
          }
        }
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

    {#if groupsEnabled && groups.length > 0}
      <div class="field">
        <div class="field__label-row">
          <label class="field__label" for="qa-group">Group</label>
          <InfoTip label="How groups work">
            <p>Optional — ungrouped by default. Pick a group or create one to place this action in.</p>
          </InfoTip>
        </div>
        <Select
          id="qa-group"
          value={groupPick}
          onchange={(v) => (groupPick = v)}
        >
          <option value="">Ungrouped</option>
          {#each groups as group (group.id)}
            <option value={String(group.id)}>{group.name}</option>
          {/each}
          <option value={NEW_GROUP}>New group…</option>
        </Select>
        {#if groupPick === NEW_GROUP}
          <TextInput
            id="qa-new-group"
            label="New group name"
            required
            placeholder="e.g. Docker maintenance"
            value={newGroupName}
            onchange={(v) => (newGroupName = v)}
          />
        {/if}
      </div>
    {/if}

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-note">Notes</label>
        <InfoTip label="How notes work">
          <p>Optional text for whatever you want to record about this action.</p>
        </InfoTip>
      </div>
      <textarea
        id="qa-note"
        class="field__cmd field__cmd--note"
        rows="4"
        placeholder="Optional — e.g. when to use it, caveats…"
        autocomplete="off"
        spellcheck="true"
        value={note}
        oninput={(e) => (note = (e.target as HTMLTextAreaElement).value)}
      ></textarea>
      <p class="field__hint">Plain text — use - or * for bullets, 1. for numbered steps. Blank line starts a new paragraph.</p>
    </div>

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

  .field__cmd--note {
    min-height: 88px;
    font-family: var(--font-body);
  }

  .field__hint {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
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
