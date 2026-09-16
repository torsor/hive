# Design: Hive desktop app — Panel, Configuration, Setup

**Date:** 2026-08-14  
**Status:** draft (approved in conversation)  
**Scope:** shareable Tauri desktop application for hive — three distinct experiences in one shell  
**See also:** [2026-08-13-hive-install-locations.md](2026-08-13-hive-install-locations.md), [2026-08-12-hive-refactor-plan.md](2026-08-12-hive-refactor-plan.md), `ansible/README.md`

## Intent

Hive already separates **on-box truth** (`hive-host`), **fleet cache/proxy** (`hive-hub`), and **HTTP clients** (CLI, panel). The desktop product should mirror that discipline at the UI layer:

| Experience | Question it answers |
|------------|-------------------|
| **Panel** | What are my agents doing right now? |
| **Configuration** | How is my fleet wired, and what are my preferences? |
| **Setup** | How do I bring machines online and keep them converged? |

**Do not conflate these.** The anti-pattern is bolting ansible/deploy/prereq checks into the session panel as an “Ops tab.” Setup mutates machines; Configuration edits durable intent; Panel observes and controls live sessions over HTTP.

**Shareability:** someone should be able to install the app, point it at an existing hub in Configuration, and use Panel — without ansible on their laptop. Fleet owners additionally get Setup (requires hive checkout + inventory on the control machine).

---

## Product shape

One shipped application (working name: **Hive**, crate `apps/hive-panel/` today). Three top-level modes, not a junk-drawer sidebar:

```
┌─────────────────────────────────────────┐
│  Hive                                   │
│  ┌─────────┬───────────────┬───────────┐│
│  │ Panel   │ Configuration │ Setup  †  ││
│  └─────────┴───────────────┴───────────┘│
└─────────────────────────────────────────┘
  † Setup shown when repo/ansible detected, or via Advanced → Fleet setup…
```

**Default launch:** Panel.

**Hard rule:** Panel code paths must not import or invoke SSH, ansible, or `hive-deploy`. If Panel needs SSH, that belongs in Setup or a library called only from Setup.

### Shared internals

| Layer | Used by |
|-------|---------|
| `hive-client` + `hive-protocol` | Panel, Configuration (hub probe / validate) |
| `hive-common` (TOML, hosts, migrate) | Configuration, Setup |
| Tauri shell, themes, typography | All three |
| Ansible / deploy runner | **Setup only** |

After the refactor plan lands, Panel and Configuration depend on `hive-client`; Setup additionally wraps local subprocesses (`ansible-playbook`, `hive-deploy`, optional SSH probes).

---

## 1. Panel (straightforward console)

**Purpose:** daily operator console — open many times per day, minimal chrome.

### In scope

- Fleet/session table (hub poll + SSE refresh)
- Formatted chat (Claude + Codex transcripts)
- Spawn, restart, kill, stop, attach, shell-at-session-dir
- Session labels (star), state/host/provider filters, browse themes
- Footer status, unreachable host errors from hub poll

### Backend

- HTTP/SSE to `hive-hub` only (`hive-client`)
- Reads `~/.hive/client.toml` (hub URL) — written by Configuration

### Out of scope

- Ansible converge, `hive-deploy`, linger, codex install
- Editing inventory or `hive_ref`
- SSH except where attach/shell explicitly delegate to OS (iTerm, `cc-attach` pattern) — not fleet provisioning

### Acceptance

- Panel works on a laptop with **no hive git checkout** and **no ansible**, given valid `client.toml`
- No ansible/SSH code linked from Panel React routes or their Tauri commands

---

## 2. Configuration (graphical settings)

**Purpose:** edit **durable intent** without touching live sessions. Changes here are what Setup applies on the next converge.

### In scope

**Every operator (consumer laptop):**

| Setting | Storage |
|---------|---------|
| Hub URL / name | `~/.hive/client.toml` |
| Browse theme, filter defaults, spawn defaults | app localStorage + optional `~/.hive/panel.toml` |
| CLI-facing host nicknames (read-only mirror of generated file) | `~/.hive/hosts` |

**Fleet owner (control laptop with repo):**

| Setting | Storage |
|---------|---------|
| Fleet inventory (hosts, hub role) | `~/.hive/fleet/inventory.yml` |
| Pinned rollout | `~/.hive/fleet/group_vars/all/vars.yml` → `hive_ref` |
| Checkout / install paths | same `group_vars` |
| One-shot migration seed | trigger `migrate_from_cc` via CLI or Setup step |

### UX

- Form-based editors with validation (not raw YAML as the only path — YAML remains power-user export/import)
- **Save** persists files; does **not** run ansible
- Drift badges when saved intent ≠ observed state (e.g. pinned `hive_ref` ≠ deployed rev on host) — probes via Setup health library or lightweight SSH read
- Optional **Regenerate client files** button → ansible tags `client` only (`client_hostsfile`, `client_config` on localhost) without full fleet converge

### Backend

- Read/write config TOML/YAML on disk
- Optional hub ping (`hive-client`) to validate hub URL
- No session spawn, no transcript streaming

### Out of scope

- Streaming deploy logs
- Sudo / linger / package install
- Live session table (that stays in Panel)

### Acceptance

- Hub URL change in Configuration → Panel reconnects without rerun Setup
- Offline-capable edits for laptop-only prefs (theme)
- Inventory edit + Save does not mutate remote hosts until user opens Setup and Apply

---

## 3. Setup (graphical provisioning)

**Purpose:** make machines match Configuration. **Re-enterable and idempotent** — not a one-time first-run gate.

> **Requirement:** Setup is idempotent and re-enterable; Configuration is the persistent record of choices; no step is locked after first success.

### In scope

**Wizard steps** (sidebar navigation — jump to any step, Back/Forward, no lock after completion):

| Step | Action | Backend |
|------|--------|---------|
| 1. Prerequisites | Report `tmux`, `claude`, `codex`, node, rust | SSH `command -v` with operator PATH |
| 2. Review intent | Load inventory + `hive_ref` from Configuration | read files |
| 3. Agent tools | Install Codex to `~/.local` (optional tag) | ansible role `hive_agent_tools`, tag `agents` |
| 4. Linger | Enable systemd user linger (one-time per host) | ansible tag `linger`, sudo |
| 5. Converge | Checkout, build, install units, hub | `hive-deploy` or ansible tags `checkout,build,hive,hub` |
| 6. Verify | Service active, git rev, `hive status` | SSH + hub poll |

**Per-host scope:** “Setup box-a only” — limit ansible/`hive-deploy` to one host without re-running the fleet.

**Re-entry state:** each step shows **last run** (timestamp, exit code, deployed rev) and **drift** vs Configuration (e.g. missing codex, rev mismatch). Steps offer **Run again** / **Skip** — never “Already done, cannot proceed.”

**Streaming:** deploy/converge steps stream stdout/stderr into a log pane (same event pattern as spawn output).

### Ansible: `hive_agent_tools` role (new)

Codex should not be silently installed on every day-2 deploy, but fleet hosts need a **stable PATH** for unattended spawns (`~/.local/bin/codex`, not nvm-only).

| | |
|--|--|
| **Tag** | `agents` |
| **When** | First converge, or Setup step 3, or manual `hive-deploy --tags agents` |
| **Action** | `npm install -g @openai/codex --prefix "$HOME/.local"` (idempotent; skip if binary exists unless `hive_codex_version` pin differs) |
| **Prerequisite** | Node/npm on host (report in step 1; optional future `node` role) |
| **After** | Re-run `hive-host install` so systemd `PATH` includes `~/.local/bin` (existing `hive_host` role) |
| **Claude** | check-only in `hive_prereq_check` — no fleet-wide install |

Day-2 code updates remain tagless `hive-deploy` (checkout, build, hive, hub) — no npm on every push.

### Backend

- Local `ansible-playbook`, `bin/hive-deploy`
- SSH for prereq probes and verification
- Runs on **control laptop** only — never on hub as control node

### Out of scope

- Session chat, attach, spawn dialog
- Editing theme/filter prefs (Configuration)
- Replacing ansible with Rust reimplementation of converge logic

### Acceptance

- User can open Setup months after first converge, add a host in Configuration, run steps 5–6 for that host only
- Re-running step 3 (agents) on hub is a no-op when codex already at pinned version
- Setup hidden or gated on consumer laptops without repo/ansible (menu: Advanced → Fleet setup… with explanation)

---

## Data flow

```
Configuration ──saved intent──► Setup ──ansible/hive-deploy──► Fleet hosts
       │                                                              │
       │ client.toml, hosts                                           │ poll
       └──────────────────────────► Panel ◄───────────────────────────┘
                                         (hive-hub HTTP/SSE only)
```

| Change later | Edit in | Run in |
|--------------|---------|--------|
| Hub URL | Configuration | — (Panel reconnects) |
| Add fleet host | Configuration | Setup → converge limited host + regen hosts |
| Pin new release | Configuration (`hive_ref`) | Setup → deploy |
| Enable linger | — | Setup step 4 |
| Install/update Codex | — | Setup step 3 (`agents`) |
| Theme / filters | Configuration | — |

---

## Implementation phases

Execute in order. Each phase ends with manual smoke on laptop + one fleet host.

### Phase A — Panel (existing)

Ship and stabilize current `hive-panel`: fleet table, chat, spawn, restart, attach, themes, filters, shell button. No Setup/Configuration UI yet — files edited by hand.

**Done when:** Panel uses `hive-client`; no regression on hub.

### Phase B — Configuration UI

- Hub URL editor → `~/.hive/client.toml`
- Theme/filter persistence unified with cc-console themes spec (`docs/superpowers/specs/2026-08-09-console-themes-filters-design.md` in main checkout)
- Fleet owner: inventory + `hive_ref` forms (load/save yaml with validation)
- Drift badges (read-only): deployed rev vs pin, hub reachability

**Done when:** new laptop can join fleet with hub URL only; no ansible required.

### Phase C — Setup wizard (read-only health)

- Setup mode shell + step navigator
- Steps 1, 2, 6 as read-only probes (prereqs, load Configuration, verify services)
- Per-host scope selector

**Done when:** Setup replaces ad-hoc SSH prereq checks from `ansible/README.md`.

### Phase D — Setup wizard (apply)

- Streamed `hive-deploy` / tagged ansible from steps 3–5
- Sudo password for linger (`-K`) — native prompt or terminal fallback
- Last-run / drift metadata persisted locally (`~/.hive/setup-history.json` or per-host in Configuration)

**Done when:** full first converge doable from GUI on a fresh host.

### Phase E — Ansible `hive_agent_tools`

- Role + `agents` tag wired in `site.yml`
- Setup step 3 calls `--tags agents`
- Document in `ansible/README.md`

**Done when:** box-a-class hosts get `~/.local/bin/codex` without manual npm over SSH.

### Phase F — Packaging / shareability

- Single `.app` / release artifact
- Setup gated when `$REPO_ROOT/ansible/site.yml` absent
- README: consumer vs fleet-owner paths

---

## Tauri command surface (sketch)

| Command | Mode | Notes |
|---------|------|-------|
| `panel_*` | Panel | delegate to `hive-client` |
| `config_load` / `config_save` | Configuration | TOML/YAML |
| `config_probe_hub` | Configuration | GET hub health |
| `setup_load_inventory` | Setup | parse `~/.hive/fleet/inventory.yml` |
| `setup_probe_host` | Setup | SSH batch prereqs + systemd + git rev |
| `setup_run_deploy` | Setup | spawn `hive-deploy`, emit line events |
| `setup_run_playbook` | Setup | tagged ansible, emit line events |
| `setup_history` | Setup | last run per step per host |

Panel commands must not call `setup_*`.

---

## Non-goals (this program)

- In-app ansible vault / secrets management
- Remote ansible executed **on** the hub
- Replacing `bin/hive-deploy` with duplicated Rust converge logic
- Phase 2 service user / `/var/lib/hive` ([install-locations](2026-08-13-hive-install-locations.md) § Phase 2 — spec only until later)
- Native iOS app (Tailscale web console remains separate track)
- grok / agy providers

---

## Verification checklist

**Panel**

- [ ] Launch with only `~/.hive/client.toml` — fleet table populates
- [ ] Spawn + chat + restart on codex session
- [ ] No ansible binary required on PATH

**Configuration**

- [ ] Change hub URL → Panel reconnects
- [ ] Edit inventory → Save → `~/.hive/hosts` updates after Regenerate (or next Setup converge)

**Setup**

- [ ] Complete full converge on test host from wizard
- [ ] Re-open Setup → jump to step 3 → Run again → idempotent
- [ ] Add host in Configuration → Setup limited converge → host appears in Panel
- [ ] Consumer laptop: Setup entry explains missing repo or opens docs

**Agents role**

- [ ] `hive-deploy box-a --tags agents` installs codex under `~/.local`
- [ ] `hive-deploy box-a` (default tags) does not run npm

---

## Files (expected touch)

| Area | Path |
|------|------|
| Panel UI | `apps/hive-panel/src/` |
| Tauri backend | `apps/hive-panel/src-tauri/src/` |
| Client lib | `crates/hive-client/` |
| Ansible agents role | `ansible/roles/hive_agent_tools/` |
| Site playbook | `ansible/site.yml` (role + tag) |
| Deploy wrapper | `bin/hive-deploy` (pass `--tags agents`) |
| This spec | `docs/superpowers/specs/2026-08-14-hive-desktop-app-design.md` |
