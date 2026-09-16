#!/usr/bin/env bash
# install-hub.sh <host>
#   DEV/BOOTSTRAP SHORTCUT — fleet deploy uses ansible/ + hive-deploy.
#   Deploy hive-hub to the designated always-on host and enable systemd --user.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=install-lib.sh
source "$here/install-lib.sh"

show_help() {
  cat <<'EOF'
usage: install-hub.sh [options] <host>

DEV/BOOTSTRAP: deploy hive-hub without Ansible. For fleet converge use:
  cd ansible && ansible-playbook site.yml -K

Deploy hive-hub to one always-on Tailscale host and enable the systemd unit.

When the laptop OS/arch differs from the hub host, sources are rsync'd and
`cargo build --release -p hive-hub` runs there (cargo required).

Options:
  --bind ADDR     Listen address (default: that host's Tailscale IPv4)
  --port N        Port (default 8787)
  --dev           Allow loopback bind
  --migrate       Copy ~/.cc/hosts into ~/.hive/hosts if missing
  --hosts-file F  Seed ~/.hive/hosts from local file F (default: ./hosts.example if present)
  --no-build      Skip build; use target/release/hive-hub locally or existing binary on host
  -h, --help

Example:
  ./install-hub.sh --migrate hub
EOF
}

bind=""
port=""
dev=0
migrate=0
build=1
hosts_file=""
hub_host=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) show_help; exit 0 ;;
    --bind) bind="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    --dev) dev=1; shift ;;
    --migrate) migrate=1; shift ;;
    --hosts-file) hosts_file="$2"; shift 2 ;;
    --no-build) build=0; shift ;;
    -*)
      echo "unknown option: $1" >&2
      show_help >&2
      exit 1
      ;;
    *)
      if [[ -n "$hub_host" ]]; then
        echo "only one hub host allowed" >&2
        exit 1
      fi
      hub_host="$1"
      shift
      ;;
  esac
done

[[ -n "$hub_host" ]] || { show_help >&2; exit 1; }

echo "==> $hub_host"
ssh "$hub_host" 'mkdir -p ~/.local/bin ~/.hive ~/.config/systemd/user'
deploy_crate_binary "$hub_host" hive-hub hive-hub "$here" "$build"

if [[ -z "$hosts_file" && -f "$here/hosts.example" ]]; then
  hosts_file="$here/hosts.example"
fi
if [[ -n "$hosts_file" ]]; then
  scp -q "$hosts_file" "$hub_host:.hive/hosts.example"
  ssh "$hub_host" 'if [ ! -f ~/.hive/hosts ]; then cp ~/.hive/hosts.example ~/.hive/hosts; echo "  seeded ~/.hive/hosts from example"; fi'
fi

remote_bind="$bind"
if [[ -z "$remote_bind" ]]; then
  if [[ "$dev" -eq 1 ]]; then
    remote_bind="127.0.0.1"
  else
    remote_bind="$(ssh "$hub_host" 'tailscale ip -4 2>/dev/null | head -n1' || true)"
    [[ -n "$remote_bind" ]] || { echo "no --bind and no Tailscale IPv4 on $hub_host" >&2; exit 1; }
  fi
fi

install_cmd="hive-hub install --bind $(printf %q "$remote_bind") --bin \"\$HOME/.local/bin/hive-hub\""
[[ -n "$port" ]] && install_cmd+=" --port $(printf %q "$port")"
[[ "$dev" -eq 1 ]] && install_cmd+=" --dev"
[[ "$migrate" -eq 1 ]] && install_cmd+=" --migrate"

ssh "$hub_host" "export PATH=\"\$HOME/.local/bin:\$PATH\"; $install_cmd"
ssh "$hub_host" 'systemctl --user is-active hive-hub.service >/dev/null 2>&1 \
  && echo "  ok: hive-hub.service active" \
  || echo "  NOTE: hive-hub.service not active"'
echo "Laptop: printf 'hub = %s\\n' $(printf %q "$hub_host") > ~/.hive/client.toml"
