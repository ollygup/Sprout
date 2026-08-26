# 0009 — Unsaved changes on navigation: sticky bar + intercept, not scroll-to-save

**Date:** 2026-08-26 · **Question:** when the user edits Settings and then tries to
leave the page (rail navigation or window close), what is the evidence-based
pattern? Is "auto-scroll down to the Save button and highlight it with a
warning" a known/best practice?

## Findings

### 1. NN/g: confirmation is legitimate exactly when work would be destroyed [official]

- *Confirmation Dialogs Can Prevent User Errors (If Not Overused)* —
  https://www.nngroup.com/articles/confirmation-dialog/ — confirmations are
  justified before "actions with serious consequences — such as destroying
  users' work"; they slow users down otherwise, and overuse backfires ("if you
  cry wolf too many times, people will stop paying attention"). Buttons must
  name consequences ("Delete file / Keep file"), never Yes/No.
- *Cancel vs Close* — https://www.nngroup.com/articles/cancel-vs-close/ —
  "always ask for confirmation before committing destructive actions" that
  lose the user's work; automatic saving of drafts is presented as the
  superior alternative where feasible.
- *Error-Message Guidelines* — https://www.nngroup.com/articles/error-message-guidelines/
  — banners/inline labels suit low-severity issues; modals are reserved for
  severe ones; messages display close to their source.
- *Preventing User Errors* — https://www.nngroup.com/articles/user-mistakes/ —
  warn at the moment the error is being made, not only after.

### 2. Discord ships the two-part pattern [datamined strings + corroboration]

- While editing, a persistent bar pins the choice on-screen:
  `GUILD_SETTINGS_OVERVIEW_NOTICE: "Careful! You have unsaved changes!"`
  (https://github.com/Discord-Datamining/Discord-Datamining/wiki/2017-03-27-1.-92368c00e3534550e506;
  corroborated by https://oh-my-design.kr/design-systems/discord).
- Exit attempts get a specific decision dialog; Discord's patch notes describe
  guarding "Edit Note" discard flows and fixing an exit path (Add Widgets
  during profile editing) that silently discarded changes — i.e., every exit
  path is guarded (https://discord.com/blog/discord-patch-notes-march-6-2026,
  https://discord.com/blog/discord-patch-notes-july-7-2026).

### 3. Scroll-to-the-Save-button on navigation attempt: recommended nowhere; counter-documented [issue trackers]

- Backdrop CMS issue #1656 filed scroll-to-show-a-warning as a BUG: "Expected
  behavior: The page should not scroll away from what I care about to show me
  a message" — the fix direction was fixed/sticky messaging
  (https://github.com/backdrop/backdrop-issues/issues/1656).
- Ecosystem consensus makes Save permanently reachable instead of moving the
  viewport: Mirakl's save-bar spec ("always located at the bottom of the
  screen and stays fixed as you scroll… visible when there are unsaved
  changes", max 2 buttons — https://design.mirakl.com/design/components/actions/save-bar);
  filament-sticky-save-bar (appears when the native Save button scrolls out of
  view — https://github.com/cocosmos/filament-sticky-save-bar); Jira Plans'
  prominent "Unsaved changes" button with change count
  (https://community.atlassian.com/forums/Advanced-Planning-in-Jira/Help-us-design-the-next-iteration-of-the-quot-Unsaved-changes/td-p/3027638).
- GitLab intercepts route exits with specific confirm dialogs
  ("Continue editing" / "Discard changes") rather than letting navigation pass
  (pipeline editor MR !52458, wizard MR !239527, work-item issue #468469).

### 4. When explicit save is right at all [design-system formalization]

Notion applies changes continuously (no save buttons). Grafana's design
system draws the line: autosave for low-risk settings only; medium/high
impact settings keep an explicit save plus "an unsaved warning … when the
user wants to move to another page without saving"
(https://github.com/grafana/design-system/blob/main/docs/06-patterns/save.mdx).
Sprout's Settings knobs (dock mode/edge/state, autostart, install directory)
have machine-level effects — explicit-save class.

### 5. Accessibility of the intercept [guides]

Use `alertdialog` semantics; initial focus lands on the safe choice; Escape
follows the safe path (https://uxpatternsguide.com/patterns/unsaved-changes-prompt/,
https://uxpatternsguide.com/patterns/exit-warning/). Dirty/clean transitions
announce politely (`aria-live="polite"`), never steal focus, never rely on
color alone (https://github.com/vmitsaras/A11y-Dirty-Form-Guard). Focus
returns to the triggering control after the dialog closes
(https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/).

## Implications for Sprout

Adopt the Discord model on Settings — never scroll the user anywhere to
deliver the warning:

1. Dirty = current field values differ from the loaded snapshot (compared
   post-clamp, so clamped numerics don't fake dirtiness).
2. A sticky bar appears at the bottom of the Settings page the moment any
   knob differs: warning text + Save / Discard (Mirakl placement rule: fixed,
   bottom, ≤2 buttons).
3. Rail navigation away, or closing the main window, while dirty → three-way
   dialog: **Save changes / Discard changes / Keep editing**; initial focus
   Keep editing; Escape = Keep editing; focus restored afterwards.
