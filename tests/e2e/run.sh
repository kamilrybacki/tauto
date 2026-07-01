#!/usr/bin/env bash
# E2E test: contract lifecycle and conflict rejection.
#
# Starts a fresh tauto server instance with an empty temp directory, uploads
# four fixture contracts via HTTP, and verifies parse results, graph state,
# and that a conflicting rule is rejected (HTTP 409) and rolled back.
#
# No src code is modified. The tauto binary is used as a black box.
#
# Environment variables:
#   TAUTO_BIN   path to the tauto binary      (default: ./target/release/tauto)
#   TAUTO_PORT  port for the temporary server  (default: 14001)
set -euo pipefail

BINARY="${TAUTO_BIN:-./target/release/tauto}"
PORT="${TAUTO_PORT:-14001}"
BASE="http://localhost:${PORT}"
FIXTURES="$(cd "$(dirname "$0")/fixtures" && pwd)"
RESP_FILE="/tmp/tauto-e2e-resp-$$.json"

# ── Setup ─────────────────────────────────────────────────────────────────────
CONTRACTS_DIR=$(mktemp -d)
UI_DIR=$(mktemp -d)
printf '<html><body>tauto e2e</body></html>' > "$UI_DIR/index.html"

SERVER_PID=""
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    rm -rf "$CONTRACTS_DIR" "$UI_DIR" "$RESP_FILE"
}
trap cleanup EXIT INT TERM

"$BINARY" serve "$CONTRACTS_DIR" --port "$PORT" --ui-dist "$UI_DIR" \
    >/tmp/tauto-e2e-server-$$.log 2>&1 &
SERVER_PID=$!

echo "Waiting for server (PID=$SERVER_PID)..."
for i in $(seq 1 40); do
    curl -s "$BASE/api/v1/contracts" >/dev/null 2>&1 && break
    sleep 0.25
    [ "$i" -lt 40 ] || { echo "ERROR: server did not start within 10s" >&2; exit 1; }
done
echo "Server ready at $BASE"
echo "Contracts dir : $CONTRACTS_DIR"

# ── Helpers ───────────────────────────────────────────────────────────────────
PASS_COUNT=0
FAIL() { echo "FAIL: $*" >&2; exit 1; }
PASS() { echo "  ✓  $*"; PASS_COUNT=$((PASS_COUNT + 1)); }

upload() {
    # upload <fixture-filename>
    # Writes response body to $RESP_FILE; prints HTTP status code on stdout.
    curl -s \
        -X POST \
        -F "file=@${FIXTURES}/$1" \
        -o "$RESP_FILE" \
        -w "%{http_code}" \
        "$BASE/api/v1/contracts/upload"
}

contract_count() {
    curl -sf "$BASE/api/v1/contracts" | jq -r .contracts
}

conflict_edge_count() {
    curl -sf "$BASE/api/v1/graph" \
        | jq '[.edges[] | select(.kind=="conflict")] | length'
}

verify_dir() {
    local outdir
    outdir=$(mktemp -d)
    "$BINARY" verify "$CONTRACTS_DIR" --output "$outdir" 2>&1 || true
    rm -rf "$outdir"
}

assert_no_conflicts_in_verify() {
    local out
    out=$(verify_dir)
    echo "$out" | grep -q "Conflict candidates" \
        && FAIL "tauto verify reported unexpected Conflict candidates" \
        || true
}

# ── Case 1: First rule ─────────────────────────────────────────────────────────
echo
echo "═══ Case 1: ShipPaidOrder — first rule ═══"

code=$(upload "case1-ship-paid-order.md")
[ "$code" = "200" ] || FAIL "upload expected HTTP 200, got $code; body: $(cat "$RESP_FILE")"
[ "$(jq -r .contracts "$RESP_FILE")" = "1" ] \
    || FAIL "upload response: expected contracts=1, got $(jq -r .contracts "$RESP_FILE")"
[ "$(jq -r .parse_errors "$RESP_FILE")" = "0" ] \
    || FAIL "upload response: expected parse_errors=0"
PASS "upload → 200, 1 contract, 0 parse errors"

[ "$(contract_count)" = "1" ] || FAIL "GET /contracts: expected 1"
PASS "GET /contracts = 1"

[ "$(conflict_edge_count)" = "0" ] || FAIL "GET /graph: expected 0 conflict edges"
PASS "GET /graph: 0 conflict edges"

assert_no_conflicts_in_verify
PASS "tauto verify: no Conflict candidates (Lean workspace written)"

# ── Case 2: Second compatible rule ────────────────────────────────────────────
echo
echo "═══ Case 2: RefundShippedOrder — compatible (different operation) ═══"

code=$(upload "case2-refund-shipped-order.md")
[ "$code" = "200" ] || FAIL "upload expected HTTP 200, got $code; body: $(cat "$RESP_FILE")"
[ "$(jq -r .contracts "$RESP_FILE")" = "1" ] \
    || FAIL "upload response: expected contracts=1"
PASS "upload → 200, 1 contract, 0 parse errors"

[ "$(contract_count)" = "2" ] || FAIL "GET /contracts: expected 2"
PASS "GET /contracts = 2"

[ "$(conflict_edge_count)" = "0" ] || FAIL "GET /graph: expected 0 conflict edges"
PASS "GET /graph: 0 conflict edges"

assert_no_conflicts_in_verify
PASS "tauto verify: no Conflict candidates (rules are logically compatible)"

# ── Case 3: Third compatible rule ─────────────────────────────────────────────
echo
echo "═══ Case 3: CancelPendingOrder — compatible (different operation) ═══"

code=$(upload "case3-cancel-pending-order.md")
[ "$code" = "200" ] || FAIL "upload expected HTTP 200, got $code; body: $(cat "$RESP_FILE")"
[ "$(jq -r .contracts "$RESP_FILE")" = "1" ] \
    || FAIL "upload response: expected contracts=1"
PASS "upload → 200, 1 contract, 0 parse errors"

[ "$(contract_count)" = "3" ] || FAIL "GET /contracts: expected 3"
PASS "GET /contracts = 3"

[ "$(conflict_edge_count)" = "0" ] || FAIL "GET /graph: expected 0 conflict edges"
PASS "GET /graph: 0 conflict edges"

assert_no_conflicts_in_verify
PASS "tauto verify: no Conflict candidates (all 3 rules logically compatible)"

# ── Case 4: Conflicting rule — must be rejected ────────────────────────────────
echo
echo "═══ Case 4: RejectShipWhenPaid — conflicts with Case 1, must be rejected ═══"

code=$(upload "case4-conflict-ship-order.md")
[ "$code" = "409" ] \
    || FAIL "upload expected HTTP 409 Conflict, got $code; body: $(cat "$RESP_FILE")"
conflict_count=$(jq '.conflicts | length' "$RESP_FILE" 2>/dev/null || echo 0)
[ "$conflict_count" -ge 1 ] 2>/dev/null \
    || FAIL "409 response missing conflicts array; got: $(cat "$RESP_FILE")"
PASS "upload → 409 Conflict, $conflict_count conflict(s) in response body"

[ "$(contract_count)" = "3" ] \
    || FAIL "GET /contracts: expected 3 (conflicting file must be rolled back)"
PASS "GET /contracts = 3 (conflicting file rolled back)"

[ "$(conflict_edge_count)" = "0" ] \
    || FAIL "GET /graph: expected 0 conflict edges after rollback"
PASS "GET /graph: 0 conflict edges (store is clean)"

assert_no_conflicts_in_verify
PASS "tauto verify: no Conflict candidates (invalid rule was never persisted)"

# ── Summary ───────────────────────────────────────────────────────────────────
echo
echo "All ${PASS_COUNT} checks passed."
