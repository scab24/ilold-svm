#!/usr/bin/env bash
# Scenario 14: validates RuntimeOverlay (T-R52) end-to-end via the Coverage
# command and the GET /api/program/{name}/overlay endpoint.
#
# Default fixture (tests/fixtures/solana/staking) exercises Anchor `init`,
# which CPIs into system_program (11111…) — so cpi_edges should be non-empty
# after initialize_pool / stake. To exercise a hand→lever CPI explicitly,
# rerun with `ILOLD_FIXTURE=tests/fixtures/solana/cpi tests/scenarios/run.sh`.
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
. "$DIR/_lib.sh"

echo "scenario 14: coverage overlay"

setup_users
init_pool >/dev/null
stake_as alice alice_stake 1000 >/dev/null
stake_as alice alice_stake 500  >/dev/null
stake_as bob   bob_stake   2000 >/dev/null

# Trigger a real CallFailed via Anchor's init re-use guard. Pool was
# already initialized at the top, so a second initialize_pool runs through
# add_solana_step and gets rejected by the VM. failed_calls_per_ix must
# tick — that is exactly the persistence path T-R52b restored.
fail_resp=$(init_pool)
fail_kind=$(echo "$fail_resp" | jq -r 'keys[0] // empty')
expect_eq "re-initialize_pool rejected as CallFailed" "CallFailed" "$fail_kind"

# Coverage via /api/cmd
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

# Anchor `init` (used by initialize_pool / stake's user_stake init) CPIs to
# system_program. The overlay must surface those edges.
sys_edges=$(echo "$overlay_json" \
  | jq -r '[.cpi_edges[] | select(.to_program=="11111111111111111111111111111111")] | length')
if [ "${sys_edges:-0}" -ge 1 ]; then has_sys=1; else has_sys=0; fi
expect_true "cpi_edges contains system_program" "$has_sys"

# REST endpoint roundtrip — same shape, served via GET.
rest_overlay=$(curl -sf "$BASE/api/program/staking/overlay")
rest_prog=$(echo "$rest_overlay" | jq -r '.program')
expect_eq "REST /overlay program" "staking" "$rest_prog"

rest_stake=$(echo "$rest_overlay" | jq -r '.calls_per_ix.stake // 0')
expect_eq "REST /overlay stake==3" "3" "$rest_stake"

scenario_summary "14-coverage-overlay"
