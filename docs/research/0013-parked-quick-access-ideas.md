# 0013 — Parked quick-access ideas

Concise log of ideas raised but **not** ticketed, with the restriction that parked them. Read before re-proposing. Each entry: IDEA + why not implemented.

## 1. Elevated Quick Action (per-action "Run elevated" to bypass yes/no popups)

- IDEA: checkbox on Quick Action creation to elevate PowerShell before running the command,goal being to silence in-flow yes/no prompts.
- WHY PARKED: no silent UAC bypass exists from a non-elevated process — the app's only elevated path (`ShellExecuteW(..., "runas", ...)` install worker) always raises one UAC prompt at Run time, and `Start-Process -Verb RunAs` is the same verb with the same prompt. In-script Confirm prompts (`ShouldProcess`, installer Y/N) need authoring (`-Force`, `-Confirm:$false`), not elevation — elevation would *add* a prompt, not remove one. Would also break the `Quick Action` glossary ("no elevation") and the silent auto-run contract. Reopen only on a genuine shield-prompt case.

## 2. Restart re-attach of running actions

- IDEA: after close/restart, re-own still-live Quick Action processes so Run/Stop state survives.
- WHY PARKED: tracking is a per-session pid registry (`RunningQuickAction`); after restart survivors are orphans, pid reuse makes blind re-attach unsafe, and detached commands (`-d`) report not-running by design. Honest fallback (persist `was_running` → "Interrupted by restart" + Re-run, never fake Stop) is designed but skipped per reporter — keep isolated, never gate other tickets on it.

## 3. Companion in-dock volume slider (0–100 loud/soft)

- IDEA: volume slider beside Companion mute for loud/soft control inside the dock.
- WHY PARKED: WebView2 exposes only `IsMuted bool` + `IsDocumentPlayingAudio`/`IsMutedChanged` (mute-only, no level API). A slider would have to drive the OS session volume via Core Audio from Rust — duplicating the Windows Volume Mixer entry that already controls Companion loud/soft today (it appears as a separate session, often without Sprout's icon). Ticket 127 ships mute + playing indicator; loud/soft stays site player + mixer + Open-externally.
