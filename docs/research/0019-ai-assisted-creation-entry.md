# AI-assisted creation entry: inline hero, not a buried toggle or a second dialog

Status: evidence + Sprout inference. No Sprout usability study has validated
exact placement; the applied choice is recorded in research 0006.

## Sources

- NN/g, Moran, *Scope in Generative AI Features* (2025-02-28) —
  https://www.nngroup.com/articles/scope-ai-features/ — narrow-scope
  features outperform broad chat on understanding and adoption: guided UI,
  short prompts, structured reviewable output.
- NN/g, *Prompt Controls in GenAI Chatbots* (2024-08-02) —
  https://www.nngroup.com/articles/prompt-controls-genai/ — controls must
  raise discoverability, offer inspiration, set constraints, enable
  followups; standard icon+label, clear names, grouping, conventions.
- NN/g, *AI Chat Is Not (Always) the Answer* (2024-03-01) —
  https://www.nngroup.com/articles/ai-chat-not-the-answer/ — embed narrow
  product-specific AI; do not bolt on universal chat.
- NN/g, *Accordion Editing and Apple Picking* (2023-09-24) —
  https://www.nngroup.com/articles/accordion-editing-apple-picking/ — users
  iterate AI output and finish by direct manual editing outside the chat.
- Microsoft, *Guidelines for Human-AI Interaction* (CHI 2019,
  https://aka.ms/aiguidelines) — G1 make clear what it can do, G2 how well,
  G7 efficient invocation, G8 efficient dismissal, G9 efficient correction.
- Microsoft Learn, *Human-centered Design for Agents* (via
  learn.microsoft.com/agents/design-guidelines/human-centered-design) —
  user initiates ("Summarize with Copilot", not "Copilot, summarize");
  outputs stay editable with visible refine controls.
- Google PAIR, *People + AI Guidebook* (https://pair.withgoogle.com/guidebook/)
  — onboard in stages, in-the-moment help, low-risk reversible first
  action, explain-for-understanding with progressive disclosure.
- Gmail Help, *Draft emails with Gemini* (support.google.com/mail/answer/13955415)
  — one compose surface, distinct `Help me write` trigger inside it,
  `Create` → `Insert / Refine / Recreate`; manual compose untouched.
- Figma, *Make* (figma.com/make) + NN/g 2025-05-09 AI-design-tools update —
  a separate prompt surface is justified only when the artifact differs
  (0→1 prototype vs precise UI); otherwise hybrid wins (AI head-start,
  manual polish).

## Sprout inference

A creation dialog whose artifact is identical either way (one Quick Action)
keeps one dialog: a buried mid-form toggle fails invocation/discoverability
(HAX G7, prompt-controls), while a wholly separate "Add with AI" dialog
splits the surface grammar and duplicates validation/save. The accepted
shape is a single dialog with an AI-first inline hero when ready (explicit
`Generate draft` → review card → explicit `Use this draft` apply, Gmail's
`Insert` grammar), mandatory manual validation below, easy `Dismiss`
(HAX G8), direct editing after apply (HAX G9, accordion behavior), and zero
AI chrome until setup (0006 pattern 3).
