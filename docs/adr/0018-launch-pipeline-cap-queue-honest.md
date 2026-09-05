# Quick Launch runs through a capped queue with honest outcomes

> Status: amended 2026-09-05 — original decision text preserved; see the executable-source audit amendment for current behavior and implementation gaps.

Starting the whole Quick Launch list runs it through a capped queue, not a fan-out: at most `launch_concurrency` entries are in flight (default 8, range 1–50), the rest wait for a slot. Only an entry with a live desktop assignment holds its slot past spawn — it waits for its main window (`wait_for_new_window`, 15 s timeout, 250 ms poll) so the move-to-desktop can land; windowless and command entries free their slot at spawn. A run is single-flight per process (`AtomicBool`): a second Start while one is in flight is refused with an "already in progress" error, and every Start affordance (tray, window, page) converges on the same runner. A failed launch never aborts the rest; a no-window timeout counts as started and never stalls the queue. Already-running apps (full exe-path match) are skipped and reported, never duplicated; a missing target fails fast. Dead-desktop assignments and refused moves degrade to notes on an otherwise successful start. Single-tap starts behave differently from batch starts by design: a single tap foregrounds the on-target window instead of reporting "already open".

## Consequences

- Large sets can't slam the machine, and the queue drains even when apps show no window.
- The summary (system notification + window/page flash) tells the truth per entry — started, moved, skipped-already-open, failed — instead of one blanket verdict.
- Concurrency is a Setting, not a per-run knob, so the morning routine behaves the same from every trigger.

## Amendment — 2026-09-05 (executable-source audit)

The Start triggers are the Quick Launch window and page; the tray opens the window (`src-tauri/src/tray.rs`). Win32 existing-window matching uses case-insensitive executable basenames, not full paths (`src-tauri/src/engine/windows.rs`, `process_matches_image`). The stronger identity guarantee in the original paragraph is not currently enforced.

Single-tap execution requests foregrounding, but `run_launch_queue_inner` in `src-tauri/src/launch.rs` discards the result of `foreground_window` and reports “foregrounded” even when Windows refuses the request. Native Store-app existing-window detection is also incomplete: `window_aumid` in the Windows adapter returns `None`, so `app_windows` cannot identify an existing Store window by AUMID. Honest foreground outcomes and the general already-open guarantee remain obligations, not reasons to endorse those gaps.

The capped queue, single-flight guard, live-assignment slot lifetime, no-window timeout, failure continuation, and desktop-move notes remain implemented. Internal refactoring must preserve those behaviors while treating the identified gaps as separate behavioral work.
