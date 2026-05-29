#!/usr/bin/env bash
# Verifies the contract dependency graph (T-R63):
#   - CLI `analyze --deps` topological layers + ego views
#   - /api/project/depgraph and /api/contract/<c>/depgraph endpoints
# Runs against the deterministic solc/cross fixture. Exits non-zero on failure.
set -uo pipefail
cd "$(dirname "$0")/.."

CROSS=tests/fixtures/solc/cross
PORT=${ILOLD_DEPGRAPH_PORT:-8137}
BIN=./target/debug/ilold
PASS=0
FAIL=0
pass() { printf '  OK   %s\n' "$1"; PASS=$((PASS + 1)); }
fail() { printf '  FAIL %s\n' "$1"; FAIL=$((FAIL + 1)); }
has()  { printf '%s' "$1" | grep -qF "$2"; }

echo "== build =="
cargo build -q -p ilold-cli || { echo "build failed"; exit 2; }

echo "== CLI: analyze --deps (topological layers) =="
MAP=$($BIN analyze "$CROSS" --deps)
has "$MAP" "── layer 0 ──" && pass "layer 0 header" || fail "no layer 0 header"
has "$MAP" "── layer 1 ──" && pass "layer 1 header" || fail "no layer 1 header"
has "$MAP" "BasePool, IPool" && pass "LendingPool inherits shown" || fail "inherits not shown"

echo "== CLI: analyze --deps --contract (ego views) =="
EGO=$($BIN analyze "$CROSS" --deps --contract Vault)
has "$EGO" "Vault → depends on" && pass "ego header" || fail "no ego header"
has "$EGO" "IPool"  && pass "Vault → IPool"  || fail "missing Vault → IPool"
has "$EGO" "calls"  && has "$EGO" "holds" && pass "calls + holds tags" || fail "missing calls/holds tags"

BLAST=$($BIN analyze "$CROSS" --deps --contract IPool)
has "$BLAST" "LendingPool" && has "$BLAST" "Vault" && pass "IPool blast radius" || fail "wrong blast radius"

ERR=$($BIN analyze "$CROSS" --deps --contract Nope 2>&1 || true)
has "$ERR" "not found" && pass "missing-contract error" || fail "no error for missing contract"

echo "== API: depgraph endpoints =="
pkill -f "ilold serve --port $PORT" 2>/dev/null
sleep 0.3
$BIN serve --port "$PORT" "$CROSS" >/tmp/ilold-depgraph-serve.log 2>&1 &
SERVE_PID=$!
trap 'kill $SERVE_PID 2>/dev/null' EXIT

up=0
for _ in $(seq 1 25); do
  sleep 0.4
  curl -sf "http://127.0.0.1:$PORT/api/project/map" >/dev/null && { up=1; break; }
done
[ "$up" = 1 ] || { echo "serve failed to start; log:"; tail -10 /tmp/ilold-depgraph-serve.log; exit 2; }

PROJ=$(curl -s "http://127.0.0.1:$PORT/api/project/depgraph")
kinds=$(printf '%s' "$PROJ" | jq -c '[.edges[] | select(.data.source=="Vault" and .data.target=="IPool") | .data.kinds][0]')
[ "$kinds" = '["calls","holds"]' ] && pass "Vault→IPool kinds = calls+holds" || fail "Vault→IPool kinds: $kinds"

ipool_layer=$(printf '%s' "$PROJ" | jq '[.nodes[] | select(.data.id=="IPool") | .data.layer][0]')
vault_layer=$(printf '%s' "$PROJ" | jq '[.nodes[] | select(.data.id=="Vault") | .data.layer][0]')
if [ "$ipool_layer" != null ] && [ "$vault_layer" != null ] && [ "$ipool_layer" -lt "$vault_layer" ]; then
  pass "topological order: IPool layer $ipool_layer < Vault layer $vault_layer"
else
  fail "topological order wrong: IPool=$ipool_layer Vault=$vault_layer"
fi

EGO_JSON=$(curl -s "http://127.0.0.1:$PORT/api/contract/Vault/depgraph")
focus=$(printf '%s' "$EGO_JSON" | jq -r '[.nodes[] | select(.data.focus==true) | .data.id][0]')
[ "$focus" = "Vault" ] && pass "ego focus = Vault" || fail "ego focus: $focus"

code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/api/contract/Nope/depgraph")
[ "$code" = 404 ] && pass "404 for unknown contract" || fail "expected 404, got $code"

kill $SERVE_PID 2>/dev/null
trap - EXIT

echo "===================================="
echo "depgraph checks: $PASS passed / $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
echo "ALL CHECKS PASSED"
