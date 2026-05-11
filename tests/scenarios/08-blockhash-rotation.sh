#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="08-blockhash-rotation"
echo "## $NAME"

setup_users
expect_ok_call "init" "$(init_pool)"

START=$(date +%s%N)
for _ in $(seq 1 50); do
  post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":1},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}' >/dev/null
done
END=$(date +%s%N)
ELAPSED_MS=$(((END - START) / 1000000))
expect_eq "total_rewards=50 after 50 calls" "50" "$(pool_total_rewards)"
echo "    info: 50 calls in ${ELAPSED_MS}ms (avg $((ELAPSED_MS / 50))ms/call)"

scenario_summary "$NAME"
