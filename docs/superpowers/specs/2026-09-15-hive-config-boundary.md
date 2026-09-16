# Config boundary: what lives in the repo and what lives in `~/.hive`

**Date:** 2026-09-15
**Status:** approved design, awaiting implementation plan

## Goal

Nothing in this repository should describe any particular fleet. Everything a deploy
needs to know about real machines comes from files under `HIVE_HOME` (default
`~/.hive/`), and the repo ships examples showing their shape.

## What a second user needs

One hub, one or more boxes, a laptop client, all on one tailnet. They clone the repo,
copy two example files into `~/.hive/fleet/`, edit hostnames, and run the playbook.

## Design

### 1. Fleet inventory and shared vars live under `~/.hive/fleet/`

| File | Purpose |
|------|---------|
| `~/.hive/fleet/inventory.yml` | Ansible inventory: the `hive_hosts` group, a `hive_hub` child group holding the hub, `hive_hub_enabled: true` on the hub |
| `~/.hive/fleet/group_vars/all/vars.yml` | Shared vars: `hive_repo_url`, `hive_ref`, `hive_checkout_dir`, `hive_hub_host`, ports, web path |

`ansible/ansible.cfg` sets `inventory = ~/.hive/fleet/inventory.yml`. Ansible loads the
`group_vars/` directory next to an inventory file, so no role changes. `bin/hive-deploy`
and the Makefile already `cd ansible` and rely on `ansible.cfg`, so they keep working.

The repo ships `ansible/inventory.example.yml` and
`ansible/group_vars.example/all/vars.yml` with placeholder names (`hub`, `box-a`,
`box-b`) and a placeholder repo URL. The Ansible README documents the copy step.
`ansible/tests/render_hostsfile.yml` asserts against the example inventory.

### 2. Neutral defaults in code and scripts

- The phone SPA's last-resort hub fallback is same-origin (the host that served the
  SPA), not a hostname. `apps/hive-web/public/config.json` is no longer tracked; the
  existing Ansible template generates it at deploy, and a tracked `config.example.json`
  documents the shape. Local dev uses `VITE_HUB_URL`.
- The Makefile's web deploy target uses `--limit hive_hub`, so it needs no hostname.
- `bin/hive-deploy`, `install-host.sh`, `install-hub.sh`, `hosts.example`, the Ansible
  README, and `CLAUDE.md` use placeholder names in usage text and examples. The phone
  console URL is written `http://<hub>/hive/`.

### 3. Test data and fixtures

- Tailnet addresses used as test data come from `100.64.0.0/24`.
- Host names used as test data are `hub`, `box-a`, `box-b`.
- Transcript fixtures use `/home/user` as `cwd`.

### 4. Docs

Design specs describe machines by role (hub, box, laptop), never by name. Operational
notes that are specific to one installation do not live in this repo; the Ansible
README carries the generic operations section a new user needs.

### 5. License

The workspace declares `license = "MIT"` but ships no license file. A `LICENSE` with
the MIT text is added so the published repo is actually usable.

### 6. Naming

The project is `hive`. The string `hive-v2` was the working name of the rewrite and
survives only as a git branch name on the author's archive. In the tree it is removed
everywhere:

- App identity: the Tauri `productName` becomes `hive-panel` (the bundle is
  `hive-panel.app`, matching the package and crate names), the bundle identifier
  becomes `dev.torsor.hive`, and the window title, the panel's HTML title and
  `APP_LABEL`, the phone SPA's HTML title and badge, and the `hive-web` crate
  description all say `hive`.
- Docs: READMEs and specs say "hive", not "hive v2". The August spec and plan files are
  renamed to drop `-v2` from their names, and links in `CLAUDE.md` and the READMEs
  follow. Where a spec contrasts hive with the older toolkit it replaced, it says
  "the older toolkit", not "v1" or "v2".
- Deploy input: the example vars pin `hive_ref: main`.

Changing the Tauri bundle identifier makes macOS treat the panel as a new app once
(remembered window state and granted permissions reset). Nothing else depends on it.

## Testing

- `cargo test --workspace` and `make web-check` pass after the test-data rename.
- The Ansible hostsfile render test passes against the example inventory.
- `ansible-playbook site.yml --check --diff` with a real `~/.hive/fleet/` shows no drift
  on the boxes other than the intended text changes.
- A grep of the tree for any real hostname, tailnet address, repo URL, or home path
  returns nothing, and a grep for `hive-v2` or `hive v2` returns nothing outside this
  spec and its plan, which describe the removal.
