# 85 — UX round: frequency-first navigation, selective export, opt-in Groups & advanced gating, individual runs, honest Run/Stop states (spec)

**What to build:** One UX round that splits Sprout's audience honestly: the navigation rail leads with the three daily surfaces (Quick Launch, Quick Actions, Quick Clips); Settings' export grows a per-collection checklist so users share part of the app instead of all of it; the two power features — virtual-desktop grouping and custom Groups — become explicit opt-in toggles so a new user never meets an accordion they didn't ask for; every Launch entry and Quick Action becomes individually runnable from the main app and from the Quick Launch window/dock; and the window's action controls gain an unambiguous Run (accent) / Stop (danger) / Stopping (disabled spinner) language with hover details showing name plus content. Implemented via tickets 86–94, audited by 95.

**Blocked by:** none (feature-area spec; implemented via tickets 86–94, audited by 95)

**Status:** ready-for-agent

## Problem Statement

The app today sits awkwardly between what a new user and an advanced user want. The rail opens on Products/Presets/Plan — the rarely used setup cluster — while Quick Launch, Quick Actions, and Quick Clips, the daily surfaces, sit below the fold. Sharing is all-or-nothing: the only export is the whole-app backup, so a user who wants to hand someone just their clips or just their launch list cannot. Power affordances appear uninvited: a fresh Quick Launch categorizes entries under virtual-desktop accordions a new user never asked for, making the menus feel confusing. And the Quick Launch window's action controls read as ambiguity: Run and Stop render in the same neutral color, clicking Stop during teardown still shows Stop (did my click land?), and neither actions nor clips reveal what they contain on hover — you run or copy blind unless you memorized the list.

## Solution

The rail reorders by frequency of use: quick trio first, setup trio second, reference (History, Logs, Settings) last, with thin dividers between clusters. Settings > Export gains five collection checkboxes — all checked by default so one click still backs up everything — writing the same `sprout-backup` document with unchecked collections empty, so importers need no changes and any partial file restores exactly like today. Two advanced capabilities become explicit per-feature toggles living in each list page's toolbar row (Notion's split: visibility control on the surface, configuration elsewhere): **Groups** on each of the three list pages and **Desktop grouping** on Quick Launch, all off by default. Desktop assignment stops structuring any list — when enabled it lives only in each entry's edit dialog / row menu with a small row badge, and when disabled it is fully dormant: hidden everywhere and ignored at launch, stored assignments resuming untouched if re-enabled. Groups are user-named buckets scoped per collection — Quick Action groups hold only Quick Actions, Clip groups only Clips, Launch-entry groups only Launch entries — rendered ungrouped-first, then as default-expanded Disclosure accordions with count badges on the page and in the window/dock. Every Launch entry gains its own run trigger on the main-app page and becomes individually clickable in the window/dock, with Start all pinned on top. Quick Actions mirror the window's run states on their main-app page. The window's controls adopt an honest three-state visual language — Run accent-filled, Stop danger-filled, Stopping disabled with a spinner — backed by a ten-second stop-command watchdog that falls back to killing the process tree, and action/clip rows show name plus truncated content on hover through the shared tooltip.

## User Stories

**Navigation**

1. As a daily user, I want Quick Launch, Quick Actions, and Quick Clips at the top of the rail, so that my most-used surfaces are one glance away.
2. As a user, I want thin dividers separating the quick, setup, and reference clusters, so that the rail reads as three groups rather than nine undifferentiated items.
3. As a new user, I want Settings to stay last in the rail, so that configuration never competes with daily surfaces.

**Selective export**

4. As a user, I want to tick which collections to export before exporting, so that I can share just my clips, just my launch list, or any combination instead of the entire app.
5. As a user who wants the old behavior, I want every collection checked by default, so that a plain click on Export still produces the full backup I know.
6. As a recipient of a partial export file, I want it to restore through the existing import flow with accurate inserted/skipped counts, so that nothing about importing changes for me.
7. As a user, I want the export file to stay one document format, so that files I exchanged before keep working after this change.
8. As a privacy-conscious user, I want machine-local install directories to stay out of selective exports exactly as they are out of whole-app backups, so that no new channel leaks machine-local data (ADR-0009).

**Advanced gating**

9. As a new user, I want grouping and desktop features invisible until I explicitly enable them, so that menus show only what I understand.
10. As a power user, I want a Groups toggle right on each list page's toolbar row, so that enabling the feature happens where I use it.
11. As a user, I want my choice of these toggles remembered across restarts, so that the app doesn't reset my sophistication level.
12. As a user, I want toggling a feature off to hide it without deleting anything, so that experimenting is reversible.
13. As a user, I want the Quick Launch window and dock to obey the same toggles, so that hidden features never resurface on another surface.
14. As a user, I want these toggles absent from the Quick Launch window itself, so that the fast-access palette stays configuration-free (research 0004 rule 3).

**Desktop assignment (demoted)**

15. As a new user, I want no virtual-desktop accordions anywhere by default, so that the launch menu is a plain list I immediately understand.
16. As a power user with the desktop feature enabled, I want to assign a desktop from the entry's edit dialog or row menu, so that assignment is deliberate configuration rather than a structural surprise.
17. As a power user, I want assigned entries to carry a small desktop badge, so that I can see assignments without the list being rearranged around them.
18. As a user who disables desktop grouping after assigning entries, I want assignments ignored at launch while hidden, so that the app never performs invisible actions on my behalf.
19. As that same user, I want my assignments preserved and effective again when I re-enable the feature, so that turning a toggle is never destructive.

**Groups**

20. As a power user with many Quick Actions, I want to create, rename, reorder, and delete groups on the Quick Actions page, so that my actions scale past a flat list.
21. As a power user, I want the same grouping ability for Quick Clips, so that clip collections stay navigable.
22. As a power user, I want the same grouping ability for Launch entries, so that large launch lists stay organized independently of desktop assignment.
23. As a user, I want a Quick Action group to accept only Quick Actions, so that groups never mix unrelated collections.
24. As a user, I want each item in at most one group, so that every item has exactly one place in the list.
25. As a user, I want ungrouped items listed before groups, so that my fastest access stays on top.
26. As a user deleting a group, I want its members returned to the ungrouped list, so that removing structure never removes items.
27. As a user, I want group sections to appear only once at least one group exists, so that empty structure never occupies the page (research 0004 rule 2).
28. As a user, I want group accordions default-expanded with a count badge, so that grouping organizes without hiding.
29. As a user, I want groups rendered through the same disclosure pattern everywhere they appear, so that the interaction is learned once.

**Individual runs**

30. As a user, I want a per-entry run affordance on the main-app Quick Launch page, so that I can start one thing without opening the mini window.
31. As a user, I want each entry row in the Quick Launch window and dock individually clickable, so that one-click access works at the dock too, not just all-at-once.
32. As a user, I want Start all to remain pinned at the top, so that the original one-click routine survives the richer layout.
33. As a user, I want the list below Start all to be flat when grouping is off and ungrouped-first-plus-accordions when on, so that the dock matches my chosen sophistication.
34. As a user, I want a Run control on each Quick Action row in the main app, so that running an action doesn't require the window either.

**Run/Stop/Stopping honesty**

35. As a user, I want Run rendered in the accent color, so that starting is visibly the primary verb on the surface.
36. As a user, I want Stop rendered in the danger color, so that stopping is never mistaken for starting.
37. As a user who clicked Stop, I want the button to become a disabled "Stopping…" spinner until the process actually exits, so that I know my click landed and the app is working.
38. As a user whose stop command hangs, I want Sprout to force-kill the process tree after ten seconds, so that a stuck stop can never wedge the control.
39. As a user, I want the same three states on the main-app Quick Actions page, so that the vocabulary is identical wherever actions live.

**Hover details**

40. As a user, I want hovering a Quick Action row to show its name and command, so that I can confirm what I'm about to run.
41. As a user, I want hovering a Clip row to show its name and content, so that I can confirm what I'm about to copy.
42. As a user, I want that hover content truncated consistently, so that even long commands stay readable tooltips rather than walls of text.

**Docs for future sessions**

43. As a future contributor, I want Notion's factual design patterns captured as a research note, so that UI decisions cite evidence instead of re-deriving it.
44. As a future session, I want AGENTS.md to point at the standing research notes and design skills, so that every UI change starts from the same rules.

## Implementation Decisions

- **Navigation**: the rail's item array reorders into three clusters separated by hairline dividers; no labels on dividers — the rail is narrow and the cluster boundaries must not become headings.
- **Selective export**: the export command gains a selection parameter; unselected collections serialize as empty arrays inside the unchanged `sprout-backup` version-1 document. No new document kinds, extensions, or importer paths. An ADR records why one document format won: exported files circulate outside the repo and a format split is effectively irreversible once users hold files.
- **Advanced gating**: no master "advanced mode" switch — per-feature toggles achieve the identical new-user experience with fewer states (a master switch is a mode, which multiplies state combinations). Four persisted setting keys, all default off: Groups × 3 collections and Desktop grouping. Toggles render in each page's toolbar row beside search through shared components; the window/dock reads the same values and offers no configuration surface.
- **Desktop assignment demotion**: the desktop-structured accordion view of the launch page is removed entirely — desktop stops being a structural axis anywhere. When enabled: a Desktop selector inside the existing edit dialog / row menu, a badge on assigned rows, launch-time honoring. When disabled: no menu presence, no badge, and the launch runner ignores assignments — dormant, never deleted.
- **Groups domain model**: a groups table whose rows carry a collection discriminator (`launch` / `action` / `clip`), enforcing namespace isolation at the data layer; nullable group-id columns on the three item tables (max one group per item). Commands: create, rename, delete, move (reorder), and per-collection assign/unassign. Deleting a group nulls members' group ids. List order: ungrouped first, then groups in user order.
- **Group rendering**: the existing Disclosure primitive renders group sections, default-expanded, with count badges, on main-app pages and in the Quick Launch window/dock alike; sections appear only once ≥1 group exists. In the window/dock, Start all stays pinned above the list region; the list below is flat when Groups is off for that collection, ungrouped-first-plus-accordions when on.
- **Individual runs**: a backend command starts one Launch entry through the existing launcher-engine seam; window/dock entry rows become buttons carrying accessible names; the main-app Quick Actions page subscribes to the same global run-state events the window already uses, so both surfaces flip states from one source of truth with no polling.
- **Stop lifecycle**: a third run phase (stopping) joins the existing binary running event stream. Clicking Stop enters Stopping: control disabled with spinner; the configured stop command gets a ten-second watchdog, then the process tree is force-killed; the exit event returns the control to Run. If the process exits first, Stopping ends immediately regardless of the watchdog.
- **Visual language**: Run = accent-filled, Stop = danger-filled (existing token families cover both themes), Stopping = disabled muted treatment. Exactly one primary verb per surface is preserved (research 0005 rule 2): on action rows the run control is that row's primary.
- **Hover details**: the shared tooltip component shows bold name plus truncated monospace content (command text for actions, clip text for clips) on window/dock rows.
- **Glossary**: a new **Group** term enters the domain glossary — a user-named bucket within exactly one collection, distinct from virtual-desktop assignment — landing alongside the groups-foundation work; the Launch entry wording is touched up to separate the two concepts.

## Testing Decisions

- Tests assert external behavior only — command/module outcomes against a temporary database, never internal call graphs. Prior art: the whole-app backup tests (export → inspect → import round-trips with counts), the Quick Action run-tracking tests, and the launch-runner tests.
- Covered behaviors: selective export writes only selected collections yet imports cleanly (including a partial file restoring into a populated database with correct inserted/skipped); group CRUD respects collection isolation (assigning across namespaces fails); assign/unassign/move/delete-cascade leave the documented ordering; desktop dormancy — the launch runner skips assignments while the feature is off and honors them when on; the stop watchdog kills the tree after the timeout when the stop command hangs, and does nothing when the process exits first; single-entry launch starts exactly the requested entry.
- Frontend verification stays at the established seams: type-check cleanliness plus manual dev passes over both the main app and the Quick Launch window/dock (there is no component-test runner in this repo); the closing audit ticket applies the web-interface-guidelines review to every changed component.

## Out of Scope

- A master "advanced mode" switch (rejected: per-feature toggles reach the same default-simple experience with fewer states).
- Cross-collection groups, nested group-inside-desktop structures, or more than one group per item.
- Changes to Preset/Product sharing — `.sprout.json` remains their format; selective export concerns the five backup collections only.
- Persisting accordion collapsed state across sessions (session-local for now).
- Drag-and-drop reassignment of items between groups directly on the dock/window surfaces (grouping is managed on main-app pages).
- Any redesign of Plan/History/Logs beyond their rail position.

## Further Notes

- Evidence base: research 0004 (progressive disclosure — show navigation if you can; absent-until-content; ≤2 levels; mandatory feedback) and 0005 (page-chrome consistency — one primary per header row) governed every decision here; Notion's factual patterns (visibility-on-surface vs configuration-elsewhere, minimal-until-content defaults, explicit-setup gating) are captured as research 0006 during this round's session rather than via a tracked ticket.
- The stop watchdog's ten-second figure is a product decision, not a constant smuggled into a second home — it lives with the other window/action geometry constants per the constants-split convention.
