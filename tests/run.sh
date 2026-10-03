#!/usr/bin/env bash
# Builds the extension and runs tests/godot/test.gd against a live mw-server.
#   GODOT=/path/to/godot MW_SERVER=/path/to/network_manager tests/run.sh
set -euo pipefail
cd "$(dirname "$0")/.."
GODOT="${GODOT:-godot}"
MW_SERVER="${MW_SERVER:?set MW_SERVER to the mw-server network_manager binary}"
TCP_PORT="${TCP_PORT:-27878}"
UDP_PORT="${UDP_PORT:-27879}"

cargo build ${CARGO_ARGS:-}
mkdir -p tests/godot/libs
cp target/debug/libgodot_network.so tests/godot/libs/

"$MW_SERVER" --tcp-addr "127.0.0.1:$TCP_PORT" --udp-addr "127.0.0.1:$UDP_PORT" &
SERVER=$!
trap 'kill -TERM $SERVER 2>/dev/null || true' EXIT
sleep 0.5

"$GODOT" --headless --path tests/godot --import >/dev/null 2>&1 || true
LOG=$(mktemp)
STATUS=0
timeout 120 "$GODOT" --headless --path tests/godot --log-file "$LOG" -s res://test.gd -- "$TCP_PORT" "$UDP_PORT" >/dev/null 2>&1 || STATUS=$?
grep -E "^(ok|FAIL|info|ALL PASSED|FAILED|ERROR|SCRIPT ERROR)" "$LOG" || true
# Godot may exit 0 on script errors, so require the explicit success marker.
grep -q "^ALL PASSED" "$LOG" && [ "$STATUS" -eq 0 ]
