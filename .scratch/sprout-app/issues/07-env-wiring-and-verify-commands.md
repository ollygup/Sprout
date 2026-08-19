# 07 — Env wiring and verify commands

**What to build:** Making installs actually usable: Env wiring applied after successful installs (set/prepend, User scope only, never overwriting existing values, `<InstallLocation>` resolved from the uninstall registry via hint), and optional verify commands that fail a Requirement loudly when the installed product doesn't behave as declared. This ticket makes "JAVA_HOME is set and `java -version` really says 21" work end to end.

**Blocked by:** 05 — Run execution in-process with winget steps and results

**Status:** done

- [x] `set` applies after a successful install only when both User and Machine scopes are unset; `prepend` only when the value is absent from both scopes; existing values are never overwritten
- [x] `<InstallLocation>` and `<InstallLocation:hint>` resolve from the uninstall registry at apply time; unresolved placeholders skip with a note in the Run
- [x] Verify commands run after install; non-zero exit or non-matching output fails the Requirement loudly
