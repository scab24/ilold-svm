#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="02-attack-reinit-pool"
echo "## $NAME"

setup_users
expect_ok_call "first initialize_pool" "$(init_pool)"
expect_failed_call "re-init same pool rejected" "$(init_pool)"

scenario_summary "$NAME"
