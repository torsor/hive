#!/usr/bin/env bash
# Shared helpers for install-host.sh / install-hub.sh
set -euo pipefail

# Normalize to os-arch (macos-arm64, linux-x86_64, …).
normalize_platform() {
  local os="${1:-$(uname -s)}"
  local arch="${2:-$(uname -m)}"
  case "$os" in
    Darwin) os=macos ;;
    Linux) os=linux ;;
    *) os="$(printf '%s' "$os" | tr '[:upper:]' '[:lower:]')" ;;
  esac
  case "$arch" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
  esac
  printf '%s-%s' "$os" "$arch"
}

remote_platform() {
  local host=$1
  local os arch
  os="$(ssh "$host" uname -s)"
  arch="$(ssh "$host" uname -m)"
  normalize_platform "$os" "$arch"
}

# Rsync workspace and `cargo build --release -p CRATE` on the box.
remote_cargo_build() {
  local host=$1
  local crate=$2
  local bin_name=$3
  local here=$4
  local staging=".hive-install-build-$$"

  if ! ssh "$host" 'command -v cargo >/dev/null 2>&1'; then
    echo "  ERROR: cargo not found on $host — install Rust there, or run install from a Linux machine with a matching arch" >&2
    return 1
  fi

  echo "  rsync sources → $host:~/$staging"
  ssh "$host" "rm -rf ~/$staging && mkdir -p ~/$staging"
  rsync -az \
    --exclude target \
    --exclude .git \
    --exclude '.worktrees' \
    --exclude 'apps/hive-panel/node_modules' \
    --exclude 'apps/hive-panel/src-tauri/target' \
    "$here/" "$host:~/$staging/"

  echo "  cargo build --release -p $crate (on $host)"
  ssh "$host" "cd ~/$staging && cargo build --release -p $crate"
  ssh "$host" "install -m755 ~/$staging/target/release/$bin_name ~/.local/bin/$bin_name"
  ssh "$host" "rm -rf ~/$staging"
}

# Copy a release binary to ~/.local/bin on HOST. Builds locally or remotely as needed.
deploy_crate_binary() {
  local host=$1
  local crate=$2
  local bin_name=$3
  local here=$4
  local build=$5
  local local_bin="$here/target/release/$bin_name"

  local local_plat remote_plat
  local_plat="$(normalize_platform)"
  remote_plat="$(remote_platform "$host")"

  if [[ "$local_plat" == "$remote_plat" ]]; then
    if [[ "$build" -eq 1 ]]; then
      echo "==> building $bin_name (release, $local_plat)"
      (cd "$here" && cargo build --release -p "$crate")
    fi
    [[ -x "$local_bin" ]] || {
      echo "missing $local_bin — build first or drop --no-build" >&2
      return 1
    }
    echo "  scp $local_bin → $host:~/.local/bin/$bin_name"
    scp -q "$local_bin" "$host:.local/bin/$bin_name"
  else
    echo "  platform mismatch: laptop=$local_plat host=$remote_plat"
    if [[ "$build" -eq 0 ]]; then
      if ssh "$host" "test -x ~/.local/bin/$bin_name"; then
        echo "  using existing ~/.local/bin/$bin_name on $host (--no-build)"
        return 0
      fi
      echo "  ERROR: --no-build but no $bin_name on $host (cannot scp a $local_plat binary to $remote_plat)" >&2
      return 1
    fi
    remote_cargo_build "$host" "$crate" "$bin_name" "$here"
  fi

  ssh "$host" "chmod +x ~/.local/bin/$bin_name"
}
