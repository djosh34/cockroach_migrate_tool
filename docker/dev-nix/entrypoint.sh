#!/usr/local/bin/busybox sh

set -eu

busybox_bin='/usr/local/bin/busybox'
workspace_dir="${WORKSPACE_DIR:-/workspace}"
dev_home="${DEV_HOME:-/home/dev}"
host_uid="${HOST_UID:-1000}"
host_gid="${HOST_GID:-1000}"
command_string="${DEV_NIX_COMMAND:-}"

bb() {
  "$busybox_bin" "$@"
}

ensure_seed() {
  target_dir="$1"

  if [ ! -d "$target_dir" ]; then
    bb mkdir -p "$target_dir"
  fi
}

cleanup() {
  status=$?

  if ! bb chown -R "$host_uid:$host_gid" "$workspace_dir"; then
    echo "error: failed to restore workspace ownership to ${host_uid}:${host_gid}" >&2
    if [ "$status" -eq 0 ]; then
      exit 1
    fi
  fi

  exit "$status"
}

run_command() {
  export HOME="$dev_home"
  export USER=root
  export LOGNAME=root
  export SHELL=/usr/local/bin/busybox
  export PATH="/usr/local/bin:/root/.nix-profile/bin:/nix/var/nix/profiles/default/bin:/nix/var/nix/profiles/default/sbin"
  export SSL_CERT_FILE=/usr/local/share/dev-nix/ca-bundle.crt
  export GIT_SSL_CAINFO=/usr/local/share/dev-nix/ca-bundle.crt
  export NIX_SSL_CERT_FILE=/usr/local/share/dev-nix/ca-bundle.crt
  export NIX_CONFIG='experimental-features = nix-command flakes
accept-flake-config = true
build-users-group =
'

  bb mkdir -p "$HOME"
  printf '[safe]\n\tdirectory = %s\n' "$workspace_dir" > "$HOME/.gitconfig"
  cd "$workspace_dir"

  "$busybox_bin" sh -c "$command_string"
}

if [ -z "$command_string" ]; then
  echo "error: DEV_NIX_COMMAND must be set" >&2
  exit 1
fi

trap cleanup EXIT

ensure_seed /nix/store
ensure_seed /nix/var/nix/db
run_command
