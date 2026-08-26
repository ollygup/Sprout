# 0010 — Enter-key submit conventions: the norm splits by content class

**Date:** 2026-08-26 · **Question:** in form dialogs, is "Enter creates/submits,
Shift+Enter makes a newline" the norm — or is Ctrl+Enter the submit key inside
multi-line fields? Rule adopted here: use only conventions users already carry
from other products; never invent app-exclusive shortcuts.

## Findings

### 1. The native baseline (universal — no product deviates)

- Single-line inputs inside a `<form>`: plain **Enter submits** (HTML default).
- Textareas: plain **Enter inserts a newline** everywhere. No mainstream
  product rebinds it, because multi-line authoring would become impossible.

### 2. The submit key follows the field's content class, not the app's identity

- **Short-message composers → Enter sends, Shift+Enter newlines** [official]:
  Slack's own preference doc — "If you choose **Send the message**, you can
  use Shift+Enter to start a new line"
  (https://slack.com/help/articles/115005523006-Set-your-Enter-key-preference);
  Discord defaults the same way (support.discord.com threads).
- **Long-form / code / data entry → Ctrl/Cmd+Enter submits** [official]:
  - GitHub's keyboard-shortcuts doc: "Command/Ctrl+Enter — Submits a comment"
    (https://docs.github.com/en/get-started/accessibility/keyboard-shortcuts).
  - VS Code shipped Ctrl+Enter for finishing PR-review comments by direct
    user demand (https://github.com/microsoft/vscode-pull-request-github/issues/3572,
    landed via microsoft/vscode#151739).
  - Gmail sends with Ctrl+Enter; plain Enter stays a newline.
  - Slack itself, in its "Start a new line" mode — the posture a long-form
    field forces — switches sending to Ctrl/Cmd+Enter (same help article).
    Even a chat product converges on Ctrl+Enter once content becomes
    multi-line-first.
- User testimony corroborates the split: "Ctrl+Enter to send message is
  something many people are used to, especially if they create new lines with
  only enter"; "the shift-enter behaviour is used for word processors … an
  instant messaging app usually uses enter to send"
  (https://support.discord.com/hc/en-us/community/posts/360050508971-Pressing-enter-to-create-a-new-line-in-chat).

### 3. The classification test

Ask what the textarea *holds*: ephemeral short messages (chat class) or data /
code payloads worth keeping (GitHub-Gmail class)? The accidental-submit cost
decides — a chat client loses a message fragment; a data form fires a
half-written command or truncates a clip mid-authoring.

## Implications for Sprout

Clip text, Quick Action command, and launch-command fields are data payloads →
GitHub class: **Ctrl+Enter submits, Enter keeps making newlines**, with the
hint shown under each such field. Single-line inputs keep the native
plain-Enter submit they already have. Implemented once in the shared `Dialog`
component so every form inherits it identically.
