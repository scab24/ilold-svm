#!/usr/bin/env bash
# Happy path: init pool + 2 stakes accumulate total_staked.
set -e
. "$(dirname "$0")/_lib.sh"
NAME="01-happy-path"
echo "## $NAME"

setup_users
expect_ok_call "initialize_pool" "$(init_pool)"
expect_ok_call "alice stake 1000" "$(stake_as alice alice_stake 1000)"
expect_ok_call "bob stake 2000"   "$(stake_as bob   bob_stake   2000)"
expect_eq "total_staked=3000"  "3000" "$(pool_total_staked)"
expect_eq "session has 3 steps" "3" "$(session_steps_len)"

scenario_summary "$NAME"
