# Shared helpers for scenario scripts. Each scenario sources this and exposes
# a series of `expect_*` assertions; the runner counts them globally.
BASE="${BASE:-http://127.0.0.1:8081}"

post() {
  curl -s -X POST "$BASE/api/cmd" -H 'content-type: application/json' -d "$1"
}

# Highest-numbered pool entry — State labels look like "pool#N" where N is
# the step index of the most recent mutation.
pool_total_staked() {
  post '{"contract":"staking","command":"State"}' \
    | jq -r '[.StateView.accounts[]? | select(.label|startswith("pool"))][-1].decoded.total_staked' \
    2>/dev/null
}

pool_total_rewards() {
  post '{"contract":"staking","command":"State"}' \
    | jq -r '[.StateView.accounts[]? | select(.label|startswith("pool"))][-1].decoded.total_rewards' \
    2>/dev/null
}

session_steps_len() {
  post '{"contract":"staking","command":"Session"}' | jq -r '.SessionView.steps | length'
}

active_scenario() {
  post '{"contract":"staking","command":{"Scenario":{"sub":"List"}}}' \
    | jq -r '.ScenarioList.items[] | select(.active==true) | .name'
}

step_failed() {
  # Returns 0 if the step at $1 has an error in its runtime trace.
  local idx="$1"
  local err
  err=$(post "{\"contract\":\"staking\",\"command\":{\"Step\":{\"index\":$idx}}}" \
    | jq -r '.StepDetail.runtime_trace.error // ""')
  [ -n "$err" ]
}

call_failed_in_logs() {
  # $1 is the JSON response from a Call. Solana now distinguishes
  # CallFailed (VM rejected, no step recorded) from StepAdded (success).
  # We accept either signal.
  local key
  key=$(echo "$1" | jq -r 'keys[0] // empty')
  case "$key" in
    CallFailed) return 0 ;;
    *)
      echo "$1" | jq -r '.StepAdded.logs_excerpt[]? // empty' \
        | grep -qE 'AnchorError|failed:|panicked'
      ;;
  esac
}

setup_users() {
  # Convenience: create pool + admin + alice + alice_stake + bob + bob_stake.
  for n in admin pool alice alice_stake bob bob_stake; do
    local L=100000000
    case "$n" in alice_stake|bob_stake|pool) L=2000000 ;; esac
    post "{\"contract\":\"staking\",\"command\":{\"UsersNew\":{\"name\":\"$n\",\"lamports\":$L}}}" >/dev/null
  done
}

init_pool() {
  post '{"contract":"staking","command":{"Call":{"ix":"initialize_pool","args":{"reward_rate":10},"accounts":{"pool":"pool","admin":"admin"},"signers":["pool","admin"]}}}'
}

stake_as() {
  # stake_as <user> <user_stake> <amount>
  post "{\"contract\":\"staking\",\"command\":{\"Call\":{\"ix\":\"stake\",\"args\":{\"amount\":$3},\"accounts\":{\"pool\":\"pool\",\"user_stake\":\"$2\",\"user\":\"$1\"},\"signers\":[\"$2\",\"$1\"]}}}"
}

# ── Assertion helpers — each one increments PASS or FAIL globally ───────────
PASS=${PASS:-0}; FAIL=${FAIL:-0}
expect_eq() { # name expected actual
  if [ "$3" = "$2" ]; then echo "    PASS $1"; PASS=$((PASS+1))
  else echo "    FAIL $1 | expected:'$2' actual:'$3'"; FAIL=$((FAIL+1)); fi
}
expect_true() { # name boolean
  if [ "$2" = "1" ] || [ "$2" = "true" ]; then echo "    PASS $1"; PASS=$((PASS+1))
  else echo "    FAIL $1 | expected truthy, got '$2'"; FAIL=$((FAIL+1)); fi
}
expect_failed_call() { # name response_json
  if call_failed_in_logs "$2"; then echo "    PASS $1 (call failed as expected)"; PASS=$((PASS+1))
  else echo "    FAIL $1 — call did NOT fail; logs: $(echo "$2" | jq -r '.StepAdded.logs_excerpt[0]?' 2>/dev/null)"; FAIL=$((FAIL+1)); fi
}
expect_ok_call() { # name response_json
  if call_failed_in_logs "$2"; then echo "    FAIL $1 — call failed unexpectedly; logs: $(echo "$2" | jq -r '.StepAdded.logs_excerpt[]?' 2>/dev/null | head -3)"; FAIL=$((FAIL+1))
  else echo "    PASS $1"; PASS=$((PASS+1)); fi
}

scenario_summary() {
  echo "  ── $1: $PASS passed / $FAIL failed ──"
  return $FAIL
}
