#!/usr/bin/env bash
# Negative: add_rewards signed by a non-admin user must fail (WrongAdmin).
set -e
. "$(dirname "$0")/_lib.sh"
NAME="05-attack-non-admin-rewards"
echo "## $NAME"

setup_users
expect_ok_call "init" "$(init_pool)"

R=$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":100},"accounts":{"pool":"pool","admin":"bob"},"signers":["bob"]}}}')
expect_failed_call "non-admin add_rewards rejected" "$R"

scenario_summary "$NAME"
