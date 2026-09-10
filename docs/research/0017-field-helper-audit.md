# 0017 - Field and helper-text audit

Date: 2026-09-08. Status: proposed recommendations, not accepted design or implementation.

## Boundary

Source inventory of Sprout-authored Svelte input, textarea, select, TextInput, SearchInput, and Select surfaces, including repeated and conditional fields. CodeGraph was consulted first; source search filled its markup gaps. This is not live visual/accessibility testing or observed user testing. Third-party sites rendered inside Companion are outside this audit. Line numbers describe the audited source and can move. The research-backed policy discussion is separate; these are proposed applications of it.

Do not equate all below-field text with clutter. Validation, meaningful state, constraints, and non-obvious consequences differ from repeated tutorials. Preserve accessible names and essential information when shortening text.

## Inventory and proposed disposition

Paths below are relative to `src/`.

| Surface / source | All fields on surface | Proposed disposition |
| --- | --- | --- |
| `lib/components/QuickActionFormDialog.svelte:198-381` | Name; Command; Working directory; Group; New group name; Notes; Show Stop button; Stop command; Run at Sprout start; Show in dock | Remove keyboard hint lines at 230, 296, 335. Remove Name's generic location InfoTip and generic Notes InfoTip. Keep working-directory blank default, foreground-process tracking limit, empty stop-command consequence, startup semantics. Move notes-format syntax into optional help. Group options already explain ordinary selection. Retain concise dock consequence help. New group name needs no additional hint. |
| `lib/components/CommandFormDialog.svelte:111-191` | Shell; Command; Show a window; Show in dock; Name | Remove keyboard hint 150 and name's "edit freely" tutorial. Consolidate shell/command tutorials into one optional explanation; preserve Direct exe non-expansion and quoting constraints. Shorten window/dock explanations. |
| `lib/components/ClipFormDialog.svelte:87-133` | Text; Name; Show in dock | Remove keyboard hint 104 and generic explanation of clip purpose. Naming placeholder and InfoTip duplicate each other: use Name (optional) and one concise explanation of first-line fallback if needed. Retain dock consequence succinctly. |
| `lib/components/ProductFormDialog.svelte:270-443` | Name; winget registry search or manual ID; Advanced install location hint; install directory; repeated environment action/name/value | Remove search tutorial explaining that picking fills an ID. Preserve searching, failure and no-results states. Manual ID's empty meaning is useful. Preserve distinction between location hint and requested directory, override/default behavior, and installer limitations. Name already lean. Environment fields need stable identification; action Select lacks an explicit accessible label. |
| `lib/components/PresetFormDialog.svelte:115-377` | Preset name; Description; Author; Version; application Product; Version policy; conditional pinned version; Timeout; dependency checkboxes; repeated environment action/name/value; repeated verify command/match text | Metadata already lean. Keep advanced dependency/environment/verification semantics; shorten generic "What an application is" explanation. Policy options latest/pinned/present rely on title explanations: consider understandable option labels. Repeated environment/verify fields rely on placeholders for visible names; preserve or improve identification. Timeout already identifies minutes. |
| `lib/components/GroupNameDialog.svelte:48` | New/renamed group name across collections | Already lean. Retain label, example, and validation. |
| `routes/companion/+page.svelte:297-340` | Site name; Site URL; Site identity | Name (optional) can replace tutorial; make address fallback clear if needed. HTTPS is a real constraint: retain concise guidance or clear validation and normalization. Mobile/Desktop explanation is useful but duplicates option wording; consolidate. Page prose at 252, "first site is your quickest pick," is a removal candidate. |
| `routes/settings/+page.svelte:1174` | Settings search | Already lean; preserve accessible name, filtering cue, and no-result state. |
| `routes/settings/+page.svelte:1237-1385` | Theme; Install directory; Start with Windows; Default timeout; Log retention; Launch concurrency | Shorten paragraphs to non-obvious scope and consequences. Keep immediate application distinction where needed; installer default/limitations; tray-only startup; timeout termination; raw-log deletion versus retained run history; concurrency meaning. Units/current values and validation can replace repeated numerical prose. |
| `routes/settings/+page.svelte:1400-1629` | Quick Launch floating/docked state; Dock mode; Default dock edge; Dock width; List density; repeated per-monitor edge/mode/width; Reveal delay; Reveal sensitivity | Remove implementation narration and label restatement. Keep fixed-space versus overlay effects, default/per-display scope, and reveal tuning meaning. Correct stale dock-mode description. Per-monitor edge/mode selects need explicit accessible names. Preserve disabled-edge reason and errors. |
| `routes/settings/+page.svelte:1649-1708` | Active companion site; Pane height | Keep concise Off consequence and dock-only scope. Retain divider discoverability if needed; starting-height explanation can be short. Saved-site count is meaningful state, not field clutter. |
| `routes/settings/+page.svelte:1893` | Backup collection checkboxes | Keep labels. Put export/restore consequences at dialog/section level once. Backup and update descriptions elsewhere on Settings should be shortened while preserving scope and restart consequences. |
| `routes/+page.svelte:719`, `routes/clips/+page.svelte:392`, `routes/quick-actions/+page.svelte:470`, `routes/products/+page.svelte:171` | Collection search fields | No persistent hints to remove. Preserve accessible names, filtering placeholders, and meaningful empty states. |
| `routes/+page.svelte:744` | Installed-app search | Keep scanning/error/no-match state. Shorten "pick one to add" if result affordance adequately communicates the action. |
| `routes/plan/+page.svelte:784,835,852,933` | Included-in-run checkbox; conflict policy/exclude radios; preset-selection checkboxes | Keep consequence-bearing options and conflict explanation. They explain decisions rather than repeat field labels. |

Shared `TextInput.svelte`, `SearchInput.svelte`, and `Select.svelte` are rendering owners, not additional product fields. Native buttons acting as choices (for example theme radio choices) were included where identified; a full interactive-control accessibility audit remains separate.

## Keyboard-hint ownership

Exactly five source instances say Ctrl+Enter submits and Enter adds a line: Quick Action Command, Notes, Stop command; launch Command; Clip Text. Product, preset and group-name fields are single-line and have no such hint.

- Ticket `158-dialog-multiline-hint-lines.md` explicitly requires a line under every dialog textarea. Its ACs are all checked although Status remains ready-for-agent.
- Ticket 157 owns the shared submission behavior; removing hint copy should preserve that grammar.
- Ticket 114 originally combined behavior and visible-hint requirements.
- Spec 109 explicitly said beneath each textarea.
- Research 0010 establishes the Ctrl+Enter convention; it does not independently establish that repeated permanent placement is optimal.

Before changing the presentation, reconcile these records so the accepted decision no longer instructs future agents to restore the removed lines.

## Concrete issues beyond redundant copy

- Settings dock-mode helper around line 1425 says auto-hide slides to a sliver when not hovered. Current CONTEXT describes fully off-screen hiding with no handle, revealed by deliberate edge push and hold. Correct the stale behavior description.
- Product environment-action Select around line 421 lacks an explicit accessible label, unlike its preset equivalent.
- Settings per-monitor edge/mode Selects around lines 1537 and 1547 lack explicit field labels. Preserve distinct accessible names when simplifying the row.
- Do not remove input labels, error messages, disabled-choice explanations, or state feedback as a side effect of helper cleanup.

## Validation still needed

Render the affected dialogs and Settings at supported sizes, inspect keyboard and assistive-technology naming, and confirm that retained constraints are available before submission. Evaluate keyboard-shortcut discoverability separately from whether the shortcut itself should exist. No live UX validation was performed for this note.

## Coverage refresh — 2026-09-09

The shared working tree was rechecked after additional AI-authoring changes. The original table is a dated baseline; add the fields below to ticket 167's required coverage. Source evidence is not a live usability test.

| Surface | Additional fields / controls | Accepted cleanup application |
| --- | --- | --- |
| QuickActionFormDialog | Shell; What should it do; Extra context (optional); AI disclosure and draft-review controls | Keep selected-shell semantics and explicit draft review. Remove the newly added permanent Ctrl+Enter-to-generate teaching line. Request and context fields own Ctrl/Cmd+Enter-to-generate and prevent outer dialog submission; preserve that behavior and the visible Generate action. |
| Settings AI assistance | Provider; conditional Local service address; conditional Local model; Test connection feedback | Preserve loopback-only endpoint constraints, exact model identity, test-does-not-save semantics, connection/error feedback and honest unavailable managed/cloud choices. Shorten duplicate mechanics, not necessary setup or consent information. |

There are now five permanent submit shortcut hints plus one generate shortcut hint. The original five-instance statement remains accurate only for submit hints; it is no longer the complete keyboard-help inventory. Ticket 167 covers all six, preserving different keyboard owners. The user accepted selective cleanup across every Sprout field; recheck the current tree at implementation for further additions.

The policy is accepted in spec 166; concrete copy and rendered accessibility still require implementation verification. ADR-0030/0031/0032's authoring-only, disclosure, provider and distribution boundaries remain unchanged. Fields present in source must be audited even when an earlier ADR/spec header still calls implementation pending.
