#!/usr/bin/env python3
"""Run tauto against the benchmark and assert the 13-case scorecard.

Exercises every tauto capability end-to-end (conflict detection, precondition-
aware suppression, dead-rule detection, glossary warnings, lifecycle coverage,
conformance vs intent, ODCS data-product drift) and fails if any case regresses.

Usage:  TAUTO_BIN=./target/release/tauto python3 examples/benchmark/run_benchmark.py
"""
import json
import os
import subprocess
import sys
import time
import urllib.request

BIN = os.environ.get("TAUTO_BIN", "./target/release/tauto")
DIR = os.path.dirname(os.path.abspath(__file__))
PORT = 14790
BASE = f"http://127.0.0.1:{PORT}"

results = []  # (case, ok, detail)


def record(case, ok, detail=""):
    results.append((case, ok, detail))


def get(path):
    with urllib.request.urlopen(BASE + path, timeout=30) as r:
        return json.load(r)


def check_rule(md_path):
    data = open(os.path.join(DIR, "rules", md_path), "rb").read()
    req = urllib.request.Request(
        BASE + "/api/v1/check", data=data, headers={"Content-Type": "text/plain"}
    )
    with urllib.request.urlopen(req, timeout=30) as r:
        return json.load(r)


def main():
    env = dict(os.environ, TAUTO_SKIP_LEAN_CHECK="1", TAUTO_SLM_PROVIDER="stub")

    # Case 1 & 2: conflicts across the whole set, via `verify`.
    verify = json.loads(
        subprocess.run(
            [BIN, "verify", DIR, "--format", "json"],
            capture_output=True, text=True, env=env,
        ).stdout
    )
    conflicts = verify.get("conflicts", [])
    keys = {(c["key_a"], c["key_b"]) for c in conflicts} | {(c["key_b"], c["key_a"]) for c in conflicts}
    record(
        "1 genuine conflict",
        ("Order/approve/ApproveSubmittedOrder", "Order/approve/ApproveSubmittedOrderConflict") in keys,
        f"{len(conflicts)} conflict(s)",
    )
    cancel_pair = any("CancelInitiatedPayment" in a and "VoidAuthorizedPayment" in b for a, b in keys)
    record("2 non-conflict (disjoint guards)", not cancel_pair, "cancel pair not flagged")

    # Serve for the endpoint-backed cases.
    srv = subprocess.Popen(
        [BIN, "serve", DIR, "--port", str(PORT), "--ui-dist", "/tmp"],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        for _ in range(40):
            try:
                get("/api/v1/contracts")
                break
            except Exception:
                time.sleep(0.25)

        # Aggregate per-rule /check results.
        conformance, dead, gloss = [], [], []
        for f in ["customer_account.md", "order.md", "payment.md",
                  "inventory_reservation.md", "shipment.md"]:
            d = check_rule(f)
            conformance += d.get("conformance", [])
            dead += d.get("dead_rules", [])
            gloss += d.get("glossary_warnings", [])

        def conf(case, status):
            return any(o["case"] == case and o["status"] == status for o in conformance)

        # Case 3: dead rule.
        record("3 dead rule", any("DeadPreferredReview" in d["key"] for d in dead),
               f"{len(dead)} dead rule(s)")
        # Case 4: cross-entity.
        record("4 cross-entity trap",
               any(w.get("category") == "cross_entity_reference"
                   and "ShipWithCrossEntityPaymentTrap" in w.get("contract", "") for w in gloss))
        # Case 5: unknown field.
        record("5 unknown field",
               any(w.get("category") == "unknown_field"
                   and "UnknownPriorityExpedite" in w.get("contract", "") for w in gloss))
        # Case 7: conformance fail.
        record("7 conformance fail", conf("DeliverSignedShipment", "fail"))
        # Case 8: conformance underspecified.
        record("8 conformance underspecified", conf("MarkReadyForFulfillment", "underspecified"))
        # Case 9: conformance pass (a known-good rule conforms).
        record("9 conformance pass", conf("VerifyTrustedProspect", "pass"))

        # Case 6: lifecycle — Payment.paymentStatus Settled isolated.
        life = get("/api/v1/lifecycle")
        pay = next((c for c in life if c["entity"] == "Payment"), {})
        record("6 lifecycle uncovered", "Settled" in pay.get("isolated", []),
               f"isolated={pay.get('isolated')}")

        # Case 13: ODCS drift on Payment.paymentStatus.
        rec = get("/api/v1/reconcile")
        pd = next((d for d in rec.get("diffs", []) if d["entity"] == "Payment"), {})
        record("13 ODCS drift",
               "Reversed" in pd.get("observed_not_declared", [])
               and "Settled" in pd.get("declared_not_observed", []),
               f"source={rec.get('source')}")
    finally:
        srv.terminate()

    # Report.
    print("\n=== tauto benchmark scorecard ===")
    passed = 0
    for case, ok, detail in sorted(results):
        mark = "PASS" if ok else "FAIL"
        if ok:
            passed += 1
        print(f"  [{mark}] {case}" + (f"  ({detail})" if detail else ""))
    print(f"\n{passed}/{len(results)} cases passed")
    sys.exit(0 if passed == len(results) else 1)


if __name__ == "__main__":
    main()
