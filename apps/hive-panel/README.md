# hive-panel

Tauri + React GUI for the hive fleet. Talks to **hive-hub** (or local **hive-host**) via the Rust HTTP client in `src-tauri/src/hub.rs` — no SSH, no legacy `cc-*` tools.

## Why this app is not in the root Cargo workspace

The repo workspace (`Cargo.toml` at the root) lists `hive-protocol`, `hive-common`, `hive-client`, `hive-host`, `hive-hub`, and `hive-cli`. **hive-panel is excluded** because:

1. **Tauri owns its own `[workspace]`** in `src-tauri/Cargo.toml` (required by `tauri-build` / standalone packaging).
2. **Different toolchains** — frontend uses Vite + TypeScript (`npm run check`); backend is a nested crate that path-depends on workspace crates.
3. **Build entrypoint** — `npm run tauri build` drives both sides; keeping the panel out avoids workspace feature unification conflicts with the daemon crates.

Path dependencies still point at `../../crates/hive-*` so types and `hive-client` stay in sync.

## Dev

```bash
cd apps/hive-panel
npm install
npm run check          # TypeScript
npm test               # vitest (fleet helpers)
npm run tauri dev      # GUI (needs hive-hub or local host)
```

Rust unit tests for the Tauri backend:

```bash
cd src-tauri && cargo test
```
