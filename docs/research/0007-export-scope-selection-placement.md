# Export scope selection: dialog, not permanent settings chrome

Standing rules for where option surfaces belong when a frequent default
action carries a rare customization. Written after ticket 87 first shipped
its five collection checkboxes permanently inline beside the Export button;
this note settles the correction with evidence instead of taste.

*Boundary with research 0008:* this note governs **per-use scope choices** —
options that apply once, to the artifact being produced right now, so they
live in a moment-of-use dialog and leave no chrome behind. A *durable
preference* that reshapes its own surface persistently (opt-in feature
switches) is the opposite case and belongs to 0008's page-features menu.
Classify the knob before choosing either home.

## Sources

- Jakob Nielsen, *Progressive Disclosure*, NN/g (2006) — https://www.nngroup.com/articles/progressive-disclosure/
- Microsoft, *Profiles in Visual Studio Code* (official docs; Export flow ticks profile contents inside the export editor/dialog, not in Settings chrome) — https://code.visualstudio.com/docs/configure/profiles
- microsoft/vscode, `userDataProfileImportExportService.ts` (checkbox state lives in the export dialog's tree, `Select …` accessibility labels) — https://github.com/microsoft/vscode/blob/main/src/vs/workbench/services/userDataProfile/browser/userDataProfileImportExportService.ts
- Notion, *Export your content* / *Back up your data* (help center; page export opens a modal whose toggles decide scope — `Include subpages`, `Include content`; workspace-wide export is one bare command in Settings) — https://www.notion.com/help/export-your-content , https://www.notion.com/help/back-up-your-data

## Findings

1. **Show few options initially; disclose specialized ones upon request.**
   Nielsen: "Initially, show users only a few of the most important options.
   Offer a larger set of specialized options upon request." The print dialog
   is his canonical example — copies and printer up front, scaling and
   reverse-order behind an *Advanced* button leading to secondary dialogs.
   Permanently rendering all five collection checkboxes next to Export puts
   a rarely-touched surface at the same visual level as the daily action,
   which is exactly the bloat he warns print dialogs had grown into.
2. **The frequency split decides placement (extends 0004 rule 2).**
   Whole-app backup is the frequent default; excluding a collection is the
   rare variant. NN/g's usability criterion: disclose everything frequently
   needed up front so users "progress to the secondary display only on rare
   occasions" — and the fact that something appears on the initial display
   tells users it is important. Five checkboxes that most users never touch
   fail that test on the Settings knob row.
3. **Disclosure must be obvious and honestly labeled (extends 0004 rule
   4's scent logic to actions).** The affordance that reveals the secondary
   level needs clear expectations: an `Export…` button that opens a dialog
   titled *Export backup* sets the expectation precisely; a wall of
   unlabeled checkboxes does the opposite.
4. **Precedent: scope pickers live inside the export flow.** VS Code's
   Export Profile opens an editor where each content type is ticked via
   checkboxes before exporting (docs + source above); Notion's page export
   is a modal with format dropdown and include-toggles, while its
   workspace-wide backup is a single bare command in Settings — the default
   path carries zero option chrome. Both put selection at the moment of
   use, not in persistent configuration.
5. **Two disclosure levels still hold (0004 rule 3).** Level 1: the knob's
   `Export…` button. Level 2: the selection dialog. A dialog adds no third
   level — it relocates level 2 out of always-visible chrome.

## Verdict

The export-scope checklist moves into a dialog opened by the knob's
`Export…` button (pattern B): the knob shows two buttons (`Export…`,
`Restore…`) and no permanent checkboxes; the dialog lists the five
collections, all ticked by default, with its confirm action labeled to say
exactly what happens next (`Export selected…`). Zero-selection disables
that confirm (and the backend refuses it — ADR-0014). This satisfies 0004
rule 2's frequency split, keeps disclosure at two levels, matches the
NN/g print-dialog shape and the VS Code/Notion precedents, and restores
the knob row to the one-glance simplicity the other knobs have.
