# Optional AI providers share scoped discovery and explicit cloud disclosure

> Status: amended 2026-09-06 — accepted design; original text preserved. See dated amendments for current scope and tracker numbering. Implementation is pending.

AI assistance is optional and installs nothing by default. Sprout offers a prominent managed local setup, an existing local model service connection, and a user-configured cloud provider. All modes share the authoring-only policy in ADR-0030. Local inference reduces provider exposure; it does not remove model mistakes or risks from untrusted local content.

## Decisions

- Managed setup explicitly downloads verified model weights and the runtime needed to run them on the device. The runtime starts on demand, unloads the model after inactivity, and stops when Sprout actually exits. Closing a main window while Sprout remains in the tray is not application exit. The idle interval, supported runtime, and hardware requirements are qualification/implementation details, not fixed by this ADR.
- Existing-local mode connects to a user-managed inference service and selects a model it exposes. Sprout does not install, stop, unload, reconfigure, or delete that service or its models. Direct import of arbitrary model files is outside the first release. Supported wire protocols are qualified in ticket 145; an arbitrary endpoint is not promised compatible merely because it serves an LLM.
- Cloud mode requires a user-configured provider/API key and a warning before enabling it or transmitting inference content: prompts and included context may contain paths, filenames, script contents, error output, and system information; the configured third party receives that content under its policies. Cancellation sends nothing. Changing the destination requires renewed disclosure. There is no silent fallback from local to cloud.
- On-device means actual on-device inference, not a UI label applied to any URL. A remote or unverified service cannot inherit the managed-local privacy claim. Endpoint validation and redirects must not silently turn a local connection into off-device transmission. Third-party local-runtime privacy remains dependent on that runtime's own configuration.
- An explicit find request authorizes bounded read-only discovery of installed-app metadata and previously approved folders. File names/paths are sufficient by default; reading contents or extending the search scope needs explicit approval. Ambiguous matches require selection. Folder boundaries, reparse points, result limits, cancellation, and expiry of target references are enforced by Sprout rather than model instructions.
- Searching locally and disclosing results to a provider are separate permissions. Where possible, the model receives an opaque target reference and trusted local code resolves and quotes the path. Unknown or modified references are refused. When raw context is necessary, the user previews and approves the additional material before sending. Enabling cloud mode is not blanket permission to upload future discovery results.
- Prompt history and discovered content are not silently collected as a persistent dataset. Credentials are machine-local secrets, excluded from prompts, ordinary logs, browser storage, and whole-app backups. A concrete protected credential-store mechanism and retention limits are implementation work, not an assertion that they already exist.

## Considered options and consequences

Mandatory local installation was rejected in favor of opt-in setup and existing/cloud alternatives. Unrestricted filesystem access was rejected even for local models. Sending every discovered path to cloud models was rejected in favor of local resolution and explicit disclosure. Provider and discovery complexity belongs behind the AI assistance interface; existing Windows operation owners remain authoritative under ADR-0029.

## Amendment — 2026-09-06 (tracker numbering)

Another session published issue 144 before this planning package was completed. The authoritative parent is [spec 145](../../.scratch/sprout-app/issues/145-ai-assisted-quick-action-authoring-spec.md), implemented by tickets 146–155. The original ticket-145 qualification reference above is now [ticket 146](../../.scratch/sprout-app/issues/146-ai-model-runtime-provider-qualification.md). Only tracker numbering changed; the decision is unchanged.

## Amendment — 2026-09-06 (installation and model compatibility)

Enabling AI alone downloads nothing. Managed setup requires a separate explicit Install action for the chosen verified model and runtime. Existing-local and cloud setup require no managed download.

Compatible models reuse one qualified runtime/provider integration through explicit model selection and validated model-specific configuration. Record artifact identity, context limits, template requirements, memory needs and minimum runtime version. A new architecture or unsupported protocol may require a separately tested runtime/adapter update and app release; JSON alone cannot add missing support. Cloud support is a bounded tested API set, not one implementation per model or universal API compatibility. Tickets 150–152 implement these distinctions.
