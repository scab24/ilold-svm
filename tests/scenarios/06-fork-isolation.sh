#!/usr/bin/env bash
# Forking a scenario at_step=N must rewind the cloned VM to that step's
# pre-Call snapshot, leaving main untouched. Regression for T-R33 fork rewind.
set -e
. "$(dirname "$0")/_lib.sh"
NAME="06-fork-isolation"
echo "## $NAME"

setup_users
post '{"contract":"staking","command":{"UsersNew":{"name":"dave","lamports":50000000}}}' >/dev/null
post '{"contract":"staking","command":{"UsersNew":{"name":"dave_stake","lamports":2000000}}}' >/dev/null

expect_ok_call "init"             "$(init_pool)"
expect_ok_call "alice stake 500"  "$(stake_as alice alice_stake 500)"
expect_eq "main pre-fork=500" "500" "$(pool_total_staked)"

post '{"contract":"staking","command":{"Scenario":{"sub":{"Fork":{"name":"branch","at_step":1}}}}}' >/dev/null
post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"branch"}}}}}' >/dev/null
expect_eq "branch starts at 0 (rewound)" "0" "$(pool_total_staked)"

expect_ok_call "branch dave stake 777" "$(stake_as dave dave_stake 777)"
expect_eq "branch=777" "777" "$(pool_total_staked)"

post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"main"}}}}}' >/dev/null
expect_eq "main preserved=500" "500" "$(pool_total_staked)"

scenario_summary "$NAME"
