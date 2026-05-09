#!/usr/bin/env bash
# Export must aggregate findings from ALL scenarios, not just the active one.
# Regression for the bug discovered in audit round 8 (real-flow harness).
set -e
. "$(dirname "$0")/_lib.sh"
NAME="10-export-cross-scenario"
echo "## $NAME"

setup_users
expect_ok_call "init" "$(init_pool)"
expect_ok_call "stake" "$(stake_as alice alice_stake 1000)"

# Fork and record a finding in the branch.
post '{"contract":"staking","command":{"Scenario":{"sub":{"Fork":{"name":"attack","at_step":1}}}}}' >/dev/null
post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"attack"}}}}}' >/dev/null
post '{"contract":"staking","command":{"Finding":{"severity":"High","title":"branch finding","description":"only in branch"}}}' >/dev/null

# Switch back to main (no findings here) and export.
post '{"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"main"}}}}}' >/dev/null
MD=$(post '{"contract":"staking","command":"Export"}' | jq -r '.Exported.markdown')

if echo "$MD" | grep -q "branch finding"; then
  echo "    PASS export aggregates findings across scenarios"; PASS=$((PASS+1))
else
  echo "    FAIL export omitted findings from non-active scenario"; FAIL=$((FAIL+1))
fi
if echo "$MD" | grep -q "Scenario: \`attack\`"; then
  echo "    PASS export includes attack scenario section"; PASS=$((PASS+1))
else
  echo "    FAIL export missing attack section"; FAIL=$((FAIL+1))
fi

scenario_summary "$NAME"
