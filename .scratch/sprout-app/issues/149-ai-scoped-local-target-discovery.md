# 149 — Find approved local targets without giving AI a terminal

**What to build:** Resolve a request such as “write an action that opens X” using installed-app metadata or user-approved folders, let the user choose an ambiguous match, and bind the chosen local path into the reviewed draft.

**Blocked by:** 148 — local drafting workflow.

**Status:** ready-for-agent

**Parent:** [145 — AI-assisted Quick Action authoring](145-ai-assisted-quick-action-authoring-spec.md).

## ACs

- [ ] An explicit find request can query existing installed-app discovery and approved-folder names/paths; no background disk crawl, automatic file-body reading, or execution of discovered targets is introduced.
- [ ] Provide approve/revoke location controls and a separate request to expand scope or read contents. Enforce approved roots, canonical paths, reparse-point containment, limits, cancellation, and bounded results outside the model.
- [ ] Show ambiguous matches for user selection; report no match, stale/missing target, inaccessible folder, cancellation, and lookup failure honestly rather than fabricating a path.
- [ ] Return request-scoped opaque target references to inference where possible. Trusted local code binds validated references using shell-appropriate quoting; unknown, modified, cross-request, or stale references cannot become arbitrary paths or code.
- [ ] Validate the final locally bound command and expose its actual target to the user before saving. A reference grants no execution and cannot be used to run a target merely to identify it.
- [ ] Keep discovery grants separate from disclosure grants. A request context records which raw fields were approved so the later cloud path cannot inherit filesystem access as upload permission.
- [ ] Cover duplicate app names, paths containing spaces/quotes/metacharacters/Unicode, reparse-point escape attempts, revoked roots, truncated result sets, stale references, and injected filenames/content through observable workflow tests.
- [ ] Extend existing discovery/Windows operation owners; do not duplicate Start Menu/registry/Store enumeration or introduce a model-facing general shell command.

## Verification

Demo app lookup and selected-folder lookup with a deterministic discovery adapter, then a benign real local target. Verify no content reads or external transmission without the respective permission and no script execution during discovery/binding.

## Implementation notes

Lookup tools are app-owned read-only operations. An opaque reference is data to validate, not a hidden string interpolation channel. Locally resolved paths later included in diagnosis need disclosure review again.
