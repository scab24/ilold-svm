#!/usr/bin/env bash
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PORT="${ILOLD_TEST_PORT:-8081}"
FIXTURE="${ILOLD_FIXTURE:-$ROOT/tests/fixtures/solana/staking}"
BIN="${ILOLD_BIN:-$ROOT/target/release/ilold}"
LOG="/tmp/ilold-scenarios-serve.log"

if [ ! -x "$BIN" ]; then
  echo "binary not found: $BIN — run 'cargo build --release --bin ilold -p ilold-cli'" >&2
  exit 2
fi

start_serve() {
  pkill -f "ilold serve --port $PORT" 2>/dev/null || true
  sleep 0.5
  "$BIN" serve --port "$PORT" "$FIXTURE" > "$LOG" 2>&1 &
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    sleep 0.4
    if curl -sf "http://127.0.0.1:$PORT/api/project/map" >/dev/null; then return 0; fi
  done
  echo "serve failed to come up; tail of $LOG:" >&2
  tail -20 "$LOG" >&2
  return 1
}

stop_serve() {
  pkill -f "ilold serve --port $PORT" 2>/dev/null || true
}

TOTAL_PASS=0
TOTAL_FAIL=0
SCENARIOS_OK=()
SCENARIOS_FAIL=()

DIR="$(dirname "$0")"
echo "================================================================"
echo " ilold scenario suite — fixture: $FIXTURE"
echo "================================================================"

for f in "$DIR"/[0-9][0-9]-*.sh; do
  [ -f "$f" ] || continue
  name=$(basename "$f" .sh)
  start_serve || { echo "  skipped $name (serve down)"; continue; }
  set +e
  output=$(env BASE="http://127.0.0.1:$PORT" PASS=0 FAIL=0 bash "$f" 2>&1)
  exit_code=$?
  set -e
  echo "$output"
  counts=$(echo "$output" | tail -3 | grep -oE '[0-9]+ passed / [0-9]+ failed' | head -1)
  p=$(echo "$counts" | grep -oE '^[0-9]+' | head -1)
  fc=$(echo "$counts" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+')
  TOTAL_PASS=$((TOTAL_PASS + ${p:-0}))
  TOTAL_FAIL=$((TOTAL_FAIL + ${fc:-0}))
  if [ "$exit_code" = "0" ] && [ "${fc:-0}" = "0" ]; then
    SCENARIOS_OK+=("$name")
  else
    SCENARIOS_FAIL+=("$name")
  fi
  stop_serve
done

echo
WS_PY="$DIR/ws-broadcast.py"
if [ -f "$WS_PY" ] && command -v python3 >/dev/null 2>&1 \
   && python3 -c 'import websockets' 2>/dev/null; then
  echo "## ws-broadcast (python3 + websockets)"
  start_serve || { echo "  skipped ws-broadcast (serve down)"; }
  set +e
  python3 "$WS_PY" 2>&1
  ws_exit=$?
  set -e
  stop_serve
  if [ "$ws_exit" = "0" ]; then
    SCENARIOS_OK+=("ws-broadcast")
    TOTAL_PASS=$((TOTAL_PASS + 1))
  else
    SCENARIOS_FAIL+=("ws-broadcast")
    TOTAL_FAIL=$((TOTAL_FAIL + 1))
  fi
else
  echo "## ws-broadcast — skipped (need python3 + 'pip install websockets')"
fi

echo
echo "================================================================"
echo " SUMMARY: ${#SCENARIOS_OK[@]} scenarios green, ${#SCENARIOS_FAIL[@]} scenarios red"
echo "          $TOTAL_PASS assertions passed / $TOTAL_FAIL failed"
echo "================================================================"
if [ ${#SCENARIOS_FAIL[@]} -gt 0 ]; then
  echo "FAILED scenarios:"
  for s in "${SCENARIOS_FAIL[@]}"; do echo "  - $s"; done
  exit 1
fi
exit 0
