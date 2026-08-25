# Virtual desktop assignments activate by use

Ticket 88 gave desktop assignment a master switch ("Desktop grouping", off by default,
full dormancy when off) because the feature had just been demoted out of the list
structure and the switch was its only remaining surface. In practice that switch
violated its own contract: NN/g's toggle-switch guidance requires a switch's effect to
be immediate and visible, yet flipping it on changed nothing until the user also went
and assigned entries one by one. We therefore removed the switch (superseding ticket
88's dormancy for desktop assignment only — Groups keeps theirs): wherever virtual
desktops are supported (Windows 11 24H2+), every entry menu carries the Virtual desktop
submenu, the feature activates with the first assignment, and opting out means
unassigning entries individually via the submenu's explicit "No assignment" item. The
model follows Notion Favorites — the section appears once you favorite your first page;
there is no master toggle and removal is per item.

Considered alternatives:

- **Keep the switch** (ticket 88 behavior): rejected — a control whose on-state shows
  nothing until extra work exists elsewhere is configuration-first noise; the off state
  guarded only a speculative bulk opt-out for a list that is trivial to rebuild.
- **Extend dormancy symmetry to Groups** (kill their switches too): rejected for now —
  Groups reshape whole lists into sections across three collections (structure), while a
  desktop assignment is inert row metadata (annotation); annotations activate by use,
  structures keep an explicit opt-in switch.

Consequence: stored assignments are always live where supported; there is no way to
keep assignments but pause them. Accepting that loss is deliberate — reassignment costs
two clicks per entry.
