# Hive fleet provisioning

`site.yml` is the runbook. Design: [install locations](../docs/superpowers/specs/2026-08-13-hive-install-locations.md), [desktop app](../docs/superpowers/specs/2026-08-14-hive-desktop-app-design.md), [config boundary](../docs/superpowers/specs/2026-09-15-hive-config-boundary.md).

Control node: your laptop with `ansible-playbook` on PATH (`uv tool install ansible-core`).

## One-time: describe your fleet

Inventory and shared vars live **outside the repo**, under `~/.hive/fleet/`:

```bash
mkdir -p ~/.hive/fleet/group_vars/all
cp inventory.example.yml ~/.hive/fleet/inventory.yml
cp group_vars.example/all/vars.yml ~/.hive/fleet/group_vars/all/vars.yml
$EDITOR ~/.hive/fleet/inventory.yml ~/.hive/fleet/group_vars/all/vars.yml
```

- `inventory.yml`: your boxes under `hive_hosts`, and the always-on hub under `hive_hub` (a child group of `hive_hosts`, so it is also a fleet host).
- `vars.yml`: `hive_repo_url` (what the boxes clone) and `hive_ref` (branch, tag, or SHA).

`ansible.cfg` points at that inventory, so run every command below from this
directory. `~/.hive/hosts` on the laptop and on the hub is GENERATED from the
inventory, while `~/.hive/client.toml` is seeded from the inventory only when
missing and may be hand-edited (e.g. to a full URL).

## Prerequisites on each box

- Reachable over SSH by the inventory hostname; Tailscale up.
- `hive_ref` pushed to `hive_repo_url` and reachable from the box.

### Ubuntu 25.10+ / 26.04: classic sudo

Those releases may route `sudo` through **sudo-rs**, which Ansible's `-K` become
cannot drive (timeout waiting for privilege escalation prompt). Classic sudo is
already installed; flip once per host **before** the first converge:

```bash
ssh -t <box> 'sudo update-alternatives --set sudo /usr/bin/sudo.ws'
```

## Converge

```bash
ansible-playbook site.yml -K                        # full fleet + laptop client files
ansible-playbook site.yml -K --limit box-a          # one host only (no laptop hosts file)
ansible-playbook site.yml -K --limit box-a,localhost
ansible-playbook site.yml -K --check --diff         # drift preview
ansible-playbook site.yml -K --tags web --limit hive_hub   # phone SPA (builds on laptop, copies to hub)
```

From the repo root: `make deploy-web` is the last line. Day-to-day code updates:

```bash
hive-deploy box-a box-b    # or: hive-deploy --all
```

`-K` (sudo password) is required on first converge when `/srv/lab` is root-owned
or when `base_packages` must install apt packages. Later runs usually need no `-K`.

Recommended first converge: one low-stakes box plus `localhost`, then the fleet,
then `hive status` from the laptop.

## After converge

On each box:

```bash
systemctl --user is-active hive-host                 # active
curl -s http://$(tailscale ip -4):8788/v1/sessions   # []
```

On the hub: `systemctl --user is-active hive-hub` and `~/.hive/hosts` lists every box.

User units need **linger** or `hive-host` stops when nobody is logged in:

```bash
sudo loginctl enable-linger $USER      # or: hive-deploy -K <box> --tags linger
```

## Procedures

- **Add a host:** one entry in `~/.hive/fleet/inventory.yml`, then `--limit newhost,localhost` (or a full run).
- **Pin a rollout:** set `hive_ref` in `~/.hive/fleet/group_vars/all/vars.yml` to a tag or SHA.
- **Move the hub:** move the host under `hive_hub` in the inventory, then rerun `--tags hub,hive,web`.
- **Change where boxes fetch code:** set `hive_repo_url`; the next converge updates each checkout's remote.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Host `(unreachable)` from hub | `hive-host` down, wrong poll URL, or tailnet path | `systemctl --user status hive-host`; hub `~/.hive/hosts` uses Tailscale IPs; `curl http://<box-ip>:8788/v1/sessions` from the hub |
| Host flaps reachable/unreachable | linger off, so the user manager stops | `sudo loginctl enable-linger $USER`; restart `hive-host` |
| Ansible `-K` hangs | sudo-rs | `update-alternatives --set sudo /usr/bin/sudo.ws` |
| `hive status` shows stale sessions | old binary on box | `hive-deploy <host>` |

## Tags

| Tag | Roles |
|-----|-------|
| `packages` | base_packages |
| `toolchain` | rust_toolchain |
| `checkout` | hive_checkout |
| `build` | hive_build |
| `hive` | hive_host (not linger — use `-K --tags linger` once per host) |
| `hub` | hive_hub |
| `web` | hive_web (phone SPA + nginx on hub) |
| `linger` | `loginctl enable-linger` (needs sudo; first converge only) |
| `clienthosts` | client_hostsfile |
| `clientconfig` | client_config |
| `summary` | post-converge git rev |

## Tests

```bash
ansible-playbook -i inventory.example.yml tests/render_hostsfile.yml
```
