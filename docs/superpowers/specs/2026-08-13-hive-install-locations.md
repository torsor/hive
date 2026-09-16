# Design: hive install locations

**Date:** 2026-08-13  
**Status:** approved (Ansible implemented in `ansible/`)  
**Scope:** where fleet binaries, checkouts, and runtime state live on laptop, agent boxes, and hub  
**See also:** [2026-08-12-hive-cutover.md](2026-08-12-hive-cutover.md), workshop-fleet `spec/site-roles-and-provisioning.md` (D-T1, D-T2)

## Intent

Hive mixes four different “install” concepts in today’s shell scripts (`install-host.sh`
copies a laptop-built binary to `~/.local/bin`). Fleet deploy should follow the
workshop-fleet pattern: **versioned checkout under `/srv/lab/`**, **runtime state under
`~/.hive/`**, **inventory-generated host lists**, and **CLI install verbs** for systemd —
without interfering with the parallel **cc-*** stack on the same machines (`~/.cc/` is
separate; hive does not read it at runtime).

## Four kinds of location (do not conflate)

| Kind | What it is | older toolkit | hive |
|------|------------|---------------|---------|
| **Runtime state** | Sessions, bind config, fleet lists | `~/.cc/` on box; `~/.cc/hosts` on laptop | `HIVE_HOME` → `~/.hive/` |
| **Operator CLI** | Commands run by hand | `~/.local/bin/cc-*` (copied scripts) | laptop: `hive` CLI, `hive-panel` (local build) |
| **Fleet daemon binary** | Long-running `hive-host` / `hive-hub` | *(none)* | built on Linux; stable path under `/srv/lab/hive/` |
| **Versioned source tree** | What Ansible converges | N/A (no checkout) | `/srv/lab/hive` @ pinned `hive_ref` |

## Paths by role

### Agent box (each fleet host)

```
/srv/lab/hive/                         # git checkout @ {{ hive_ref }}
/srv/lab/hive/bin/hive-host            # stable binary (copied after cargo build)
~/.hive/                               # HIVE_HOME — runtime (parallel to ~/.cc/)
  sessions/<task>.meta                 # (+ optional .labels)
  host.toml                            # bind/port (optional)
~/.local/bin/hive-host                 # optional symlink for manual debugging
~/.config/systemd/user/hive-host.service
```

- **Sessions stay in `~/.hive/sessions`**, not under `/srv/lab/`. The daemon runs as a
  **systemd user** unit for the operator; tmux sessions and metadata share that uid.
- **Build on the box:** `cargo build --release -p hive-host` inside `/srv/lab/hive` after
  git pull. Never scp a Mac-built binary to Linux.
- **Install verb:** `hive-host install --bin /srv/lab/hive/bin/hive-host --home ~/.hive …`
  Re-run when the embedded binary path changes.

### Hub host (one always-on host)

Same checkout/binary pattern, plus:

```
~/.hive/hosts                         # GENERATED from Ansible inventory (D-T2)
~/.hive/hub.toml
/srv/lab/hive/bin/hive-hub
~/.config/systemd/user/hive-hub.service
```

The hub **hosts file is authoritative on the hub only** and is templated from inventory —
not hand-edited on the hub, not copied from `~/.cc/hosts` unless an explicit one-shot seed
task is used.

### Laptop (control seat)

```
~/…/hive/                             # dev checkout (build CLI + panel here)
~/.cargo/bin/hive                     # or ~/.local/bin — operator CLI
~/.hive/client.toml                   # hub = "hub" (or full URL)
~/.hive/hosts                         # GENERATED from inventory (D-T2)
```

The laptop does **not** need `/srv/lab/hive`. Dev and fleet converge are separate trees on
hosts that are also dev sites (see below).

## Separate from cc-* (same machines, different app)

Hive and cc-* are **independent apps**. They may run on the same fleet hosts but do not
share runtime visibility:

| | older toolkit | hive |
|---|------|---------|
| Config root | `~/.cc/` | `~/.hive/` |
| tmux prefix | `cc-<task>` | `hive-<task>` |
| Fleet console | `cc-status`, cc-console | `hive status`, hive-panel |
| Remote tools | `~/.local/bin/cc-*` | `/srv/lab/hive/bin/hive-host` (+ optional symlink) |
| Host list (laptop) | `~/.cc/hosts` (manual) | `~/.hive/hosts` (generated) |

`hive-host` lists only `~/.hive/sessions`. It never reads `~/.cc/` during normal operation.

No install step should modify or remove `~/.cc/`. Optional `--migrate` may **copy into
`~/.hive/` only when a target file is missing** (one-shot import, not ongoing sync).

## Dev site vs fleet host on the same machine

A machine like box-b may be both a **development site** and a **converged fleet host**:

- `~/lab/…/hive` — hacking, `cargo test`, panel dev  
- `/srv/lab/hive` — what systemd runs after Ansible converge  

Do not assume these are the same directory. `hive-deploy` updates `/srv/lab/hive` only.

## Ansible (planned)

Mirrors workshop-fleet: `~/.hive/fleet/inventory.yml` is authoritative; `site.yml` is the
runbook; roles call idempotent CLI verbs rather than reimplementing domain logic.

Shared vars (`~/.hive/fleet/group_vars/all/vars.yml`):

```yaml
hive_repo_url: …
hive_ref: main
hive_checkout_dir: /srv/lab/hive
hive_bin_dir: "{{ hive_checkout_dir }}/bin"
hive_home: "{{ ansible_facts.env.HOME }}/.hive"
```

Role sequence (sketch):

1. `hive_checkout` — git at `hive_ref`  
2. `hive_build` — `cargo build --release -p hive-host` (and hub on hub host)  
3. `hive_host` / `hive_hub` — copy to `hive_bin_dir`, run `hive-* install`  
4. `client_hostsfile` — localhost play: template `~/.hive/hosts` from inventory  

Day-to-day code updates: `hive-deploy <host>` → ansible `--tags checkout,hive` (name TBD).

`install-host.sh` / `install-hub.sh` remain **dev/bootstrap shortcuts**, not the fleet
runbook.

## Decisions

| Question | Decision |
|----------|----------|
| Runtime home on boxes? | `~/.hive` (`HIVE_HOME`), user-scoped |
| Fleet checkout on boxes? | `/srv/lab/hive` @ pinned ref |
| Stable daemon binary path? | `/srv/lab/hive/bin/hive-{host,hub}` |
| Where to build Rust? | On the Linux host (in checkout), not on the Mac laptop |
| Laptop host list? | Generated from inventory into `~/.hive/hosts` |
| Hub host list? | Generated from same inventory into hub `~/.hive/hosts` |
| cc-* interaction? | Separate apps; hive never reads `~/.cc` at runtime; optional `--migrate` only |

## Non-goals

- Cross-compiling from macOS in install scripts  
- Putting session metadata under `/srv/lab/hive/` (couples runtime to deploy rollback)  
- Hand-edited host lists on provisioned clients or hub  
- Ansible managing cc-* install paths  

## Hazards

1. **Moving the checkout dir** without re-running `hive-* install` leaves systemd
   `ExecStart` pointing at a stale absolute path (workshop-fleet learned this with
   `/srv/lab/workshop-fleet`).
2. **Scp from laptop** produces `Exec format error` on Linux — build on the target arch.
3. **Inventory `--limit`** on a single new host excludes the localhost
   `client_hostsfile` play; use `--limit newhost,localhost` or a full run when adding a
   host (D-T2, same as workshop-fleet).
4. **`loginctl enable-linger`** on phase-1 user units — without linger, `hive-host` stops
   when no login session exists (seen on box-b). Phase 2 removes this dependency.

## Phase 2: dedicated service user (Option A)

**Status:** planned (not implemented). Phase 1 stays on **systemd user units + linger**
for the operator account.

### Why move

Phase 1 ties daemon lifecycle to the operator’s logind session (even with linger). Long
term, fleet daemons should:

- start at boot without login or linger
- run with least privilege (not the operator’s full home dir)
- survive independently of personal shell/tmux habits

Hive is a good candidate: on-box `hive-host` does not SSH out — it only manages local
tmux + filesystem. Auth is Tailscale membership at the HTTP bind, not the unix uid.

### Target shape

| Piece | Phase 1 (now) | Phase 2 |
|-------|---------------|---------|
| Unix account | operator (your login) | dedicated **`hive`** system user |
| Unit type | `systemd --user` | **system** unit (`/etc/systemd/system/`) |
| Runtime home | `~/.hive/` | **`/var/lib/hive/`** (or `/srv/lab/hive/state/`) |
| tmux owner | operator | **`hive`** |
| Session metadata | `~/.hive/sessions/` | `/var/lib/hive/sessions/` |
| Hub hosts file | `~/.hive/hosts` on hub | `/var/lib/hive/hosts` |
| Linger | required on agent/hub boxes | **not used** |
| Operator laptop | `~/.hive/client.toml` | unchanged (HTTP client only) |

Checkout and binaries stay at `/srv/lab/hive/` — only **runtime state** and **service
identity** move off the operator home.

### Install layout (sketch)

```
/srv/lab/hive/bin/hive-host              # unchanged — versioned binary
/var/lib/hive/                           # HIVE_HOME on fleet hosts
  sessions/
  host.toml
  hub.toml                               # hub host only
  hosts                                  # GENERATED (hub only)
/etc/systemd/system/hive-host.service    # User=hive, system unit
```

`hive-host install` gains a **`--system`** (or separate `install-system`) path that writes
the system unit, creates `/var/lib/hive` owned by `hive`, and does not touch
`~/.config/systemd/user/`.

### Product / UX tradeoffs

- **Attach (`hive attach`)** — today: SSH as operator → attach to your tmux. Phase 2
  options (pick one at implementation time):
  - **A1 (recommended):** attach runs `sudo -u hive tmux attach …` over SSH (operator
    needs passwordless sudo for that one command, or a setuid helper).
  - **A2:** deprecate terminal attach on fleet; panel/chat-only for remote control.
  - **A3:** `hive attach` opens a `machinectl shell`-style login as `hive` (heavier).

- **Manual debugging** — operators use `sudo -u hive …` or `systemctl status hive-host`,
  not `systemctl --user`.

- **Dev on box** — `~/lab/…/hive` dev checkout remains separate from `/srv/lab/hive`;
  local `cargo run` smoke tests can still use `--dev --bind 127.0.0.1` as the operator.

### Ansible changes (sketch)

New or extended role tasks:

1. Create system user `hive` (no login shell, or `/usr/sbin/nologin`).
2. Ensure `/var/lib/hive/{sessions,…}` owned by `hive`.
3. `hive-host install --system --home /var/lib/hive --bin …` (and hub equivalent).
4. `systemctl enable --now hive-host` (system scope).
5. Remove / stop phase-1 user units if present (one-time migration task).
6. Drop `loginctl enable-linger` from converge once system units are live.

Inventory may add `hive_service_user: hive` and `hive_home: /var/lib/hive` vars; override
only for dev hosts that intentionally stay on phase 1.

### Migration from phase 1

One-shot per host (manual or `--migrate-system` verb):

1. Stop `systemctl --user stop hive-host hive-hub`.
2. Copy `~/.hive/sessions/` → `/var/lib/hive/sessions/` (preserve metadata; tmux names
   stay `hive-<task>` — **re-home tmux sessions** or accept that running agents must be
   restarted under the `hive` user's tmux server).
3. Install system units; start services.
4. Disable user units + linger no longer required.

Running tmux sessions **cannot** be uid-migrated live — plan for restart/resume, same as
any uid change.

### Non-goals for phase 2

- Changing Tailscale auth model (still bind to tailnet IP only).
- Merging back with cc-* or reading `~/.cc/`.
- Running hive daemons as root.

### References

- workshop-fleet service-identity design (user+linger vs system unit vs dedicated service
  user) — same tradeoff arc; hive skips the SSH-key-at-boot problem that pushed workshop
  toward user units in step 1.
