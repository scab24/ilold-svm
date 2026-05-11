#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="04-attack-unstake-overflow"
echo "## $NAME"

setup_users
expect_ok_call "init"               "$(init_pool)"
expect_ok_call "alice stake 1000"   "$(stake_as alice alice_stake 1000)"

R=$(post '{"contract":"staking","command":{"Call":{"ix":"unstake","args":{"amount":99999999},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')
expect_failed_call "overflow unstake rejected" "$R"
expect_eq "state preserved at 1000" "1000" "$(pool_total_staked)"

scenario_summary "$NAME"
