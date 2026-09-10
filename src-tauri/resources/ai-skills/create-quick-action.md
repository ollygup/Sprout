# Create Quick Action task skill

Load with the shared rules on every new-draft request. Proposes one Script
draft for the user to review and explicitly save. Never executes.

## Authorized targets

- The user's task description plus explicitly selected shell (PowerShell or
  CMD).
- Installed-app metadata only after an explicit find request, resolved
  through Sprout's bounded discovery (approved roots, name/path search,
  bounded results, depth, time, and cancellation; reparse-point containment
  enforced locally).
- Approved-folder name/path search under the same bounds. No file-content
  reads unless the user explicitly permits them for this request.
- Previously granted disclosure scope for cloud requests only. Never inherit
  consent across destinations.

## Shell-specific quoting and prerequisites

- PowerShell 5.1: quote paths with spaces using double quotes; escape
  embedded double quotes with the backtick; join statements with `;`. Do not
  use PowerShell 7-only operators, parameters, or modules. Do not assume
  `pwsh`, `ForEach-Object -Parallel`, ternary operators, or PSGallery modules
  exist.
- CMD: run shape is `cmd /c {command}`; quote paths with spaces using double
  quotes; use `&&` and `||` for chaining; `%VAR%` expansion only where the
  user explicitly wrote it. Do not assume Unix utilities, PowerShell cmdlets,
  or optional external programs exist.
- Multiline is allowed in both shells; keep lines readable and comment-free
  unless the user asked for comments.
- When a prerequisite is unknown (a module, an executable, a path), disclose
  it in Assumptions or ask for clarification — never invent a path.

## What a draft contains

- `shell`: the selected shell, echoed back explicitly.
- `command`: the proposed script text only (no destructive content — the
  shared refusal boundary applies before and after inference).
- `assumptions`: prerequisites the draft depends on.
- `affected_targets`: installed apps or approved paths the draft touches, as
  opaque references where possible.
- `explanation`: what the command does, in plain language, without claiming
  execution.

## Refusal and repair limits

- The shared destructive boundary applies to creation, including stop-command
  text bundled with a draft proposal.
- Do not repair a manually authored destructive command into a working
  destructive command. Offer a risk explanation and a non-destructive
  alternative where one exists.
- Do not provide actionable workaround steps or a prefilled manual draft
  after refusal.
