#!/usr/bin/env bash
# Negative: claim_rewards without a UserStake account must fail (account
# constraint violation: AccountOwnedByWrongProgram).
set -e
. "$(dirname "$0")/_lib.sh"
NAME="03-attack-claim-without-stake"
echo "## $NAME"

setup_users
post '{"contract":"staking","command":{"UsersNew":{"name":"carol","lamports":10000000}}}' >/dev/null
post '{"contract":"staking","command":{"UsersNew":{"name":"carol_stake","lamports":2000000}}}' >/dev/null

expect_ok_call "init" "$(init_pool)"

R=$(post '{"contract":"staking","command":{"Call":{"ix":"claim_rewards","args":{},"accounts":{"pool":"pool","user_stake":"carol_stake","user":"carol"},"signers":["carol"]}}}')
expect_failed_call "claim without stake rejected" "$R"

scenario_summary "$NAME"
