# DBCraft

Native, open-source database client in the ArtCraft style — familiar Navicat-like workflows,
written in pure Rust (egui/eframe), with shared [craft-libs](https://github.com/storytold/craft-libs).

**MVP engines:** MySQL / MariaDB (priority), SQLite, PostgreSQL.

## Features (MVP)

- Connection manager (profiles in `craft-settings`, passwords in the OS keyring)
- Schema browser (databases → tables → columns)
- SQL editor and result grid
- Table data preview
- Headless CLI for agents

## Build

Requires Rust 1.95+ (edition 2024).

```sh
cargo run -p dbcraft
cargo run -p dbcraft-cli -- --help
cargo test --workspace
```

## License

MIT OR Apache-2.0
