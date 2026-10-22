# AGENTS.md: guide for AI agents and contributors

DBCraft is an open-source, native database client (Navicat-comparable workflows) written in
**Rust only**. **No Tauri, Electron or webview shells:** the desktop app is native egui/eframe.
The product name is always written **DBCraft** in user-facing text. Machine names stay lowercase:
crates (`dbcraft-*`), binaries, file names, ids (`ai.storyteller.dbcraft`).

Shared Craft standards live in [storytold/craft-libs](https://github.com/storytold/craft-libs)
(pinned by immutable git revision) and craftrules where available.

## Workspace map

```text
crates/
  core      L0 pure data (profiles, schema, query results)
  driver    L1 sqlx backends (MySQL/MariaDB, SQLite, PostgreSQL)
  engine    L2 Session + command registry + async jobs
  ui-egui   L3 thin egui shell (all actions go through the engine)
apps/
  dbcraft       desktop app
  dbcraft-cli   headless CLI
xtask/          cargo xtask layers
```

**Layering:** a crate may depend only on lower layers. Nothing below `ui-egui` may use egui,
eframe, winit or rfd.

## Golden rules

1. **Never crash.** No `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!` in non-test code.
   Return the crate's error type. Every crate's `lib.rs` starts with the clippy deny list.
2. **Everything is a command.** User-visible behaviour = a command in the engine with id, label,
   params, `enabled` and `run`. UI and CLI dispatch by id.
3. **The UI is thin.** UI state is serializable; colours come from theme tokens.
4. **No `unsafe`.** Workspace forbids it.
5. **Tests are the gate.** Bad params must return `Err`, never panic.
