# Verification helpers — agent reference

> Read this file when: you need to inspect the Library database or reason
> about first-run state. Otherwise skip it.

- Inspect the Library DB read-only (Node has built-in sqlite):
  `node -e "const {DatabaseSync}=require('node:sqlite');const d=new DatabaseSync(process.env.LOCALAPPDATA+'\\Sprout\\sprout.db',{readOnly:true});console.log(d.prepare('SELECT COUNT(*) c FROM products').get().c)"`
- Fresh installs open to an empty Library (ADR-0008): `c` is 0 until the user adds Products from the live winget registry search — nothing is seeded.
