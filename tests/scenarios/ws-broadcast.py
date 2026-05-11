#!/usr/bin/env python3
"""Captura broadcasts WS durante una sesión REST y reporta lo que recibió."""
import asyncio
import json
import sys
import time
import urllib.request
import websockets

BASE = "http://127.0.0.1:8081"

def post(body):
    req = urllib.request.Request(
        f"{BASE}/api/cmd",
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=5) as r:
        return json.loads(r.read())

async def listen(received: list, ready: asyncio.Event):
    async with websockets.connect("ws://127.0.0.1:8081/ws") as ws:
        ready.set()
        try:
            while True:
                msg = await asyncio.wait_for(ws.recv(), timeout=10.0)
                received.append(json.loads(msg))
        except asyncio.TimeoutError:
            return

async def driver():
    received = []
    ready = asyncio.Event()
    listener = asyncio.create_task(listen(received, ready))
    await ready.wait()
    await asyncio.sleep(0.1)

    # Setup
    for name, lam in [("admin", 100_000_000), ("pool", 2_000_000),
                      ("alice", 50_000_000), ("alice_stake", 2_000_000)]:
        post({"contract":"staking","command":{"UsersNew":{"name":name,"lamports":lam}}})

    post({"contract":"staking","command":{"Call":{"ix":"initialize_pool","args":{"reward_rate":10},
        "accounts":{"pool":"pool","admin":"admin"},"signers":["pool","admin"]}}})
    post({"contract":"staking","command":{"Call":{"ix":"stake","args":{"amount":1234},
        "accounts":{"pool":"pool","user_stake":"alice_stake","user":"alice"},"signers":["alice_stake","alice"]}}})

    post({"contract":"staking","command":{"Scenario":{"sub":{"Fork":{"name":"branch_x","at_step":1}}}}})
    post({"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"branch_x"}}}}})
    post({"contract":"staking","command":"Back"})
    post({"contract":"staking","command":"Clear"})
    post({"contract":"staking","command":{"Scenario":{"sub":{"Switch":{"name":"main"}}}}})
    post({"contract":"staking","command":{"Scenario":{"sub":{"Delete":{"name":"branch_x"}}}}})

    await asyncio.sleep(0.5)
    listener.cancel()
    try: await listener
    except: pass
    return received

async def main():
    received = await driver()
    counts = {}
    runtime_with_data = 0
    for m in received:
        t = m.get("type", "?")
        counts[t] = counts.get(t, 0) + 1
        if t == "session_add_node" and m.get("runtime"):
            r = m["runtime"]
            if r.get("compute_units", 0) > 0 and r.get("logs_excerpt"):
                runtime_with_data += 1

    print("=== WS broadcast count by type ===")
    for k, v in sorted(counts.items()):
        print(f"  {k}: {v}")
    print()
    print(f"=== session_add_node con runtime CU+logs: {runtime_with_data} ===")
    print()

    expected_topics = {
        "solana_users_changed": 4,    # 4 UsersNew
        "session_add_node": 2,         # 2 Calls
        "session_overlay_update": 2,   # one per Call (StepAdded path)
        "scenario_forked": 1,
        "scenario_switched": 2,        # main→branch, branch→main
        "session_remove_node": 1,      # Back
        "session_clear": 1,
        "scenario_deleted": 1,
    }
    issues = 0
    for topic, expected in expected_topics.items():
        actual = counts.get(topic, 0)
        if actual != expected:
            print(f"  ISSUE {topic}: expected {expected}, got {actual}")
            issues += 1
        else:
            print(f"  OK    {topic}: {actual}")

    print()
    print(f"=== {len(received)} frames total / {issues} issues ===")
    sys.exit(0 if issues == 0 else 1)

asyncio.run(main())
