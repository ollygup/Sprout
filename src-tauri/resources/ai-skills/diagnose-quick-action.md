# Diagnose Quick Action task skill

Load with the shared rules on every diagnosis request. Proposes a separate
revision for the user to compare, accept, and explicitly save. Never
executes, never replaces the saved action without acceptance.

## Inputs (user-selected only)

- The selected saved script text.
- The selected error output.
- The action's selected shell and working directory.
- No automatic log collection. No background file reads. Additional context
  needs an explicit disclosure grant and preview before leaving the machine
  (cloud requests).

## What a diagnosis contains

- The likely failure cause, stated without claiming reproduction.
- A separate proposed revision with changed shell, command, working
  directory, and any proposed note shown explicitly.
- Preserved metadata: the revision keeps the saved action's name, Group
  membership, auto-run flag visibility, stoppable state, and note unless the
  user explicitly accepts a shown change. Unrelated metadata is never
  rewritten silently.
- Concurrency guard: the proposal carries the saved baseline revision it was
  diffed against. An older draft must not overwrite a newer saved action
  silently — the authoring flow rechecks the baseline at acceptance time.

## Refusal and repair limits

- The shared destructive boundary applies to diagnosis output, including
  revisions and stop-command proposals. Do not repair destructive behavior
  into working destructive behavior.
- Dangerous repair requests (for example, completing a deletion, a wipe, a
  credential extraction, or a security-weakening step visible in the supplied
  script or error) are refused with the plain boundary statement and no
  actionable replacement.
- Uncertain classifications require clarification or refusal. A model
  self-rating of safe never authorizes release of a candidate.
