# Offline static Discord Rich Presence via handwired IPC (no accounts)

> Status: accepted 2026-09-12 — decision; implementation pending in ticket 182 under spec 181.

Sprout shows a static offline Rich Presence activity while it runs by handwiring the `discord-rich-presence` crate directly over Discord's local IPC. No Tauri wrapper plugin, no OAuth, no account access, no user-information read. This keeps the fully-offline posture (AI cloud mode excepted) while giving Discord users a lightweight "Playing …" status.

## Decisions

- Handwire `discord-rich-presence` (`DiscordIpcClient::new(APPLICATION_ID)` → `connect()` → `set_activity(Activity::new().details().state())` → `close()`). The crate speaks only to the local IPC pipe (`discord-ipc-*`); it performs no network, OAuth, or user-identity exchange.
- Static v1 payload only: `details: "Using Sprout"`, `state: "Composing presets"`. No user content in presence — no preset/action names, paths, counts, or run states. No art assets, Join buttons, or dynamic per-screen text in v1.
- Always attempt on app start (including tray-only boot) on a background thread; never block startup or window open. Discord absent/closed = silent no-op with local debug log and reconnect-with-backoff. No toast, dialog, or error surface. Clear/close on actual app exit; main-window close-to-tray is not exit.
- One hardcoded Application ID constant (placeholder until the user supplies the real numeric ID in ticket 182). Not Settings-editable, not backed up, not exported. No client secret, redirect URI, or token anywhere.
- Single ownership per ADR-0029: the new presence module is the sole Discord IPC site. Crate dependencies are minimal (`serde`/`uuid` class, already in-tree); the NFR-43 size budget is unaffected beyond a negligible delta recorded at delivery.

## Considered options

- A Tauri wrapper plugin was rejected: an extra abstraction over a three-call IPC surface with no lifecycle benefit, against the handwire-per-docs instruction and the single-owner rule.
- A Settings toggle (default off, per research 0008 app-global placement) was considered and deferred: v1 has no switch by explicit user scope ("not relevant") — closing Discord is the opt-out. A future toggle, if requested, belongs in Settings per 0008 rule 1 and needs its own amendment.
- Dynamic presence (current screen, counts, action names) was rejected for v1: any user-derived string risks leaking machine-local names into a public profile and breaks the no-user-info promise without a separate disclosure review.

## Consequences

- Requires the Discord desktop client with activity sharing enabled; web/mobile alone shows nothing. Users without Discord see zero behavior change.
- The Application ID is a release-blocking input: placeholder code cannot show presence until the real ID lands. No presence text may carry user data without a new amendment.
- AI assistance boundaries (ADR-0030/0031/0032) are untouched; presence sends no prompts, discovery results, or diagnostics.

## Amendment — 2026-09-12

Art assets ship after all, at the user's explicit direction: the portal app
icon (`src-tauri/icons/app_icon_1024.png`, byte-identical to the 1024
`app-icon.png` render) plus Rich Presence large/small art (`rp_large.png`
fully opaque brand tile, `rp_small.png` mark-only on transparency with a
doubled stem for ~24 px badge legibility). All three render from the
`app-icon.svg` source of truth through the owned `tools/render-icon.mjs`
pipeline, so no new brand geometry exists to drift. The v1 payload promise
is unchanged — static text only, still no OAuth, tokens, user-ID reads,
Join buttons, or dynamic per-screen text; images carry the fixed mark, no
user content.
