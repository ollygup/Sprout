<script lang="ts">
  import type { LaunchCommandTest } from "$lib/types";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";

  let {
    open,
    command,
    probe,
    validate,
    onerror,
    testing = $bindable(false),
  }: {
    /** Reopening the owning dialog clears the last result. */
    open: boolean;
    command: string;
    /** The backend probe this dialog's Test runs (timeboxed server-side). */
    probe: () => Promise<LaunchCommandTest>;
    /** Extra pre-flight beyond the non-empty rule — e.g. the quick-action
     *  working-directory check. Returns a message to refuse, null to pass. */
    validate?: () => string | null;
    /** Every diagnostic for the block — guard refusals and probe failures
     *  alike; an empty string clears the owner's error line as a run starts. */
    onerror: (message: string) => void;
    testing?: boolean;
  } = $props();

  let test: LaunchCommandTest | null = $state(null);
  let tested = $state(false);

  $effect(() => {
    if (open) {
      test = null;
      tested = false;
      testing = false;
    }
  });

  async function runTest() {
    if (!command.trim()) {
      onerror("Type a command first.");
      return;
    }
    if (validate) {
      const problem = validate();
      if (problem) {
        onerror(problem);
        return;
      }
    }
    onerror("");
    tested = true;
    testing = true;
    test = null;
    try {
      test = await probe();
    } catch (e) {
      console.error(e);
      onerror(String(e));
    } finally {
      testing = false;
    }
  }
</script>

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

<style>
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

  .mono {
    font-family: var(--font-mono);
  }
</style>
