# Hive — standalone project notes

This directory is the **hive** Rust fleet stack (host / hub / CLI / panel). It is
a new project inspired by an older SSH-based toolkit; it does not include or
depend on that toolkit’s `cc-*` binaries.

## Relationship to cc-* (prior toolkit)

Hive and cc-* are **separate apps**. They may coexist on the same hosts (`~/.hive/` vs
`~/.cc/`, `hive-<task>` vs `cc-<task>` tmux names) but hive does **not** list or manage cc
sessions at runtime.

Optional `--migrate` on install may copy missing files from `~/.cc/` into `~/.hive/` once;
nothing modifies cc-*.

There is no API compatibility with the old CLI or Tauri console — use cc-* tools for cc
agents, hive tools for hive agents.

## Daily driver

1. Converge fleet with Ansible (`ansible/README.md`).
2. Laptop: `~/.hive/client.toml` + generated `~/.hive/hosts`.
3. Use `hive status` and `apps/hive-panel`.
4. Stop using the old console when comfortable.

## Bootstrap

```bash
make install-ops
git push origin main

cd ansible
ansible-playbook site.yml -K --limit box-a,localhost
ansible-playbook site.yml -K

cargo install --path crates/hive-cli
hive status
```

Day-2 deploy: `hive-deploy --all` (after pushing `hive_ref` to `hive_repo_url`).

SSH remains **install and attach** only.

## Phase 2 (planned)

Fleet daemons move off the operator account to a dedicated **`hive`** system user,
**system units**, and **`/var/lib/hive/`** runtime state — no linger. See
[2026-08-13-hive-install-locations.md](2026-08-13-hive-install-locations.md) § Phase 2.
