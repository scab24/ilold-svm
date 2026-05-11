#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="09-save-load-roundtrip"
echo "## $NAME"

setup_users
expect_ok_call "init"            "$(init_pool)"
expect_ok_call "alice stake 888" "$(stake_as alice alice_stake 888)"
expect_eq "pre-save=888" "888" "$(pool_total_staked)"

SAVED=$(post '{"contract":"staking","command":{"SaveSession":{}}}' | jq -r '.SessionSaved.json')
post '{"contract":"staking","command":"Clear"}' >/dev/null
# After Clear the pool account is gone, so State omits it or jq returns "null".
post_clear=$(pool_total_staked)
case "$post_clear" in
  ""|null) echo "    PASS post-clear pool gone"; PASS=$((PASS+1)) ;;
  *)       echo "    FAIL post-clear leaks pool=$post_clear"; FAIL=$((FAIL+1)) ;;
esac

ESC=$(echo "$SAVED" | jq -Rs '.')
post "{\"contract\":\"staking\",\"command\":{\"LoadSession\":{\"json\":$ESC}}}" >/dev/null
expect_eq "post-load=888" "888" "$(pool_total_staked)"

scenario_summary "$NAME"
