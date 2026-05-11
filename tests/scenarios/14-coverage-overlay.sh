#!/usr/bin/env bash
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
. "$DIR/_lib.sh"

echo "scenario 14: coverage overlay"

setup_users
init_pool >/dev/null
stake_as alice alice_stake 1000 >/dev/null
stake_as alice alice_stake 500  >/dev/null
stake_as bob   bob_stake   2000 >/dev/null

# Re-init triggers CallFailed so failed_calls_per_ix ticks.
fail_resp=$(init_pool)
fail_kind=$(echo "$fail_resp" | jq -r 'keys[0] // empty')
expect_eq "re-initialize_pool rejected as CallFailed" "CallFailed" "$fail_kind"

cov_response=$(post '{"contract":"staking","command":"Coverage"}')
overlay_json=$(echo "$cov_response" | jq '.Coverage.overlay')

prog=$(echo "$overlay_json" | jq -r '.program')
expect_eq "coverage.program is staking" "staking" "$prog"

scn=$(echo "$overlay_json" | jq -r '.scenario')
if [ -n "$scn" ] && [ "$scn" != "null" ]; then has_scn=1; else has_scn=0; fi
expect_true "coverage.scenario is set" "$has_scn"

stake_calls=$(echo "$overlay_json" | jq -r '.calls_per_ix.stake // 0')
expect_eq "stake called three times" "3" "$stake_calls"

init_calls=$(echo "$overlay_json" | jq -r '.calls_per_ix.initialize_pool // 0')
expect_eq "initialize_pool called once" "1" "$init_calls"

cu_samples=$(echo "$overlay_json" | jq -r '.cu_stats_per_ix.stake.samples // 0')
expect_eq "cu_stats.stake.samples == 3" "3" "$cu_samples"

failed_init=$(echo "$overlay_json" | jq -r '.failed_per_ix.initialize_pool // 0')
expect_eq "failed_per_ix.initialize_pool == 1" "1" "$failed_init"

sys_edges=$(echo "$overlay_json" \
  | jq -r '[.cpi_edges[] | select(.to_program=="11111111111111111111111111111111")] | length')
if [ "${sys_edges:-0}" -ge 1 ]; then has_sys=1; else has_sys=0; fi
expect_true "cpi_edges contains system_program" "$has_sys"

rest_overlay=$(curl -sf "$BASE/api/program/staking/overlay")
rest_prog=$(echo "$rest_overlay" | jq -r '.program')
expect_eq "REST /overlay program" "staking" "$rest_prog"

rest_stake=$(echo "$rest_overlay" | jq -r '.calls_per_ix.stake // 0')
expect_eq "REST /overlay stake==3" "3" "$rest_stake"

scenario_summary "14-coverage-overlay"
