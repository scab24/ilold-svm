#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="07-back-rewinds-vm"
echo "## $NAME"

setup_users
expect_ok_call "init"            "$(init_pool)"
expect_ok_call "alice stake 1000" "$(stake_as alice alice_stake 1000)"
expect_ok_call "bob stake 2000"   "$(stake_as bob   bob_stake   2000)"
expect_eq "after 2 stakes=3000"  "3000" "$(pool_total_staked)"

post '{"contract":"staking","command":"Back"}' >/dev/null
expect_eq "after 1 back=1000"   "1000" "$(pool_total_staked)"
post '{"contract":"staking","command":"Back"}' >/dev/null
expect_eq "after 2 backs=0"     "0"    "$(pool_total_staked)"

scenario_summary "$NAME"
