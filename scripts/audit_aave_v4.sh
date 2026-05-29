#!/usr/bin/env bash
# Audits solc cross-contract resolution against the real aave-v4 project.
# aave-v4 is external (tests/real, gitignored); clone it there and init submodules:
#   cd tests/real/aave-v4 && git submodule update --init
set -euo pipefail
cd "$(dirname "$0")/.."

PROJ=tests/real/aave-v4
PORT=8091
if [ ! -d "$PROJ" ]; then
  echo "SKIP: $PROJ not present"
  exit 0
fi
if [ ! -f "$PROJ/lib/forge-std/src/Vm.sol" ]; then
  echo "FAIL: forge-std submodule missing — run: (cd $PROJ && git submodule update --init)"
  exit 1
fi

cargo build -q -p ilold-cli
./target/debug/ilold serve "$PROJ" --port "$PORT" >/tmp/aave_audit.log 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null' EXIT
echo "compiling aave-v4 (may take ~2 min)..."
for _ in $(seq 1 300); do curl -s -o /dev/null "localhost:$PORT/api/project" && break; sleep 1; done

python3 - "$PORT" <<'PY'
import json, sys, urllib.request
base = "http://localhost:%s" % sys.argv[1]
def get(p): return json.load(urllib.request.urlopen(base + p))

ok = True

# 1. Every contract loads and its callgraph builds without error.
contracts = [c["name"] for c in get("/api/project")["contracts"]]
print("contracts loaded: %d" % len(contracts))
failed, total_ext, var_placeholders = [], 0, 0
for c in contracts:
    try:
        data = get("/api/contract/%s/callgraph" % c)
    except Exception as e:
        failed.append("%s (%s)" % (c, e)); continue
    ext = [n["data"]["contract"] for n in data["nodes"] if n["data"].get("is_external")]
    total_ext += len(ext)
    # a variable placeholder = lowercase target that isn't a known builtin
    var_placeholders += sum(1 for t in ext if t[:1].islower() and t not in ("abi",))
if failed:
    ok = False
    print("  FAIL callgraph errored for %d contracts:" % len(failed))
    for f in failed[:10]: print("     -", f)
else:
    print("  OK   all %d contracts build a callgraph" % len(contracts))
print("  external calls: %d | unresolved variable placeholders: %d" % (total_ext, var_placeholders))

# 2. Specific cross-contract cases resolve to the real target.
checks = [
    ("PositionManagerBase", "ISpoke"),       # casting ISpoke(spoke).f()
    ("TokenizationSpoke",    "IHub"),         # casting IHub(hub).f()
    ("Hub",                  "AssetLogic"),   # using-for library
    ("Spoke",                "IAaveOracle"),  # typed external call
]
for contract, expect in checks:
    targets = {n["data"]["contract"] for n in get("/api/contract/%s/callgraph" % contract)["nodes"] if n["data"].get("is_external")}
    if expect in targets:
        print("  OK   %s -> resolves %s" % (contract, expect))
    else:
        print("  FAIL %s -> %s NOT resolved" % (contract, expect)); ok = False

print("AAVE-V4 AUDIT PASSED" if ok else "AUDIT FAILED")
sys.exit(0 if ok else 1)
PY
