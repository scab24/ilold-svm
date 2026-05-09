#!/usr/bin/env bash
# T-R39 regression: SaveSession + Clear + LoadSession reconstructs VM via
# replay of call_payload. The pool's total_staked must come back.
set -e
. "$(dirname "$0")/_lib.sh"
NAME="09-save-load-roundtrip"
echo "## $NAME"

setup_users
expect_ok_call "init"            "$(init_pool)"
expect_ok_call "alice stake 888" "$(stake_as alice alice_stake 888)"
expect_eq "pre-save=888" "888" "$(pool_total_staked)"

SAVED=$(post '{"contract":"staking","command":"SaveSession"}' | jq -r '.SessionSaved.json')
post '{"contract":"staking","command":"Clear"}' >/dev/null
# After Clear, the VM rewinds to pre-step-0 — pool account doesn't exist yet,
# so the State view either omits it (empty string) or jq returns "null".
post_clear=$(pool_total_staked)
case "$post_clear" in
  ""|null) echo "    PASS post-clear pool gone"; PASS=$((PASS+1)) ;;
  *)       echo "    FAIL post-clear leaks pool=$post_clear"; FAIL=$((FAIL+1)) ;;
esac

ESC=$(echo "$SAVED" | jq -Rs '.')
post "{\"contract\":\"staking\",\"command\":{\"LoadSession\":{\"json\":$ESC}}}" >/dev/null
expect_eq "post-load=888" "888" "$(pool_total_staked)"

scenario_summary "$NAME"
