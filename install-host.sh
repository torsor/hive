#!/usr/bin/env bash
# install-host.sh <host> [<host> ...]
#   DEV/BOOTSTRAP SHORTCUT — fleet deploy uses ansible/ + hive-deploy.
#   Deploy hive-host to each box; builds on laptop when OS/arch matches,
#   otherwise rsyncs sources and builds with cargo on the box.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=install-lib.sh
source "$here/install-lib.sh"

show_help() {
  cat <<'EOF'
usage: install-host.sh [options] <host> [<host>...]

DEV/BOOTSTRAP: deploy hive-host without Ansible. For fleet converge use:
  cd ansible && ansible-playbook site.yml -K
  hive-deploy --all

Build and deploy hive-host to each SSH host, then enable the systemd --user unit.

When the laptop OS/arch differs from a host (e.g. macOS → Linux), sources are
rsync'd and `cargo build --release -p hive-host` runs on that host (cargo required).

Options:
  --bind ADDR     Listen address on the box (default: that host's Tailscale IPv4)
  --port N        Port (default 8788)
  --dev           Allow loopback bind (local smoke only)
  --migrate       Copy ~/.cc/hosts + sessions into ~/.hive once on the box
  --no-build      Skip build; use target/release/hive-host locally or existing binary on host
  -h, --help      Show this help

Prerequisites on each box: SSH access, tmux, systemd --user, Tailscale (for
production bind), claude and/or codex on PATH. Cross-platform deploy also needs
Rust/cargo on the box.

Examples:
  ./install-host.sh box-a box-b
  ./install-host.sh --migrate hub
EOF
}

bind=""
port=""
dev=0
migrate=0
build=1
hosts=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) show_help; exit 0 ;;
    --bind) bind="$2"; shift 2 ;;
    --port) port="$2"; shift 2 ;;
    --dev) dev=1; shift ;;
    --migrate) migrate=1; shift ;;
    --no-build) build=0; shift ;;
    --) shift; hosts+=("$@"); break ;;
    -*)
      echo "unknown option: $1" >&2
      show_help >&2
      exit 1
      ;;
    *) hosts+=("$1"); shift ;;
  esac
done

[[ ${#hosts[@]} -ge 1 ]] || { show_help >&2; exit 1; }

for host in "${hosts[@]}"; do
  echo "==> $host"
  ssh "$host" 'mkdir -p ~/.local/bin ~/.hive/sessions ~/.config/systemd/user'
  deploy_crate_binary "$host" hive-host hive-host "$here" "$build"

  remote_bind="$bind"
  if [[ -z "$remote_bind" ]]; then
    if [[ "$dev" -eq 1 ]]; then
      remote_bind="127.0.0.1"
    else
      remote_bind="$(ssh "$host" 'tailscale ip -4 2>/dev/null | head -n1' || true)"
      if [[ -z "$remote_bind" ]]; then
        echo "  ERROR: no --bind and Tailscale IPv4 not found on $host" >&2
        exit 1
      fi
    fi
  fi

  install_cmd="hive-host install --bind $(printf %q "$remote_bind") --bin \"\$HOME/.local/bin/hive-host\""
  [[ -n "$port" ]] && install_cmd+=" --port $(printf %q "$port")"
  [[ "$dev" -eq 1 ]] && install_cmd+=" --dev"
  [[ "$migrate" -eq 1 ]] && install_cmd+=" --migrate"

  ssh "$host" "export PATH=\"\$HOME/.local/bin:\$PATH\"; $install_cmd"
  ssh "$host" 'bash -s' <<'REMOTE'
run_check() { "${SHELL:-/bin/bash}" -ic "$1" 2>/dev/null; }
run_check 'case ":$PATH:" in *":$HOME/.local/bin:"*) exit 0;; esac; exit 1' \
  && echo "  ok: ~/.local/bin on PATH" \
  || echo "  NOTE: add ~/.local/bin to PATH on this box"
run_check 'command -v tmux' >/dev/null || echo "  NOTE: tmux not found — install it"
run_check 'command -v claude' >/dev/null || echo "  NOTE: claude not on PATH — set CLAUDE_BIN"
run_check 'command -v codex' >/dev/null || echo "  NOTE: codex not on PATH — set CODEX_BIN"
if systemctl --user is-active hive-host.service >/dev/null 2>&1; then
  echo "  ok: hive-host.service active"
else
  echo "  NOTE: hive-host.service not active — check journalctl --user -u hive-host"
fi
REMOTE
done
