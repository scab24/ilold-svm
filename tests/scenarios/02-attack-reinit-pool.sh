#!/usr/bin/env bash
# Negative scenario: re-initializing an already-initialized pool must fail.
# Anchor's `init` constraint should reject the second call with "already in use".
set -e
. "$(dirname "$0")/_lib.sh"
NAME="02-attack-reinit-pool"
echo "## $NAME"

setup_users
expect_ok_call "first initialize_pool" "$(init_pool)"
expect_failed_call "re-init same pool rejected" "$(init_pool)"

scenario_summary "$NAME"
