#!/usr/bin/env bash
# SDD-03 verification: save --with-keypairs persists user keypairs and load
# rehydrates them so pubkeys (and any PDA derived from them) are identical
# across save/load.
set -e
. "$(dirname "$0")/_lib.sh"
NAME="12-save-load-deterministic"
echo "## $NAME"

setup_users
expect_ok_call "init"            "$(init_pool)"
expect_ok_call "alice stake 500" "$(stake_as alice alice_stake 500)"

# Capture user pubkeys before save.
PRE_USERS=$(post '{"contract":"staking","command":"Users"}' \
  | jq -c '.UserList.users | sort_by(.name) | map({name, pubkey})')

# Save with keypairs.
SAVED=$(post '{"contract":"staking","command":{"SaveSession":{"with_keypairs":true}}}' \
  | jq -r '.SessionSaved.json')
echo "$SAVED" | grep -q '"keypairs_present": true' \
  && { echo "    PASS keypairs_present header set"; PASS=$((PASS+1)); } \
  || { echo "    FAIL keypairs_present missing"; FAIL=$((FAIL+1)); }

# Wipe state. Clear rewinds the VM and drops snapshots; this also forces
# the load path to actually rehydrate rather than reuse memory.
post '{"contract":"staking","command":"Clear"}' >/dev/null

# Load with the persisted bundle.
ESC=$(echo "$SAVED" | jq -Rs '.')
LOAD=$(post "{\"contract\":\"staking\",\"command\":{\"LoadSession\":{\"json\":$ESC}}}")
LOADED=$(echo "$LOAD" | jq -r '.SessionLoaded.steps | length')
[ "$LOADED" = "2" ] \
  && { echo "    PASS load reports 2 steps"; PASS=$((PASS+1)); } \
  || { echo "    FAIL expected 2 steps got $LOADED"; FAIL=$((FAIL+1)); }

# Capture user pubkeys after load.
POST_USERS=$(post '{"contract":"staking","command":"Users"}' \
  | jq -c '.UserList.users | sort_by(.name) | map({name, pubkey})')

if [ "$PRE_USERS" = "$POST_USERS" ]; then
  echo "    PASS user pubkeys identical across save/load (deterministic)"; PASS=$((PASS+1))
else
  echo "    FAIL user pubkeys diverged"
  echo "      pre:  $PRE_USERS"
  echo "      post: $POST_USERS"
  FAIL=$((FAIL+1))
fi

# State is also reproduced (also covered by scenario 09; double-check the
# --with-keypairs path doesn't regress the existing total).
TOTAL=$(pool_total_staked)
[ "$TOTAL" = "500" ] \
  && { echo "    PASS pool.total_staked replayed=500"; PASS=$((PASS+1)); } \
  || { echo "    FAIL pool.total_staked is '$TOTAL' (expected 500)"; FAIL=$((FAIL+1)); }

# Negative path: a save WITHOUT keypairs must not embed the bundle.
PLAIN=$(post '{"contract":"staking","command":{"SaveSession":{"with_keypairs":false}}}' \
  | jq -r '.SessionSaved.json')
echo "$PLAIN" | grep -q '"keypairs_present": false' \
  && { echo "    PASS default save reports keypairs_present=false"; PASS=$((PASS+1)); } \
  || { echo "    FAIL default save header wrong"; FAIL=$((FAIL+1)); }
echo "$PLAIN" | grep -q '"keypairs": null' \
  && { echo "    PASS default save has no keypairs payload"; PASS=$((PASS+1)); } \
  || { echo "    FAIL default save leaks keypairs"; FAIL=$((FAIL+1)); }

scenario_summary "$NAME"
