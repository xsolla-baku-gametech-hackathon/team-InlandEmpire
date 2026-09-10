#!/usr/bin/env sh
# Starts server and bridge, then the game if the Tauri CLI is installed.
# Usage: scripts/demo.sh            fake taps, mock provider
#        scripts/demo.sh /dev/cu.usbserial-0001   real pad, provider from .env
set -eu
cd "$(dirname "$0")/.."

port="${1:-}"
if [ -n "$port" ]; then
  bridge_args="--port $port"
else
  bridge_args="--fake"
  export TAPPAD_PROVIDER="${TAPPAD_PROVIDER:-mock}"
fi

cargo build -q -p tappad-server -p tappad-bridge

./target/debug/tappad-server &
server=$!
# shellcheck disable=SC2086
./target/debug/tappad-bridge $bridge_args &
bridge=$!
trap 'kill $server $bridge 2>/dev/null' EXIT INT TERM

if cargo tauri --version >/dev/null 2>&1; then
  (cd crates/tappad-game && cargo tauri dev)
else
  echo "no tauri CLI, server and bridge are up, start the game by hand" >&2
  wait $server
fi
