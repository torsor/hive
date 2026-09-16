# hive

Rust fleet console for Claude/Codex agents running in tmux on always-on boxes.

Laptop may sleep. Boxes and one designated **hub** stay up. Clients talk HTTP
to the hub; the hub polls each box's **hive-host**. SSH is only for install and
attach.

## Pieces

| Binary | Role |
|--------|------|
| `hive-host` | Per-box daemon (`~/.hive/sessions`, tmux, transcripts) — port **8788** |
| `hive-hub` | Always-on aggregator + action proxy — port **8787** |
| `hive` | CLI (`status`, `say`, `stop`, `run`, …) |
| `hive-panel` | Tauri GUI — hub HTTP/SSE only |
| `hive-web` | Phone Safari console, served by nginx on the hub |

Shared types: `crates/hive-protocol`. Specs: [design](docs/superpowers/specs/2026-08-12-hive-design.md), [install locations](docs/superpowers/specs/2026-08-13-hive-install-locations.md), [config boundary](docs/superpowers/specs/2026-09-15-hive-config-boundary.md).

## Layout

```
crates/hive-protocol|common|client|host|hub|web|cli
apps/hive-panel/       # desktop GUI (Tauri)
apps/hive-web/         # phone SPA
ansible/               # fleet provisioning (authoritative deploy path)
bin/hive-deploy        # day-2 code push wrapper
install-host.sh        # dev/bootstrap shortcut only
install-hub.sh         # dev/bootstrap shortcut only
hosts.example          # seed for ~/.hive/hosts when not using Ansible
tests/fixtures/        # transcript golden files
```

## Bootstrap (fleet)

Production deploy uses Ansible. Your fleet's inventory lives in `~/.hive/fleet/`,
never in this repo. See [ansible/README.md](ansible/README.md) for the copy step,
converge order, and operations.

```bash
# control node: install deploy helper
make install-ops

# describe your fleet (see ansible/README.md), then converge
cd ansible
ansible-playbook site.yml -K --limit box-a,localhost
ansible-playbook site.yml -K

# laptop CLI
cargo install --path crates/hive-cli
hive status

# day-2 code updates
hive-deploy --all

# GUI
cd apps/hive-panel && npm install && npm run tauri dev
```

Dev/bootstrap without Ansible (single host, quick iteration):

```bash
./install-host.sh box-a
./install-hub.sh hub
```

Local smoke (no Tailscale):

```bash
cargo run -p hive-host -- serve --dev --bind 127.0.0.1
cargo run -p hive-hub  -- serve --dev --bind 127.0.0.1
printf 'hub = "http://127.0.0.1:8787"\n' > ~/.hive/client.toml
cargo run -p hive-cli -- status
```

## Config (`HIVE_HOME`, default `~/.hive/`)

| Path | Where |
|------|--------|
| `fleet/inventory.yml`, `fleet/group_vars/all/vars.yml` | laptop (Ansible control node) |
| `sessions/<task>.meta` (+ `.labels`) | each box |
| `host.toml` | each box (`bind`, `port`) |
| `hosts` | hub (GENERATED from the inventory) |
| `hub.toml` | hub |
| `client.toml` | laptop (`hub = "…"`, seeded from the inventory when missing) |

Bind production addresses to Tailscale CGNAT (`100.64.0.0/10`) or use `--dev` + loopback.

## Trust

Tailscale membership is the auth boundary. No tokens.

## Attach

External terminal only: hub returns `ssh -t host tmux attach …`. No embedded terminal in the panel.

## License

MIT, see [LICENSE](LICENSE).
