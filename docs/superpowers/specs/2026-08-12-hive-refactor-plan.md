# Hive refactor + testing plan

**Status:** plan (post first-draft spike)  
**Scope:** `.worktrees/hive` — refactor in place, no rewrite  
**Goal:** correct crate layering, one HTTP client implementation, test pyramid that catches regressions before real-box deploy

---

## 1. Why refactor, not rewrite

The first draft (~8k LOC, one session) got the **product architecture** right:

- `hive-host` = on-box truth (tmux, sessions, transcripts)
- `hive-hub` = cache + proxy (no SSH hot path)
- Clients = HTTP only (CLI, panel)

What it got wrong is **crate boundaries and duplication** — predictable for a spike:

| Problem | Impact |
|---------|--------|
| `hive-hub` → `hive-host` dependency | Hub links transcript/tmux/axum host code it never uses |
| Client HTTP duplicated in CLI + panel | Every new endpoint edited twice |
| systemd install duplicated host/hub | Drift on flags, bin paths, unit text |
| `hive-hub` monolith (`lib.rs` 426 LOC) | Hard to test poll/proxy/SSE separately |
| `hive-cli` is one `main.rs` | No shared lib for panel |
| Tests = transcript goldens only | No HTTP integration, no hub stack, no client tests |

Refactor cost: **~1–2 days focused agent work**. Rewrite cost: same architecture, new bugs, lost fixtures.

---

## 2. Target crate graph

```
hive-protocol          JSON types, ports, route prefix, ErrorBody
     │
hive-common            HiveHome, bind, config TOML, read_hosts,
     │                 migrate_from_cc, systemd install helper
     ├──────────────────┐
     │                  │
hive-host              hive-hub
(sessions, tmux,       (poll, cache, proxy, SSE)
 transcript, http)
     │                  │
     └────────┬─────────┘
              │
         hive-client     Endpoints, url build, reqwest,
              │          fleet/say/transcript/spawn/…
              │
         hive-cli        clap dispatch only (~150 LOC)
              │
         hive-panel      Tauri invoke + fleet cache + SSE listener
         (depends on hive-client + hive-protocol)
```

**Rules after refactor:**

1. `hive-hub` must **not** depend on `hive-host`.
2. `hive-client` must **not** depend on `hive-host` or `hive-hub` (only protocol + common + reqwest).
3. `hive-protocol` stays I/O-free.
4. Route paths use `hive_protocol::API_PREFIX` everywhere (no bare `"/v1"` strings).

---

## 3. Phases (execute in order)

Each phase ends with `cargo test --workspace` green and a short checklist. Do not start phase N+1 until N passes.

### Phase 0 — Baseline & harness (½ day)

**Purpose:** lock current behavior before moving code.

| Task | Detail |
|------|--------|
| 0.1 | Add root `Makefile` or `justfile`: `test`, `test-integration`, `check`, `build-release` |
| 0.2 | Document env vars: `HIVE_HOME`, `HIVE_HOST_BIN`, `HIVE_HUB_BIN`, `CLAUDE_PROJECTS_ROOT` |
| 0.3 | Snapshot “smoke script” `scripts/dev-stack.sh`: host `--dev` + hub `--dev` + `hive status` against loopback |
| 0.4 | Count current tests: protocol 1, host integration 10, hub 2, cli 0, panel 0 |

**Acceptance:** baseline script runs locally; test count recorded in this doc’s appendix.

---

### Phase 1 — `hive-common` (1 day)

**Extract from `hive-host`:**

| Module | Source today | New location |
|--------|--------------|--------------|
| `HiveHome`, `ensure_dir` | `hive-host/src/home.rs` | `hive-common/src/home.rs` |
| `bind` (in_tailnet, assert_bind_allowed, tailscale_v4) | `hive-host/src/bind.rs` | `hive-common/src/bind.rs` |
| `HostConfig`, `HubConfig`, `ClientToml` | host `config.rs`, hub `HubConfig`, cli/panel `ClientToml` | `hive-common/src/config.rs` |
| `read_hosts()` | `hive-hub/src/lib.rs` | `hive-common/src/hosts.rs` |
| `migrate_from_cc()` | `hive-host/src/install.rs` | `hive-common/src/migrate.rs` |
| `systemd_user_install()` | duplicated host + hub install | `hive-common/src/systemd.rs` |

**API sketch:**

```rust
// bind.rs — role-aware errors
pub fn assert_bind_allowed(role: &str, host: &str, dev: bool) -> Result<()>;

// systemd.rs
pub struct SystemdInstall {
    pub service: &'static str,  // "hive-host" | "hive-hub"
    pub exec: String,           // full argv prefix: "/path/hive-host serve"
    pub bind: String,
    pub port: u16,
    pub home: String,
}
pub fn install_user_unit(cfg: &SystemdInstall) -> Result<PathBuf>;
```

**Update dependents:**

- `hive-host`: `use hive_common::…`; delete moved modules; re-export nothing from old paths unless needed for one release.
- `hive-hub`: replace `hive-host` dep with `hive-common`.
- `hive-cli` / panel: use `HiveHome::resolve`, `ClientToml::load`.

**Tests (new in `hive-common`):**

| Test | What |
|------|------|
| `home_resolve_explicit_env_default` | `HIVE_HOME`, explicit path, `~/.hive` |
| `bind_rejects_public_and_loopback_without_dev` | table-driven |
| `bind_accepts_tailscale_cgnat` | `100.64.1.2` |
| `read_hosts_skips_comments_and_blanks` | move from hub |
| `migrate_from_cc_copies_once` | temp dirs, idempotent |
| `systemd_unit_text_snapshot` | insta or golden string for host + hub units |

**Acceptance:** `hive-hub/Cargo.toml` has zero dependency on `hive-host`. All prior tests still pass.

---

### Phase 2 — `hive-client` (1 day)

**New crate** used by CLI and panel.

| Module | Responsibility |
|--------|----------------|
| `endpoints.rs` | Parse `client.toml`, normalize hub URL (`http://`, port default), `host_url(host, path)` |
| `http.rs` | Shared `reqwest::Client`, `get_json`, `post_json`, map `ErrorBody` |
| `encode.rs` | `urlencoding` (one copy) |
| `api.rs` | `Client` struct: `fleet()`, `refresh_fleet()`, `sessions(host)`, `say`, `stop`, `spawn`, `transcript`, `fs`, `attach_argv` |

**Design choices:**

- `Client::new(home: &HiveHome) -> Result<Self>` — loads config once.
- Errors: `hive_client::Error` with `Http(status, ErrorBody)` and `Transport`.
- Panel fleet cache stays in panel (`RwLock<Option<Fleet>>`), not in client — client is stateless HTTP.
- CLI uses same `Client`; no duplicate `get_json`.

**Shrink consumers:**

- `hive-cli/src/main.rs`: keep clap + `print_status` + attach `Command`; delegate all HTTP to `hive_client::Client`.
- `apps/hive-panel/src-tauri/src/hub.rs`: thin wrapper calling `Client` + cache + `String` errors for Tauri.

**Tests (`hive-client`, use `wiremock` or `axum-test` server):**

| Test | What |
|------|------|
| `endpoints_hub_hostname_adds_port` | `hub` → `http://hub:8787` |
| `endpoints_hub_url_passthrough` | full URL unchanged |
| `host_url_via_hub_proxy` | `/v1/hosts/box/v1/sessions/foo` |
| `host_url_local_direct` | no hub → `http://127.0.0.1:8788/v1/…` |
| `get_json_maps_error_body` | 404 → code + message |
| `fleet_parses` | mock `/v1/fleet` JSON |

Prefer **`wiremock`** in dev-dependencies for stable HTTP fakes without spinning full host.

**Acceptance:** delete duplicated blocks in cli `main.rs` (~lines 107–186, 420–431) and panel `hub.rs` (~73–143, 259–270). Single implementation remains.

---

### Phase 3 — Split `hive-hub` (½ day)

**From `lib.rs` into:**

```
crates/hive-hub/src/
  lib.rs      run(), re-exports
  config.rs   HubConfig load/default
  hosts.rs    thin wrapper → hive_common::read_hosts (or delete file)
  state.rs    HubState, fleet RwLock, broadcast
  poll.rs     refresh_once, poll_loop, host_base
  proxy.rs    proxy handler
  http.rs     router, health, fleet, events SSE
  install.rs  HubCmd::Install (uses hive_common::systemd)
```

**Tests:**

| Test | What |
|------|------|
| `host_base` | already exists — keep |
| `proxy_rewrites_path` | unit test pure path join logic extracted from proxy |
| `poll_merges_host_errors` | mock reqwest/wiremock: one host 500, one 200 |

**Acceptance:** `lib.rs` under ~80 lines. No behavior change in `/v1/fleet` or proxy.

---

### Phase 4 — Split `hive-host` discover (½ day, optional but recommended)

**Problem:** `discover.rs` (479 LOC) mixes discovery, IO, tail, prompt detection.

**Split:**

```
crates/hive-host/src/
  discover/
    mod.rs           transcript_document orchestration
    claude.rs        find_claude_jsonl, slug
    codex.rs         find_codex_rollout
  transcript/        (rename from transcript.rs — adapters only)
    mod.rs
    claude_jsonl.rs
    codex_rollout.rs
```

Move `blocked_on_prompt` → `tmux.rs` or `sessions.rs` (pane state, not discovery).

**Tests:** existing `tests/transcript.rs` unchanged paths; add:

| Test | What |
|------|------|
| `discover_missing_session` | clear error code/message |
| `tail_respects_has_earlier` | fixture + temp dir |

**Acceptance:** no public API change on `/v1/sessions/{task}/transcript`.

---

### Phase 5 — CLI + install alignment (½ day)

| Task | Detail |
|------|--------|
| 5.1 | `hive host install` / `hive hub install` → delegate to `hive-host install` / `hive-hub install` subprocess OR call same `hive_common` + `run(Install)` — **one code path** |
| 5.2 | **Unify `--migrate`**: CLI install does **not** migrate unless `--migrate` (match native binaries) |
| 5.3 | `install-host.sh` / `install-hub.sh` pass `--migrate` explicitly |
| 5.4 | Optional: `hive-cli` becomes `lib.rs` + thin `main.rs` if any logic remains |

**Tests:**

| Test | What |
|------|------|
| `cli_install_unit_text` | integration with temp `HOME`, no systemctl (mock or skip if no systemd) |
| `install_scripts_dry_run` | bash `-n` + document expected ssh invocations |

---

### Phase 6 — Panel frontend hygiene (½ day)

| Task | Detail |
|------|--------|
| 6.1 | `apps/hive-panel/src/types.ts` — types aligned with `hive-protocol` (Fleet, SessionRow, TranscriptDoc, ChatBlock) |
| 6.2 | Import types in `main.tsx`, `ChatPanel.tsx`, `SpawnDialog.tsx` |
| 6.3 | Extract SSE parser from `lib.rs` → `sse.rs` with unit tests (frame splitting) |
| 6.4 | Consider adding panel to workspace as optional member OR document why excluded |

**Tests:**

| Test | What |
|------|------|
| `npm run check` | tsc |
| `sse_parse_multi_frame` | Rust unit test on sample bytes |
| Optional | one Vitest test for `flattenFleet()` pure function extracted from main |

---

## 4. Testing strategy (pyramid)

### Layer A — Unit tests (fast, no network)

**Crates:** `hive-protocol`, `hive-common`, `hive-client`, `hive-host` (transcript/adapters/meta/bind), `hive-hub` (path math).

| Area | Examples |
|------|----------|
| Protocol | JSON round-trip, `ErrorBody`, `AttachCommand` |
| Common | bind table, hosts parse, migrate idempotent |
| Client | URL building, error mapping (wiremock) |
| Host | existing transcript goldens, meta, complete_lines |
| Hub | host_base, proxy path strip |

**Target:** 40+ unit tests (today ~13).

---

### Layer B — HTTP integration tests (in-process)

**New crate or directory:** `crates/hive-testing` (dev-only) **or** `crates/hive-host/tests/http.rs` + `crates/hive-hub/tests/http.rs`.

**Technique:** `axum::Router` from `hive_host::http::router(home)` + `tower::ServiceExt::oneshot` (or `axum-test` crate).

**Host integration scenarios** (temp `HIVE_HOME` per test):

| # | Scenario |
|---|----------|
| H1 | `GET /v1/health` → `{ ok: true, role: "host" }` |
| H2 | `GET /v1/sessions` empty → `[]` |
| H3 | Write fake `.meta` + mock tmux optional → row shape (or skip tmux if not installed — use trait injection later) |
| H4 | Transcript with fixture jsonl + env `CLAUDE_PROJECTS_ROOT` |
| H5 | `POST /v1/sessions/{task}/label` star/unstar |
| H6 | `GET /v1/fs?path=` on temp dir |

**Hub integration scenarios** (wiremock standalone host **or** spawn host router on random port):

| # | Scenario |
|---|----------|
| B1 | `GET /v1/fleet` returns cache immediately (empty hosts) |
| B2 | One mock host `/v1/sessions` → fleet row |
| B3 | Proxy `GET /v1/hosts/{box}/v1/health` forwards |
| B4 | Host unreachable → `error` field on `HostFleet`, no hang |

**Client integration:** `hive-client` tests against the in-process host/hub routers (no TCP).

---

### Layer C — Stack tests (TCP, local only)

**File:** `tests/stack_test.rs` at workspace root (integration test crate) or `scripts/stack-test.sh` invoked from Makefile.

**Flow:**

1. Pick free ports `8788`, `8787`.
2. Spawn `hive-host serve --dev --bind 127.0.0.1 --port PORT_H`.
3. Write minimal `~/.hive/hosts` → `127.0.0.1` (hub must reach host — use `host_port` in hub.toml).
4. Spawn `hive-hub serve --dev --bind 127.0.0.1 --port PORT_U`.
5. Write `client.toml` → `http://127.0.0.1:PORT_U`.
6. Run `hive status` (subprocess) — exit 0.
7. Optional: `hive-client` fleet fetch asserts JSON shape.

Mark `#[ignore]` for CI without network stack; run in `make test-integration`.

---

### Layer D — Install script tests (shell)

| Test | Method |
|------|--------|
| `install-host.sh --help` | exits 0 |
| `--no-build` with prebuilt binary | docker or skip in CI |
| Remote steps | manual checklist on one real box |

---

### Layer E — Panel / frontend

| Test | Method |
|------|--------|
| Typecheck | `npm run check` in CI |
| SSE parser | Rust unit tests |
| E2E | **defer** — Tauri WebDriver is heavy; rely on stack tests + manual panel smoke |

---

### Layer F — Regression fixtures (keep)

| Fixture | Location | Used by |
|---------|----------|---------|
| `transcript-claude-session.jsonl` | `tests/fixtures/` | host transcript + HTTP transcript |
| `transcript-codex-rollout.jsonl` | same | same |

Add fixture snapshots for **HTTP JSON** (optional insta snapshots of `/v1/sessions` row, transcript tail doc) once shapes stabilize.

---

## 5. CI / local commands (target)

```makefile
# Makefile (target state)
test:              cargo test --workspace
test-integration:  cargo test --workspace -- --ignored
check:             cargo clippy --workspace -- -D warnings && cargo fmt --check
panel-check:       cd apps/hive-panel && npm run check
release:           cargo build --release -p hive-host -p hive-hub -p hive-cli
stack-smoke:       ./scripts/dev-stack.sh
```

**CI pipeline (when added):**

1. `cargo fmt --check`
2. `cargo clippy`
3. `cargo test --workspace`
4. `cargo test --workspace -- --ignored` (stack tests, optional job)
5. `cd apps/hive-panel && npm ci && npm run check`

---

## 6. What not to do

- **Do not** rewrite host transcript adapters — goldens are valuable.
- **Do not** add SSH to hub hot path “for convenience.”
- **Do not** merge panel into workspace until Tauri build time is acceptable in CI.
- **Do not** chase 100% coverage on `tmux`/`run` without injection — mark as manual/box tests initially.
- **Do not** block refactor on phone/web SPA — out of scope.

---

## 7. Execution schedule (suggested)

| Day | Phases | Deliverable |
|-----|--------|-------------|
| 1 AM | 0 + 1 | `hive-common`, hub decoupled, +15 unit tests |
| 1 PM | 2 | `hive-client`, CLI/panel deduped, +10 wiremock tests |
| 2 AM | 3 + 4 | hub + discover split, same behavior |
| 2 PM | 5 + 6 | install alignment, panel types, Makefile |
| 3 | Stack tests + docs | `dev-stack.sh`, update design spec crate graph |

Total: **~2–3 days** for one agent session with verification gates.

---

## 8. Definition of done (refactor complete)

- [ ] Crate graph matches section 2; `cargo tree -p hive-hub` shows no `hive-host`
- [ ] Single HTTP client implementation in `hive-client`
- [ ] Single systemd install in `hive-common`
- [ ] `API_PREFIX` used in host, hub, client URL builders
- [ ] `cargo test --workspace` ≥ 40 tests, all green
- [ ] At least 6 host HTTP integration tests (Layer B)
- [ ] At least 3 hub integration tests (Layer B)
- [ ] Stack smoke script documented and passing locally
- [ ] README crate diagram updated
- [ ] `--migrate` semantics identical across CLI, scripts, native install

---

## Appendix A — Current test inventory (baseline)

| Location | Count | Notes |
|----------|-------|-------|
| `hive-protocol` | 1 | error_body_json |
| `hive-host/tests/transcript.rs` | 10 | adapters + bind + meta + tail |
| `hive-hub` | 2 | host_base, read_hosts |
| `hive-cli` | 0 | |
| `hive-panel` | 0 | tsc only |

---

## Appendix B — Files to delete or shrink after refactor

| File | After |
|------|-------|
| `hive-host/src/home.rs`, `bind.rs` | moved to common |
| `hive-host/src/config.rs` | HostConfig in common |
| Duplicated client code in `hive-cli/main.rs` | ~200 lines removed |
| `hive-panel/.../hub.rs` | ~150 lines → thin wrapper |
| Monolithic `hive-hub/src/lib.rs` | split into 6 modules |

---

## Appendix C — Resolved decisions (2026-08-12)

1. **Integration tests:** per-crate `tests/` only — no shared `hive-testing` crate. Extract helpers later if duplication grows.
2. **HTTP integration:** axum `tower::ServiceExt::oneshot` against `hive_host::http::router` in-process; wiremock deferred until `hive-client` (phase 2).
3. **Tmux in tests:** real tmux on PATH — `crates/hive-host/tests/tmux.rs` creates/kills a detached session. Tests skip gracefully only when tmux is absent (not stubbed via env var).

Phase 0 harness: `Makefile` (`test`, `test-integration`, `stack-smoke`) + `scripts/dev-stack.sh`.
