# Local agent registry — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Laptop-side agent bookmarks in `~/.hive/local-agents.json` with **`hive local`** CLI (register, groups, open-group) and a **Local** section in hive-panel — so Dan can restore a tagged cohort after laptop shutdown without hunting paths; Cursor agents use `cursor .` + GUI chat selection.

**Architecture:** Shared Rust crate **`hive-local-agents`** (load/save/validate/filter) used by **hive-cli** and **hive-panel** Tauri. No hub/host API; fleet poll unchanged. Groups are **shared tags** on records. macOS **Open** reuses panel terminal backends where possible.

**Tech stack:** Rust (`hive-local-agents`, `hive-cli`), Tauri 2 + React (panel), `local-agents.json` v1.

**Spec:** [../specs/2026-10-04-hive-panel-local-agents.md](../specs/2026-10-04-hive-panel-local-agents.md)  
**Operator guide:** [../specs/2026-10-04-hive-local-agents-operator-guide.md](../specs/2026-10-04-hive-local-agents-operator-guide.md)

## Global constraints

- Registry path: `$HIVE_HOME/local-agents.json` (default `~/.hive/local-agents.json`).
- Schema version `1`; agent fields and validation per spec (id slug, cwd must exist on write, resume single line ≤2048).
- Local rows **never** appear in `GET /v1/fleet` or hub poll.
- **`hive local register`** upserts; **`hive local add`** fails on duplicate id.
- Cursor laptop **`resume`**: `'cursor .'` until devbox Cursor CLI pilot changes it.
- No fleet hostnames/addresses in repo docs; examples use placeholder slugs (`my-agent`, `my-team`).

## Review focus

| Risk | Expected behavior | Test owner |
|------|-------------------|------------|
| Corrupt / missing JSON | Empty registry, no panic | Task 1 |
| Duplicate `add` | Error | Task 1 |
| `register` twice same id | Second write updates row | Task 1 |
| `open-group` no matches | Non-zero exit, clear message | Task 2 |
| Panel + CLI concurrent write | Last write wins; prefer panel reload after save | Task 5 |
| `cwd` deleted after register | `open` still prints line; panel may warn on edit | Task 5 (warn-only) |

## Current state (2026-10-04)

- [x] **`hive-local-agents`** crate: CRUD, tag filter, tests.
- [x] **`hive local`** CLI: list/show/add/register/set/remove/open/open-group; macOS `--launch` (Terminal.app in CLI; panel uses `client.toml` terminal).
- [x] Spec + operator guide + plan.
- [x] Panel Local section + Tauri CRUD/open/open-group.
- [ ] CLI `--launch` still Terminal.app only (panel Open uses configured terminal).
- [ ] Task 6 fleet self-register (agents, out of repo).
- [ ] Task 7 final verification + spec status → implemented.

---

### Task 1: Land library + CLI on main

**Files:**
- Create: `crates/hive-local-agents/` (already on disk)
- Create: `crates/hive-cli/src/local.rs`, `local_launch.rs`
- Modify: `Cargo.toml`, `crates/hive-cli/Cargo.toml`, `crates/hive-cli/src/lib.rs`, `Cargo.lock`

**Interfaces:**
- Produces: `hive_local_agents::{load, save, add, register, set, remove, get, agents_matching_tags, open_shell_line, LocalAgent, …}`

- [ ] **Step 1:** Run `cargo test -p hive-local-agents -p hive-cli` — all pass.
- [ ] **Step 2:** Run `hive local --help` and `hive local open-group --help` — text matches operator guide.
- [ ] **Step 3:** Commit: `Add hive local registry CLI and hive-local-agents crate.`

---

### Task 2: CLI polish — help, tests, terminal preference

**Files:**
- Modify: `crates/hive-cli/src/local.rs` (long_about / after_help / per-subcommand about)
- Modify: `crates/hive-cli/src/local_launch.rs`
- Modify: `crates/hive-cli/src/lib.rs` (test for open-group in help)
- Test: `crates/hive-cli/src/local.rs` or integration test for `open-group` empty tag set

**Interfaces:**
- Consumes: `hive_common::ClientToml::load` for `terminal` field (same as panel)

- [ ] **Step 1:** Add test: `open-group` with no matching agents exits with error message containing tag name.

```rust
// in hive-local-agents or cli test with temp HIVE_HOME
```

- [ ] **Step 2:** Align `LOCAL_LONG_ABOUT` with operator guide (Cursor: `cursor .`, not `cursor-agent` as default).
- [ ] **Step 3:** Optional: read `~/.hive/client.toml` terminal setting and reuse panel iTerm/Ghostty osascript paths (extract shared helper or duplicate minimal from `apps/hive-panel/src-tauri/src/terminal.rs` into `hive-cli` `local_launch.rs`).
- [ ] **Step 4:** `cargo test -p hive-cli`; commit: `Improve hive local help and open-group errors.`

---

### Task 3: Documentation pass

**Files:**
- Modify: [../specs/2026-10-04-hive-panel-local-agents.md](../specs/2026-10-04-hive-panel-local-agents.md) (dedupe CLI table, link operator guide)
- Modify: [../specs/2026-10-04-hive-local-agents-operator-guide.md](../specs/2026-10-04-hive-local-agents-operator-guide.md) if needed
- Modify: [../../CLAUDE.md](../../CLAUDE.md) — one bullet under “Commands agents should know”: `hive local register …`, link operator guide

- [ ] **Step 1:** Spec CLI table: single **`list`** row with `--tag` / `--json`; examples use `'cursor .'`.
- [ ] **Step 2:** Add “Documentation” section in spec pointing to operator guide.
- [ ] **Step 3:** CLAUDE.md bullet; commit: `Docs: hive local operator guide and plan.`

---

### Task 4: Panel — Tauri read/write registry

**Files:**
- Modify: `apps/hive-panel/src-tauri/Cargo.toml` — depend on `hive-local-agents`
- Create: `apps/hive-panel/src-tauri/src/local_agents.rs`
- Modify: `apps/hive-panel/src-tauri/src/lib.rs` — commands `local_agents_list`, `local_agents_save`, etc.
- Modify: `apps/hive-panel/src/types.ts` — `LocalAgent` type

**Interfaces:**
- Produces: Tauri commands returning `Vec<LocalAgent>` and accepting CRUD payloads matching JSON schema.

- [ ] **Step 1:** Implement `list_local_agents_cmd` → `hive_local_agents::load`.
- [ ] **Step 2:** Implement `register_local_agent_cmd` / `remove_local_agent_cmd` wrapping crate API.
- [ ] **Step 3:** Smoke: `npm run tauri dev`, invoke from devtools or temporary button.
- [ ] **Step 4:** Commit: `Panel Tauri: read/write local-agents.json.`

---

### Task 5: Panel — Local section UI

**Files:**
- Create: `apps/hive-panel/src/LocalAgents.tsx` (table/tiles, tag filter, CRUD form)
- Modify: `apps/hive-panel/src/main.tsx` — section below fleet filters
- Modify: `apps/hive-panel/src/styles.css`

**Interfaces:**
- Consumes: Tauri commands from Task 4
- Reuses: `SessionTags`-style tag chips where applicable; `terminal::open_shell_at` pattern for **Open** with `cd && resume` on **local** path (no SSH)

- [ ] **Step 1:** Render Local list from Tauri on app load; refresh after edits.
- [ ] **Step 2:** Row actions: **Open** (external terminal), **Edit**, **Remove**; header **Add**.
- [ ] **Step 3:** Tag filter chips + **Open group** (all rows matching selected tag(s)).
- [ ] **Step 4:** `npm test` in hive-panel; manual Open smoke on macOS.
- [ ] **Step 5:** Commit: `Panel: Local agents section with open and open-group.`

---

### Task 6: Fleet agent adoption (out of repo, checklist)

**Files (external):** Each team agent’s bootstrap / `CLAUDE.md` or wrapper — not hive-repo.

- [ ] **Step 1:** Post operator guide link on agentmsg (thread for laptop restore).
- [ ] **Step 2:** Each poll agent adds `hive local register …` to wake path with `--tags laptop,my-team`.
- [ ] **Step 3:** Dan: one manual `open-group` dry-run without `--launch`, then with `--launch`.

No commit in hive-repo required unless adding example script under `docs/` only.

---

### Task 7: Verification before “done”

- [ ] `cargo test --workspace` (allow known tmux flake if pre-existing).
- [ ] `make web-check` if panel touched.
- [ ] Operator guide walkthrough: register → list → open → open-group on empty and non-empty registry.
- [ ] Update spec **Status** to “implemented” for CLI + panel when Tasks 1–5 complete.

---

## Execution recommendation

**Native (single session)** fits Tasks 1–3 (small, already mostly coded). **Subagent-driven** helps Tasks 4–5 (panel UI + Tauri) as a separate pass with review.

## Self-review (spec coverage)

| Spec section | Task |
|--------------|------|
| Storage / fields | 1, 4 |
| CLI subcommands | 1, 2 |
| Self-registration | 1, 3, 6 |
| Groups / open-group | 1, 2, 5 |
| Cursor wrinkle | 3 (docs), 6 |
| Panel UX | 4, 5 |
| Not in scope (hub, cursor provider) | — (no task) |
