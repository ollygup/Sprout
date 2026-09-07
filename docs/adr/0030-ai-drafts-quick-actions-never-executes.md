# AI assists Quick Action authoring; execution remains user controlled

> Status: amended 2026-09-06 — accepted design; original text preserved. See dated amendments for current scope and tracker numbering. Implementation is pending.

Sprout's AI assistance produces PowerShell or CMD Script drafts. The user reviews and explicitly saves a draft as a Quick Action, then uses the existing Run control. Generating, diagnosing, revising, accepting, or saving must never execute the script. This separates a model's fallible suggestions from the user's authority to change the machine, while preserving one-click execution of saved actions.

## Decisions

- AI assistance has no general terminal, Quick Action Run/Stop/Test, Quick Launch command-test, elevation, or automatic repair-and-retry capability. Starting a Sprout-managed inference runtime and performing bounded, app-owned read-only discovery are distinct authorized operations, not permission to execute generated code.
- Highly destructive requests are refused, including disk wipes/formatting, broad permanent deletion, credential extraction, and disabling security protections. Policy also applies to generated output, including revisions and stop commands; model self-assessment cannot authorize release of a draft. The precise rule corpus and adversarial examples are a blocking qualification deliverable in ticket 145.
- Refusal instructions, curated skills, static checks, and user review reduce risk. They do not prove arbitrary PowerShell/CMD harmless. The product must not claim a sandbox, guaranteed safety, successful execution, or a successful dry run based on generation, parsing, execution policy, or WhatIf.
- A revision remains separate from the saved Quick Action until explicit acceptance and saving. Its explanation and changed command are reviewable; rejecting, cancelling, or failing generation leaves the saved record intact. Concurrent edits must not be overwritten silently.
- New AI-assisted actions have auto-run off. The AI cannot change auto-run, run/stop controls, Group membership, or other execution metadata silently. Existing user-configured auto-run behavior is preserved; revision review exposes an existing auto-run flag so accepting a different script is deliberate. This feature introduces no automatic execution path.

## Considered options

An autonomous script agent that executes to test and repair was rejected by the user. A universal ban on all file modification was not the selected scope: scoped routine tasks remain useful, while highly destructive assistance is refused. A model/skill-only safety gate was rejected because the output being checked cannot own its authorization.

## Consequences

The existing manual editor and Run/Stop lifecycle remain usable without AI or an available provider. This decision governs AI assistance, not a retroactive prohibition on all manually authored Quick Actions. Discovery and cloud disclosure follow ADR-0031; trusted recommendation and skill distribution follow ADR-0032. Existing Quick Action privileges and tracking limitations in ADR-0017 remain material.

## Amendment — 2026-09-06 (tracker numbering)

Another session published issue 144 before this planning package was completed. The authoritative parent is [spec 145](../../.scratch/sprout-app/issues/145-ai-assisted-quick-action-authoring-spec.md), implemented by tickets 146–155. The original ticket-145 qualification reference above is now [ticket 146](../../.scratch/sprout-app/issues/146-ai-model-runtime-provider-qualification.md). Only tracker numbering changed; the decision is unchanged.

## Amendment — 2026-09-06 (destructive authoring boundary)

The user tightened the original highly-destructive-only threshold to refuse destructive AI assistance. Refuse permanent deletion even for one file, destructive overwrites, disk wipes/formatting, security weakening, credential extraction, and equivalent composed or disguised effects. Uncertain effects require clarification or refusal. This supersedes the narrower threshold above; ordinary non-destructive tasks remain supported. The same boundary applies to diagnosis of manual or AI-authored scripts: do not repair or complete destructive behavior.

Present plainly: "Destructive commands are outside AI assistance. You can write and manage those commands in the manual editor." Do not provide destructive code, actionable workaround steps or a prefilled manual draft after refusal. Risk explanations and non-destructive alternatives remain allowed. Manual authorship remains available without a safety guarantee or claim that responsibility transfers away from Sprout. Fixed app-owned removal of explicitly selected managed inference artifacts grants the model no deletion capability. Tickets 146, 148, 153–155 carry qualification and verification obligations.
