<script lang="ts">
  import { tick } from "svelte";
  import type { Group, QuickAction, QuickActionShell } from "$lib/types";
  import type {
    AiApprovedRoot,
    AiBoundTarget,
    AiDiscoveryScope,
    AiFileContent,
    AiFindOutcome,
  } from "$lib/types";
  import { quickActionShellLabel } from "$lib/types";
  import type { AiDraftOutcome } from "$lib/types";
  import {
    aiApproveDisclosure,
    aiApproveRoot,
    aiBindTarget,
    aiCancelDraft,
    aiCheckCandidate,
    aiFindTargets,
    aiGenerateDraft,
    aiListApprovedRoots,
    aiReadTargetFile,
    aiRevokeRoot,
    assignToGroup,
    createGroup,
    createQuickAction,
    testQuickAction,
    unassignFromGroup,
    updateQuickAction,
  } from "$lib/api";
  import { open as openFolderPicker } from "@tauri-apps/plugin-dialog";
  import Dialog from "./Dialog.svelte";
  import Button from "./Button.svelte";
  import TextInput from "./TextInput.svelte";
  import Disclosure from "./Disclosure.svelte";
  import InfoTip from "./InfoTip.svelte";
  import Notice from "./Notice.svelte";
  import Select from "./Select.svelte";
  import TestResult from "./TestResult.svelte";

  let {
    open,
    action,
    groups = [],
    groupsEnabled = false,
    aiReady = false,
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
    /** Whether AI drafting is configured and ready: the dialog gate for the
     *  drafting block. Optional assistance stays invisible until deliberately
     *  set up in Settings (ADR-0031), so an unready route renders zero AI
     *  chrome instead of a disabled teaser. The backend remains the
     *  generation gate; this is disclosure only. */
    aiReady?: boolean;
    onsave: (message: string) => void | Promise<void>;
    oncancel: () => void;
  } = $props();

  let name = $state("");
  let shell = $state<QuickActionShell>("powershell");
  let command = $state("");
  let cwd = $state("");
  let note = $state("");
  let stoppable = $state(false);
  let stopCommand = $state("");
  let autoRun = $state(false);
  let showInDock = $state(true);
  let detailsOpen = $state(false);
  let saving = $state(false);
  let error = $state("");
  let testing = $state(false);
  // AI drafting (ADR-0030): an explicit-shell request becomes a reviewable
  // candidate the user applies into the fields above and saves normally.
  // Generation never runs, tests, or stops anything — this section never
  // calls the run/test controls. `aiSeq` drops late completions so a slow
  // answer can never replace a newer draft or claim a cancelled success. It
  // is deliberately NOT reactive state: nothing renders it, and a
  // read-modify-write inside the reset effect below would retrigger that
  // effect forever.
  let aiOpen = $state(false);
  let aiRequest = $state("");
  let aiContext = $state("");
  let aiPending = $state(false);
  let aiSeq = 0;
  let aiRequestId: string | null = null;
  let aiOutcome = $state<AiDraftOutcome | null>(null);
  let aiNotice = $state("");
  let aiApplied = $state<{ shell: QuickActionShell; command: string } | null>(null);
  // The draft under review, if any — flat derivations keep the review
  // markup free of inline declarations and narrowing chains.
  const aiDraft = $derived(aiOutcome?.kind === "draft" ? aiOutcome.draft : null);
  const aiRefusal = $derived(
    aiOutcome?.kind === "refused" || aiOutcome?.kind === "clarify" ? aiOutcome.message : null
  );
  const aiFailure = $derived(aiOutcome?.kind === "failed" ? aiOutcome.message : null);
  const aiAppliedCurrent = $derived(
    aiDraft !== null &&
      aiApplied !== null &&
      aiApplied.shell === shell &&
      aiApplied.command === command.trim()
  );
  /** Group picker selection: "" = ungrouped, a group id, or NEW_GROUP. */
  let groupPick = $state("");
  let newGroupName = $state("");

  const NEW_GROUP = "__new__";

  const editing = $derived(action !== null);

  $effect(() => {
    if (open) {
      if (aiRequestId) void aiCancelDraft(aiRequestId).catch(() => {});
      aiRequestId = null;
      name = action?.name ?? "";
      shell = action?.shell ?? "powershell";
      command = action?.command ?? "";
      cwd = action?.cwd ?? "";
      note = action?.note ?? "";
      stoppable = action?.stoppable ?? false;
      stopCommand = action?.stop_command ?? "";
      autoRun = action?.auto_run ?? false;
      showInDock = action?.show_in_dock ?? true;
      detailsOpen = false;
      saving = false;
      error = "";
      // AI-first on Add, manual-first on Edit: a new action starts from the
      // drafting block when one is available, while an edit keeps the
      // author's fields in front (ADR-0030 keeps applied drafts reviewable
      // either way — generation never fills anything unreviewed).
      aiOpen = aiReady && action === null;
      aiRequest = "";
      aiContext = "";
      aiPending = false;
      aiSeq += 1;
      aiOutcome = null;
      aiNotice = "";
      aiApplied = null;
      findQuery = "";
      findScope = "apps";
      findPending = false;
      findSeq += 1;
      findOutcome = null;
      findPick = "";
      findNotice = "";
      roots = [];
      rootsNotice = "";
      boundTarget = null;
      filePreview = null;
      if (aiOpen) void loadRoots();
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

  async function generateDraft() {
    if (aiPending) return;
    if (!aiRequest.trim()) {
      aiOutcome = null;
      aiNotice = "Describe what the action should do, then generate.";
      return;
    }
    aiPending = true;
    aiOutcome = null;
    aiNotice = "";
    error = "";
    const mine = ++aiSeq;
    const requestId = `draft-${Date.now()}-${mine}`;
    aiRequestId = requestId;
    try {
      const outcome = await aiGenerateDraft(
        aiRequest.trim(),
        shell,
        aiContext.trim() || null,
        requestId,
      );
      if (mine !== aiSeq) return;
      aiOutcome = outcome;
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } catch (e) {
      console.error(e);
      if (mine !== aiSeq) return;
      aiOutcome = { kind: "failed", message: String(e) };
      await tick();
      document.getElementById("ai-outcome")?.focus();
    } finally {
      if (mine === aiSeq) {
        aiPending = false;
        aiRequestId = null;
      }
    }
  }

  function cancelDraft() {
    if (aiRequestId) void aiCancelDraft(aiRequestId).catch(() => {});
    aiRequestId = null;
    aiSeq += 1;
    aiPending = false;
    aiNotice = "Generation cancelled — nothing saved.";
  }

  function applyDraft() {
    if (aiOutcome?.kind !== "draft") return;
    command = aiOutcome.draft.command;
    shell = aiOutcome.draft.shell;
    aiApplied = { shell: aiOutcome.draft.shell, command: aiOutcome.draft.command };
    aiNotice = "";
    error = "";
  }

  function dismissDraft() {
    aiSeq += 1;
    aiOutcome = null;
    aiApplied = null;
    aiNotice = "";
  }

  // AI local-target search (ADR-0031): an explicit find over installed apps
  // and approved folders returns opaque references; the user picks a match
  // and trusted code binds it into the command below. Finding reads
  // names/paths only — contents need the separate preview, disclosure needs
  // its own approval, and nothing here runs. `findSeq` drops late answers
  // the same way `aiSeq` does for drafts.
  let findQuery = $state("");
  let findScope = $state<AiDiscoveryScope>("apps");
  let findPending = $state(false);
  let findSeq = 0;
  let findOutcome = $state<AiFindOutcome | null>(null);
  let findPick = $state("");
  let findNotice = $state("");
  let roots = $state<AiApprovedRoot[]>([]);
  let rootsNotice = $state("");
  let boundTarget = $state<AiBoundTarget | null>(null);
  let filePreview = $state<AiFileContent | null>(null);
  const findPicked = $derived(
    findOutcome?.matches.find((m) => m.ref_id === findPick) ?? null
  );

  async function loadRoots() {
    rootsNotice = "";
    try {
      roots = await aiListApprovedRoots();
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function approveRoot() {
    rootsNotice = "";
    try {
      const picked = await openFolderPicker({ directory: true, multiple: false });
      if (typeof picked !== "string" || !picked) return;
      const root = await aiApproveRoot(picked);
      roots = [...roots.filter((r) => r.path !== root.path), root];
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function revokeRoot(path: string) {
    rootsNotice = "";
    try {
      await aiRevokeRoot(path);
      roots = roots.filter((r) => r.path !== path);
    } catch (e) {
      console.error(e);
      rootsNotice = String(e);
    }
  }

  async function runFind() {
    if (findPending) return;
    if (!findQuery.trim()) {
      findNotice = "Type an app or file name, then find.";
      return;
    }
    findPending = true;
    findNotice = "";
    filePreview = null;
    const mine = ++findSeq;
    try {
      const outcome = await aiFindTargets(findQuery.trim(), findScope);
      if (mine !== findSeq) return;
      findOutcome = outcome;
      findPick = outcome.matches.length === 1 ? outcome.matches[0].ref_id : "";
      if (outcome.matches.length === 0) {
        findNotice = outcome.notice ?? "No match.";
      }
    } catch (e) {
      console.error(e);
      if (mine !== findSeq) return;
      findNotice = String(e);
    } finally {
      if (mine === findSeq) findPending = false;
    }
  }

  function cancelFind() {
    findSeq += 1;
    findPending = false;
    findNotice = "Search cancelled — nothing saved.";
  }

  async function useTarget() {
    const pick = findOutcome?.matches.find((m) => m.ref_id === findPick);
    if (!pick) {
      findNotice = "Pick a match first.";
      return;
    }
    try {
      const bound = await aiBindTarget(pick.ref_id, shell);
      command = bound.command;
      shell = bound.shell;
      boundTarget = bound;
      error = "";
      findNotice = "";
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  function dismissBound() {
    boundTarget = null;
  }

  async function previewFile(refId: string) {
    filePreview = null;
    findNotice = "";
    try {
      filePreview = await aiReadTargetFile(refId);
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  async function approvePreviewDisclosure() {
    if (!filePreview) return;
    try {
      await aiApproveDisclosure(filePreview.ref_id, ["name", "path", "contents"]);
      findNotice =
        "Recorded. Nothing is sent anywhere by this build — a later cloud step must ask again before uploading.";
    } catch (e) {
      console.error(e);
      findNotice = String(e);
    }
  }

  function aiKeydown(event: KeyboardEvent) {
    // Ctrl/Cmd+Enter generates here instead of submitting the whole dialog —
    // preventing default keeps the shared Dialog handler's precedence rule.
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void generateDraft();
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
    // Content applied from a draft and then edited is rechecked before it
    // may save on AI authority: a refusal or clarification blocks with an
    // escape hatch to manual saving, never a silent pass. Unchanged applied
    // text was already checked at generation; purely manual text skips this.
    if (
      aiApplied &&
      (shell !== aiApplied.shell || command.trim() !== aiApplied.command)
    ) {
      try {
        const verdict = await aiCheckCandidate(shell, command.trim());
        if (verdict.verdict !== "allow") {
          error = `${verdict.message} Discard the draft below to save this as your own manual text instead.`;
          return;
        }
      } catch (e) {
        console.error(e);
      }
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
          shell,
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
          auto_run: autoRun,
          show_in_dock: showInDock,
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
          shell,
          command: command.trim(),
          cwd: cwd.trim() || null,
          stoppable,
          stop_command: stoppable ? stopCommand.trim() || null : null,
          note: trimmedNote,
          auto_run: autoRun,
          show_in_dock: showInDock,
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
  focusTarget={aiReady && !editing ? "#qa-ai-request" : undefined}
>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      submit();
    }}
  >
    {#if aiReady}
      <!-- AI drafting leads the dialog while ready (never in the compact
           dock/window): describe the task, review the candidate, apply it
           into the fields below, then validate and save normally. Frequent
           intent first, rare fields behind Details — two levels max. -->
      <div class="ai">
        <Disclosure
          open={aiOpen}
          controls="qa-ai-body"
          label="Draft with AI"
          onclick={() => {
            aiOpen = !aiOpen;
            if (aiOpen) void loadRoots();
          }}
        />

        <div id="qa-ai-body" class="ai__body" hidden={!aiOpen}>
          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-ai-request">What should it do</label>
              <InfoTip label="How AI drafting works">
                <p>Drafts for the {quickActionShellLabel[shell]} shell selected below. Only what you type here is sent — nothing is read from disk or the app. Drafts never run; you review and save each one.</p>
              </InfoTip>
            </div>
            <textarea
              id="qa-ai-request"
              name="ai-request"
              class="field__cmd"
              rows="3"
              maxlength="2000"
              placeholder="e.g. show the status of the print spooler service"
              autocomplete="off"
              spellcheck="true"
              value={aiRequest}
              oninput={(e) => (aiRequest = (e.target as HTMLTextAreaElement).value)}
              onkeydown={aiKeydown}
            ></textarea>
          </div>

          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-ai-context">Extra context (optional)</label>
            </div>
            <textarea
              id="qa-ai-context"
              name="ai-context"
              class="field__cmd"
              rows="2"
              maxlength="2000"
              placeholder="e.g. service names, paths to mention"
              autocomplete="off"
              spellcheck="true"
              value={aiContext}
              oninput={(e) => (aiContext = (e.target as HTMLTextAreaElement).value)}
              onkeydown={aiKeydown}
            ></textarea>
          </div>

          <!-- Local-target search: installed apps and approved folders by
               name. Nothing is read or run — pick a match to bind its path
               into the command below, then review and save normally. -->
          <div class="field">
            <div class="field__label-row">
              <label class="field__label" for="qa-find-scope">Search in</label>
              <InfoTip label="How local target search works">
                <p>Searches installed apps and folders you approve below — names and paths only. You pick a match; Sprout quotes its path into the command. Nothing is read or run during the search.</p>
              </InfoTip>
            </div>
            <Select
              id="qa-find-scope"
              value={findScope}
              onchange={(v) => (findScope = v as AiDiscoveryScope)}
            >
              <option value="apps">Installed apps</option>
              <option value="files">Approved folders</option>
              <option value="both">Both</option>
            </Select>
          </div>

          <TextInput
            id="qa-find-query"
            label="Find a local target"
            placeholder="e.g. notepad"
            value={findQuery}
            onchange={(v) => (findQuery = v)}
            info="What the search covers"
          >
            {#snippet infobody()}
              <p>Matches names, not contents. Ambiguous names list every match for you to choose from.</p>
            {/snippet}
          </TextInput>

          <div class="field">
            <div class="field__label-row">
              <span class="field__label" id="qa-roots-label">Approved folders</span>
              <InfoTip label="How approved folders work">
                <p>Only folders you approve are searched — names and paths, never contents. Revoking forgets a folder, and earlier matches from it stop binding.</p>
              </InfoTip>
            </div>
            {#if roots.length === 0}
              <p class="ai__meta">None yet — folder search starts after you approve one.</p>
            {:else}
              <ul class="find__roots" aria-labelledby="qa-roots-label">
                {#each roots as root (root.path)}
                  <li class="find__root">
                    <span class="find__path">{root.path}</span>
                    <Button type="button" variant="ghost" onclick={() => void revokeRoot(root.path)}>
                      Revoke
                    </Button>
                  </li>
                {/each}
              </ul>
            {/if}
            <div class="ai__actions">
              <Button type="button" variant="secondary" onclick={() => void approveRoot()}>
                Approve a folder…
              </Button>
            </div>
            {#if rootsNotice}
              <p class="ai__status" role="status">{rootsNotice}</p>
            {/if}
          </div>

          <div class="ai__actions">
            {#if findPending}
              <p class="ai__status" role="status">Searching…</p>
              <Button type="button" variant="ghost" onclick={cancelFind}>Cancel</Button>
            {:else}
              <Button
                type="button"
                variant="secondary"
                onclick={() => void runFind()}
              >
                Find target
              </Button>
            {/if}
          </div>

          {#if findNotice}
            <p class="ai__status" role="status">{findNotice}</p>
          {/if}

          {#if findOutcome && findOutcome.matches.length > 0}
            <div class="ai__outcome">
              {#if findOutcome.notice}
                <p class="ai__meta">{findOutcome.notice}</p>
              {/if}
              {#if findOutcome.truncated}
                <p class="ai__meta">Showing the first {findOutcome.matches.length} matches.</p>
              {/if}
              <div class="find__list" role="radiogroup" aria-label="Matching targets">
                {#each findOutcome.matches as match (match.ref_id)}
                  <label class="find__row">
                    <input
                      type="radio"
                      name="qa-find-pick"
                      value={match.ref_id}
                      checked={findPick === match.ref_id}
                      onchange={() => (findPick = match.ref_id)}
                    />
                    <span class="find__name">{match.name}</span>
                    <span class="find__path">{match.path}</span>
                    {#if match.kind !== "app"}
                      <span class="find__kind">{match.kind}</span>
                    {/if}
                  </label>
                {/each}
              </div>
              <div class="ai__actions">
                <Button type="button" variant="secondary" onclick={() => void useTarget()}>
                  Use this target
                </Button>
                {#if findPicked?.kind === "file"}
                  <Button type="button" variant="ghost" onclick={() => void previewFile(findPicked.ref_id)}>
                    Preview contents
                  </Button>
                {/if}
              </div>
              {#if filePreview}
                <p class="ai__meta">Preview of {filePreview.path}{filePreview.truncated ? " (first 8 KB)" : ""} — shown only to you, never sent anywhere.</p>
                <p class="ai__command">{filePreview.content}</p>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={() => void approvePreviewDisclosure()}>
                    Approve sending this to the model
                  </Button>
                  <Button type="button" variant="ghost" onclick={() => (filePreview = null)}>
                    Hide preview
                  </Button>
                </div>
              {/if}
            </div>
          {/if}

          {#if boundTarget}
            <div class="ai__outcome">
              <p class="ai__flag">Not run — review before saving.</p>
              <p class="ai__shell">{quickActionShellLabel[boundTarget.shell]}</p>
              <p class="ai__command">{boundTarget.command}</p>
              <p class="ai__meta">Opens: {boundTarget.target}</p>
              {#if boundTarget.warning}
                <Notice tone="warn">{boundTarget.warning}</Notice>
              {/if}
              <div class="ai__actions">
                <Button type="button" variant="ghost" onclick={dismissBound}>Dismiss</Button>
              </div>
            </div>
          {/if}

          <div class="ai__actions">
            {#if aiPending}
              <p class="ai__status" role="status">Drafting…</p>
              <Button type="button" variant="ghost" onclick={cancelDraft}>Cancel</Button>
            {:else}
              <Button
                type="button"
                variant="secondary"
                onclick={() => void generateDraft()}
              >
                Generate draft
              </Button>
            {/if}
          </div>

          {#if aiNotice}
            <p class="ai__status" role="status">{aiNotice}</p>
          {/if}

          {#if aiOutcome}
            <div id="ai-outcome" tabindex="-1" class="ai__outcome">
              {#if aiDraft}
                <p class="ai__flag">Not run — review before saving.</p>
                <p class="ai__shell">{quickActionShellLabel[aiDraft.shell]}</p>
                <p class="ai__command">{aiDraft.command}</p>
                {#if aiDraft.assumptions.length > 0}
                  <p class="ai__meta">Assumes: {aiDraft.assumptions.join("; ")}</p>
                {/if}
                {#if aiDraft.affected_targets.length > 0}
                  <p class="ai__meta">Touches: {aiDraft.affected_targets.join("; ")}</p>
                {/if}
                {#if aiDraft.explanation}
                  <p class="ai__explanation">{aiDraft.explanation}</p>
                {/if}
                <div class="ai__actions">
                  {#if aiAppliedCurrent}
                    <p class="ai__status" role="status">
                      Applied — this exact text is in the fields. Edit freely; saving an edit saves your own text.
                    </p>
                  {:else}
                    <Button type="button" variant="secondary" onclick={applyDraft}>
                      Use this draft
                    </Button>
                  {/if}
                  <Button type="button" variant="ghost" onclick={dismissDraft}>Dismiss</Button>
                </div>
              {:else if aiRefusal}
                <Notice tone="warn">{aiRefusal}</Notice>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissDraft}>Dismiss</Button>
                </div>
              {:else if aiFailure}
                <Notice tone="error">{aiFailure}</Notice>
                <div class="ai__actions">
                  <Button type="button" variant="ghost" onclick={dismissDraft}>Dismiss</Button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/if}

    <TextInput
      id="qa-name"
      label="Name"
      required
      autofocus={!aiReady || editing}
      placeholder="e.g. docker start"
      value={name}
      onchange={(v) => (name = v)}
    />

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-shell">Shell</label>
        <InfoTip label="How the shell works">
          <p>Runs hidden as the current user. The same text under a different shell is a different action.</p>
        </InfoTip>
      </div>
      <Select
        id="qa-shell"
        value={shell}
        onchange={(v) => (shell = v as QuickActionShell)}
      >
        <option value="powershell">{quickActionShellLabel.powershell}</option>
        <option value="cmd">{quickActionShellLabel.cmd}</option>
      </Select>
      <p class="field__hint">
        {shell === "powershell"
          ? "PowerShell runs with -NoProfile -NonInteractive. Multi-line is fine."
          : "cmd runs as: cmd /c {command}."}
      </p>
    </div>

    <div class="field">
      <div class="field__label-row">
        <label class="field__label" for="qa-command">Command</label>
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

    <!-- Rarely-touched fields behind one Details disclosure: frequent intent
         stays up front while the layout stays scannable without a new page.
         Each choice applies on Save, never immediately. -->
    <div class="advanced">
      <Disclosure
        open={detailsOpen}
        controls="qa-details-body"
        label="Details"
        onclick={() => (detailsOpen = !detailsOpen)}
      />

      <div id="qa-details-body" class="advanced__body" hidden={!detailsOpen}>
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
              <p>Optional — for whatever you want to record. Plain text: - or * for bullets, 1. for numbered steps.</p>
            </InfoTip>
          </div>
          <textarea
            id="qa-note"
            class="field__cmd field__cmd--note"
            rows="4"
            placeholder="e.g. when to use it, caveats…"
            autocomplete="off"
            spellcheck="true"
            value={note}
            oninput={(e) => (note = (e.target as HTMLTextAreaElement).value)}
          ></textarea>
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

        <label class="stoppable">
          <input
            type="checkbox"
            class="stoppable__check"
            checked={autoRun}
            onchange={(e) => (autoRun = (e.target as HTMLInputElement).checked)}
          />
          <span class="stoppable__title">Run at Sprout start</span>
          <InfoTip label="What running at start does">
            <p>Runs once each time Sprout starts, in list order, as if Run were clicked.</p>
          </InfoTip>
        </label>

        <!-- Dock visibility: the control lives on its object — hiding is
             per-item, not a feature switch. -->
        <label class="stoppable">
          <input
            type="checkbox"
            class="stoppable__check"
            checked={showInDock}
            onchange={(e) => (showInDock = (e.target as HTMLInputElement).checked)}
          />
          <span class="stoppable__title">Show in dock</span>
          <InfoTip label="What showing in the dock does">
            <p>Uncheck to hide this action from the Quick Launch dock. It stays here and stays runnable.</p>
          </InfoTip>
        </label>
      </div>
    </div>

    <TestResult
      {open}
      {command}
      bind:testing
      validate={() => cwdError(cwd)}
      probe={() => testQuickAction(shell, command.trim(), cwd.trim() || null)}
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

  /* Details-collapsed rare options: the same flat disclosure treatment as
     the product form's Advanced — no frame, body separated by a dashed rule.
     `hidden` needs its own rule or the flex display above keeps the panel
     permanently open. */
  .advanced {
    display: flex;
    flex-direction: column;
  }

  .advanced__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }

  .advanced__body[hidden] {
    display: none;
  }

  /* AI drafting hero above the authored fields: the same flat disclosure
     treatment as Details — no frame, body separated by a dashed rule. */
  .ai {
    display: flex;
    flex-direction: column;
  }

  .ai__body {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    padding-top: var(--space-3);
    border-top: 1px dashed var(--border);
  }

  .ai__body[hidden] {
    display: none;
  }

  .ai__actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  /* Local-target search rows: the same flat list treatment as the AI
     outcome — no overlay, no absolute placement, radios stay native. */
  .find__list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .find__row {
    display: flex;
    align-items: baseline;
    gap: var(--space-2);
    cursor: pointer;
    min-width: 0;
  }

  .find__row input {
    margin: 0;
    accent-color: var(--accent);
    flex: none;
    align-self: center;
  }

  .find__name {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    overflow-wrap: anywhere;
  }

  .find__path {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
    min-width: 0;
  }

  .find__kind {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
    flex: none;
  }

  .find__roots {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-width: 0;
  }

  .find__root {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .find__root .find__path {
    flex: 1;
  }

  .ai__status {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .ai__outcome {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--bg-page);
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .ai__outcome:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: var(--ring-glow);
  }

  .ai__flag {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    font-weight: 500;
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .ai__shell {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    letter-spacing: var(--tracking-mono);
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .ai__command {
    margin: 0;
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
    color: var(--text);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .ai__meta {
    margin: 0;
    font-size: var(--text-xs);
    line-height: var(--leading-tight);
    color: var(--text-muted);
  }

  .ai__explanation {
    margin: 0;
    font-size: var(--text-sm);
    line-height: var(--leading-normal);
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
