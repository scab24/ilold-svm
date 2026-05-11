#!/usr/bin/env bash
set -e
. "$(dirname "$0")/_lib.sh"
NAME="11-export-deliverable"
echo "## $NAME"

setup_users
expect_ok_call "init"   "$(init_pool)"
expect_ok_call "stake"  "$(stake_as alice alice_stake 1000)"

post '{"contract":"staking","command":{"Finding":{"severity":"High","title":"missing reentrancy guard","description":"unstake calls user before zeroing balance","recommendation":"Apply checks-effects-interactions"}}}' >/dev/null
post '{"contract":"staking","command":{"Finding":{"severity":"Medium","title":"no max stake","description":"any user can stake unlimited"}}}' >/dev/null

MD=$(post '{"contract":"staking","command":{"Export":{"metadata":{"auditor":"Demo Auditor","project_version":"v0.1.0","audit_date":"2026-05-09"}}}}' | jq -r '.Exported.markdown')

echo "$MD" | head -3 | sed 's/^/    | /'

echo "$MD" | grep -q "Auditor.*Demo Auditor" && echo "    PASS auditor in header" && PASS=$((PASS+1)) || { echo "    FAIL no auditor"; FAIL=$((FAIL+1)); }
echo "$MD" | grep -q "Version.*v0.1.0" && echo "    PASS version in header" && PASS=$((PASS+1)) || { echo "    FAIL no version"; FAIL=$((FAIL+1)); }
echo "$MD" | grep -q "Date.*2026-05-09" && echo "    PASS date in header" && PASS=$((PASS+1)) || { echo "    FAIL no date"; FAIL=$((FAIL+1)); }

echo "$MD" | grep -q "## Methodology" && echo "    PASS methodology section" && PASS=$((PASS+1)) || { echo "    FAIL no methodology"; FAIL=$((FAIL+1)); }
echo "$MD" | grep -q "LiteSVM" && echo "    PASS methodology mentions LiteSVM" && PASS=$((PASS+1)) || { echo "    FAIL methodology too vague"; FAIL=$((FAIL+1)); }

echo "$MD" | grep -q "## Severity Matrix" && echo "    PASS severity matrix section" && PASS=$((PASS+1)) || { echo "    FAIL no severity matrix"; FAIL=$((FAIL+1)); }
echo "$MD" | grep -qE "\| High \| 1 \|" && echo "    PASS High count = 1" && PASS=$((PASS+1)) || { echo "    FAIL High count wrong"; FAIL=$((FAIL+1)); }
echo "$MD" | grep -qE "\| Medium \| 1 \|" && echo "    PASS Medium count = 1" && PASS=$((PASS+1)) || { echo "    FAIL Medium count wrong"; FAIL=$((FAIL+1)); }

echo "$MD" | grep -q "Step.*#1" && echo "    PASS step index rendered" && PASS=$((PASS+1)) || { echo "    FAIL step index missing"; FAIL=$((FAIL+1)); }

echo "$MD" | grep -q "checks-effects-interactions" && echo "    PASS recommendation present" && PASS=$((PASS+1)) || { echo "    FAIL recommendation missing"; FAIL=$((FAIL+1)); }

echo "$MD" | grep -q "## Scenarios" && echo "    PASS scenarios section" && PASS=$((PASS+1)) || { echo "    FAIL scenarios section missing"; FAIL=$((FAIL+1)); }

scenario_summary "$NAME"
