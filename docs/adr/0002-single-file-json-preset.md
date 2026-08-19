# Presets are single-file JSON with a schema version

Exported presets are one self-contained `.sprout.json` — `schemaVersion`, metadata, and full product definitions embedded — so the file is the unit of sharing and needs no knowledge of the recipient's library or Sprout version. JSON over YAML because the backend is serde-native, validation is strict, and the file stays diffable in git; YAML would add a parser and looser semantics for no gain. `schemaVersion` guards future format changes: too-new or too-old files are rejected with a clear message instead of being misread.
