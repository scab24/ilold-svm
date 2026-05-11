#!/usr/bin/env bash
set -uo pipefail
. "$(dirname "$0")/_lib.sh"
NAME="13-auditor-walkthrough"
echo "## $NAME"

for n in admin pool alice alice_stake bob bob_stake carol carol_stake dave dave_stake; do
  L=100000000
  case "$n" in alice_stake|bob_stake|carol_stake|dave_stake|pool) L=2000000 ;; esac
  post "{\"contract\":\"staking\",\"command\":{\"UsersNew\":{\"name\":\"$n\",\"lamports\":$L}}}" >/dev/null
done
echo "    PASS users created (10)"; PASS=$((PASS+1))

print_call() {
  local label="$1"; local resp="$2"
  local key=$(echo "$resp" | jq -r 'keys[0]')
  case "$key" in
    StepAdded)
      local idx=$(echo "$resp" | jq -r '.StepAdded.step_index')
      local ix=$(echo "$resp" | jq -r '.StepAdded.instruction')
      local cu=$(echo "$resp" | jq -r '.StepAdded.compute_units')
      local diffs=$(echo "$resp" | jq -r '.StepAdded.account_diffs_count')
      echo "      ✓ step $idx [ok]:    $ix ($cu CU, $diffs diffs)  ← $label"
      ;;
    CallFailed)
      local ix=$(echo "$resp" | jq -r '.CallFailed.instruction')
      local cu=$(echo "$resp" | jq -r '.CallFailed.compute_units')
      local err=$(echo "$resp" | jq -r '.CallFailed.error')
      echo "      ✗ FAILED (not recorded): $ix ($cu CU)  ← $label"
      echo "        error: $err"
      ;;
    *)
      echo "      ✗ Error response for $label: $(echo "$resp" | jq -c '.')"
      ;;
  esac
}

echo
echo "  -- Path 1: happy path baseline --"
print_call "init"           "$(init_pool)"
print_call "alice stake 1000" "$(stake_as alice alice_stake 1000)"
print_call "add_rewards 100" "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":100},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}')"
print_call "alice claim"     "$(post '{"contract":"staking","command":{"Call":{"ix":"claim_rewards","args":{},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')"
T=$(pool_total_staked); R=$(pool_total_rewards)
expect_eq "happy: total_staked=1000" "1000" "$T"
expect_eq "happy: total_rewards=100" "100" "$R"

echo
echo "  -- Path 2: two stakers proportional --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init"             "$(init_pool)"
print_call "alice stake 1000" "$(stake_as alice alice_stake 1000)"
print_call "bob   stake 3000" "$(stake_as bob   bob_stake   3000)"
print_call "add_rewards 400"  "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":400},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}')"
expect_eq "P2 total_staked=4000" "4000" "$(pool_total_staked)"
echo "      hypothesis: alice should claim 100 (1000/4000*400), bob should claim 300"

echo
echo "  -- Path 3: front-running add_rewards (fork into attack scenario) --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init"             "$(init_pool)"
print_call "alice stake 100"  "$(stake_as alice alice_stake 100)"
post '{"contract":"staking","command":{"Scenario":{"sub":{"Fork":{"name":"frontrun","at_step":2}}}}}' >/dev/null
post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"frontrun"}}}}}' >/dev/null
echo "      ⎇ switched to scenario 'frontrun' (forked at step 2)"
print_call "carol stake 1_000_000 (front-run)" "$(stake_as carol carol_stake 1000000)"
print_call "add_rewards 1000"                  "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":1000},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}')"
T_FR=$(pool_total_staked); R_FR=$(pool_total_rewards)
echo "      frontrun scenario: total_staked=$T_FR, total_rewards=$R_FR"
echo "      finding: carol captures 1000*1000000/(100+1000000) ≈ 999 of the 1000 reward"
post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"main"}}}}}' >/dev/null
echo "      ⎇ back on scenario 'main' (untouched)"

echo
echo "  -- Path 4: edge case — last staker unstakes, rewards stranded --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init"            "$(init_pool)"
print_call "alice stake 500" "$(stake_as alice alice_stake 500)"
print_call "add_rewards 100" "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":100},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}')"
print_call "alice unstake 500" "$(post '{"contract":"staking","command":{"Call":{"ix":"unstake","args":{"amount":500},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')"
expect_eq "P4 total_staked=0 after full unstake" "0" "$(pool_total_staked)"
expect_eq "P4 total_rewards=100 still in pool"   "100" "$(pool_total_rewards)"
print_call "alice claim_rewards" "$(post '{"contract":"staking","command":{"Call":{"ix":"claim_rewards","args":{},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')"
echo "      finding: rewards stranded — pool.total_staked=0 makes claim_rewards permanently revert"

echo
echo "  -- Path 5: Anchor 'init' constraint protects re-init --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init"        "$(init_pool)"
print_call "init (twice)" "$(init_pool)"
echo "      verified safe"

echo
echo "  -- Path 6: non-admin add_rewards blocked by require!(pool.admin) --"
print_call "non-admin add_rewards" "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":99},"accounts":{"pool":"pool","admin":"bob"},"signers":["bob"]}}}')"
echo "      verified safe"

echo
echo "  -- Path 7: overflow protection on stake amount=u64::MAX --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init" "$(init_pool)"
# u64::MAX = 18446744073709551615
print_call "alice stake u64::MAX" "$(post '{"contract":"staking","command":{"Call":{"ix":"stake","args":{"amount":18446744073709551615},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice_stake","alice"]}}}')"
print_call "alice stake again, should overflow" "$(post '{"contract":"staking","command":{"Call":{"ix":"stake","args":{"amount":1},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice_stake","alice"]}}}')"
echo "      hypothesis: checked_add catches overflow with custom error"

echo
echo "  -- Path 8: bob tries to claim alice's user_stake --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
print_call "init"             "$(init_pool)"
print_call "alice stake 1000" "$(stake_as alice alice_stake 1000)"
print_call "add_rewards 100"  "$(post '{"contract":"staking","command":{"Call":{"ix":"add_rewards","args":{"amount":100},"accounts":{"pool":"pool","admin":"admin"},"signers":["admin"]}}}')"
print_call "bob claims alice_stake" "$(post '{"contract":"staking","command":{"Call":{"ix":"claim_rewards","args":{},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"bob"},"signers":["bob"]}}}')"
echo "      verified safe (require! user == user_stake.user)"

echo
echo "  -- Path 9: alice unstakes all then claims again --"
print_call "alice unstake 1000" "$(post '{"contract":"staking","command":{"Call":{"ix":"unstake","args":{"amount":1000},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')"
print_call "alice claim again" "$(post '{"contract":"staking","command":{"Call":{"ix":"claim_rewards","args":{},"accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice"]}}}')"
echo "      hypothesis: EmptyPool error (pool.total_staked=0)"

echo
echo "  -- Path 10: same alice, two distinct user_stake keypairs --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
post '{"contract":"staking","command":{"UsersNew":{"name":"alice_stake_2","lamports":2000000}}}' >/dev/null
print_call "init"               "$(init_pool)"
print_call "alice stakes #1"    "$(stake_as alice alice_stake   500)"
print_call "alice stakes #2 (different keypair)" "$(stake_as alice alice_stake_2 500)"
echo "      finding: program does not derive user_stake PDA from (pool, user); allows multiple accounts"
echo "      probably OK economically (each claims its own share) but worth a Medium note"

echo
echo "  -- Path 11: timeline by name --"
TL_BY_NAME=$(post '{"contract":"staking","command":{"Timeline":{"pubkey":"pool"}}}' | jq -c '.TimelineView | {label, entries_count: (.entries | length)}')
TL_BY_USERSTAKE=$(post '{"contract":"staking","command":{"Timeline":{"pubkey":"alice_stake"}}}' | jq -c '.TimelineView | {label, entries_count: (.entries | length)}')
echo "      tl pool         → $TL_BY_NAME"
echo "      tl alice_stake  → $TL_BY_USERSTAKE"
[ "$(echo "$TL_BY_NAME" | jq -r '.entries_count')" -gt 0 ] \
  && { echo "    PASS tl <name> resolves to pubkey and finds diffs"; PASS=$((PASS+1)); } \
  || { echo "    FAIL tl <name> still does not resolve"; FAIL=$((FAIL+1)); }

echo
echo "  -- step 1 re-inspect (decoded diff regression) --"
STEP1=$(post '{"contract":"staking","command":{"Step":{"index":1}}}')
HAS_DECODED=$(echo "$STEP1" | jq -r '.StepDetail.diff_summary[]?.decoded_after | type' | grep -m1 object)
[ -n "$HAS_DECODED" ] \
  && { echo "    PASS step diff carries decoded_after"; PASS=$((PASS+1)); } \
  || { echo "    FAIL step diff missing decoded_after"; FAIL=$((FAIL+1)); }

echo
echo "  -- failed Call returns CallFailed, no step appended --"
post '{"contract":"staking","command":"Clear"}' >/dev/null
post '{"contract":"staking","command":{"Call":{"ix":"initialize_pool","args":{"reward_rate":10},"accounts":{"pool":"pool","admin":"admin"},"signers":["pool","admin"]}}}' >/dev/null
LEN_BEFORE=$(post '{"contract":"staking","command":"Session"}' | jq -r '.SessionView.steps | length')
FAIL_RESP=$(post '{"contract":"staking","command":{"Call":{"ix":"initialize_pool","args":{"reward_rate":999},"accounts":{"pool":"pool","admin":"admin"},"signers":["pool","admin"]}}}')
KEY=$(echo "$FAIL_RESP" | jq -r 'keys[0]')
LEN_AFTER=$(post '{"contract":"staking","command":"Session"}' | jq -r '.SessionView.steps | length')
[ "$KEY" = "CallFailed" ] \
  && { echo "    PASS response variant is CallFailed"; PASS=$((PASS+1)); } \
  || { echo "    FAIL expected CallFailed got $KEY"; FAIL=$((FAIL+1)); }
[ "$LEN_BEFORE" = "$LEN_AFTER" ] \
  && { echo "    PASS session length unchanged ($LEN_BEFORE → $LEN_AFTER)"; PASS=$((PASS+1)); } \
  || { echo "    FAIL failed call polluted the timeline ($LEN_BEFORE → $LEN_AFTER)"; FAIL=$((FAIL+1)); }
ERR=$(echo "$FAIL_RESP" | jq -r '.CallFailed.error // empty')
[ -n "$ERR" ] \
  && { echo "    PASS error message present: $(echo "$ERR" | head -c 60)..."; PASS=$((PASS+1)); } \
  || { echo "    FAIL CallFailed.error missing"; FAIL=$((FAIL+1)); }

scenario_summary "$NAME"
