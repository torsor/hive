# hive

Standalone Rust project: always-on **hive-host** on each box, **hive-hub** on a
designated host, **hive** CLI + **hive-panel** as HTTP clients. Phone Safari console
at `http://<hub>/hive/` via nginx on the hub; **hive-web** is local dev/smoke only.
Config lives in `~/.hive/`. Laptop may sleep.

Not the older SSH/`cc-*` toolkit — that lives elsewhere. This tree has no
`bin/cc-*`, no `install-remote.sh`, no Python providers, no Tauri SSH console.

## Commands agents should know

```bash
ansible-playbook ansible/site.yml -K     # fleet converge (see ansible/README.md)
hive-deploy <host>… | --all            # day-2 code push
hive status | say | stop | run | …     # HTTP to hub (or local host)
hive host serve|install
hive hub serve|install
hive-web serve                         # phone SPA local dev/smoke only (prod: http://<hub>/hive/)
cd apps/hive-panel && npm run tauri dev
cd apps/hive-web && npm run dev          # phone UI dev → VITE_HUB_URL=http://<hub>:8787
cargo test --workspace
make web-check
make install-ops                       # symlink hive-deploy → ~/.local/bin
```

Dev/bootstrap only (not the fleet runbook):

```bash
./install-host.sh <host>…
./install-hub.sh <hub-host>
```

## Architecture

- **hive-host** — source of truth: `~/.hive/sessions`, tmux (`hive-<task>` only),
  Claude/Codex transcript adapters in Rust.
- **hive-hub** — polls hosts, caches `GET /v1/fleet`, proxies `/v1/hosts/{host}/…`.
- **Clients** — never SSH for fleet/chat; attach opens an external terminal.

Design: [docs/superpowers/specs/2026-08-12-hive-design.md](docs/superpowers/specs/2026-08-12-hive-design.md).  
Bootstrap notes: [docs/superpowers/specs/2026-08-12-hive-cutover.md](docs/superpowers/specs/2026-08-12-hive-cutover.md).  
Install locations: [docs/superpowers/specs/2026-08-13-hive-install-locations.md](docs/superpowers/specs/2026-08-13-hive-install-locations.md).  
Config boundary: [docs/superpowers/specs/2026-09-15-hive-config-boundary.md](docs/superpowers/specs/2026-09-15-hive-config-boundary.md). Operations: [ansible/README.md](ansible/README.md).

## Do not

- Shell out to `cc-*` or `ssh` on the status/chat path.
- Bind hub/host to `0.0.0.0` / public internet.
- Reintroduce `apps/cc-console`’s blocking `Command` pattern into hive-panel.
- Put any real hostname, tailnet address, or repo URL in the tree. Fleet config lives in `~/.hive/fleet/`; examples use `hub`, `box-a`, `box-b`.
