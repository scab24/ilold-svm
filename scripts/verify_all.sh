#!/usr/bin/env bash
# Master verification for the solc Solidity backend: build, tests, solar
# removal, cross-contract resolution (fixtures + aave-v4), folder grouping.
# Run from anywhere:  ./scripts/verify_all.sh
set -uo pipefail
cd "$(dirname "$0")/.."

fails=0
pass() { printf '  \033[32mOK\033[0m   %s\n' "$1"; }
fail() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; fails=$((fails + 1)); }

echo "=== ilold-evm full verification ==="

echo "[1/6] workspace builds"
cargo build -q --workspace 2>/dev/null && pass "workspace builds" || fail "build failed"

echo "[2/6] all tests green"
cargo test -q --workspace >/dev/null 2>&1 && pass "all tests pass" || fail "tests failed"

echo "[3/6] solar fully removed"
refs=$(grep -rn "solar" crates Cargo.toml --include='*.rs' --include='*.toml' 2>/dev/null | grep -v solc || true)
[ -z "$refs" ] && pass "no solar references" || fail "solar still referenced: $refs"

echo "[4/6] cross-contract resolution on fixtures"
./scripts/verify_evm_frontend.sh >/dev/null 2>&1 && pass "fixtures cross-contract" || fail "fixture verification"

echo "[5/6] contracts grouped by source folder"
folder_out=$(./target/debug/ilold analyze tests/fixtures/solc/cross 2>/dev/null || true)
printf '%s' "$folder_out" | grep -q "(root)" \
  && pass "folder grouping shown" || fail "no folder grouping in analyze"

echo "[6/6] aave-v4 real-project audit"
if [ -d tests/real/aave-v4 ]; then
  if ./scripts/audit_aave_v4.sh; then pass "aave-v4 audit"; else fail "aave-v4 audit"; fi
else
  echo "  SKIP  tests/real/aave-v4 not present"
fi

echo "==================================="
if [ "$fails" -eq 0 ]; then
  echo "ALL CHECKS PASSED"
else
  echo "$fails CHECK(S) FAILED"
  exit 1
fi
