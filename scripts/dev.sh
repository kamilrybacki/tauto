#!/usr/bin/env bash
#
# Local dev / testing harness for tauto.
#
# Build the binary + UI, run a local `tauto serve` over the example rules, and
# exercise the HTTP API and the MCP `check_rule` flow — no cluster required.
#
# Usage:
#   scripts/dev.sh build              Build the release binary and the web UI
#   scripts/dev.sh serve              Serve examples/rules (UI at http://localhost:4000)
#   scripts/dev.sh check <file.md>    Dry-run a proposed rule file via POST /api/v1/check
#   scripts/dev.sh glossary           Show the domain glossary (GET /api/v1/glossary)
#   scripts/dev.sh lifecycle          Show state-machine coverage (GET /api/v1/lifecycle)
#   scripts/dev.sh reconcile          Reconcile declared vs observed states (GET /api/v1/reconcile)
#   scripts/dev.sh mcp                Run the MCP stdio server against the local serve (interactive)
#   scripts/dev.sh mcp-call <tool> [json-args]
#                                     One-shot MCP tool call, e.g.
#                                       scripts/dev.sh mcp-call list_contracts
#                                       scripts/dev.sh mcp-call check_rule '{"contract":"..."}'
#   scripts/dev.sh demo               End-to-end demo: seed rules + check a compatible,
#                                     a conflicting, and a malformed proposal
#
# `serve` must be running in another terminal for check / mcp / mcp-call / demo.
# Env: TAUTO_PORT (default 4000), TAUTO_RULES (default examples/rules).
# TAUTO_SKIP_LEAN_CHECK=1 is set automatically so you don't need a Lean toolchain.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${TAUTO_PORT:-4000}"
BASE="http://127.0.0.1:${PORT}"
BIN="$ROOT/target/release/tauto"
RULES_DIR="${TAUTO_RULES:-$ROOT/examples/rules}"
export TAUTO_SKIP_LEAN_CHECK=1

# Prefer python3 for pretty-printing; fall back to raw cat.
pp() { if command -v python3 >/dev/null; then python3 -m json.tool; else cat; fi; }

need_bin() {
    [ -x "$BIN" ] || { echo "binary not built — run: scripts/dev.sh build" >&2; exit 1; }
}

wait_for_serve() {
    for _ in $(seq 1 40); do
        curl -sf "$BASE/api/v1/contracts" >/dev/null 2>&1 && return 0
        sleep 0.25
    done
    echo "no tauto serve reachable at $BASE — start one with: scripts/dev.sh serve" >&2
    exit 1
}

cmd_build() {
    echo "==> cargo build --release"
    ( cd "$ROOT" && cargo build --release )
    if command -v npm >/dev/null; then
        echo "==> building web UI"
        ( cd "$ROOT/ui" && npm ci --prefer-offline && npm run build )
    else
        echo "==> npm not found; skipping UI build (API still works, UI will 404)"
    fi
    echo "done."
}

cmd_serve() {
    need_bin
    echo "serving $RULES_DIR at $BASE  (UI: $BASE , API: $BASE/api/v1)"
    exec "$BIN" serve "$RULES_DIR" --port "$PORT" --ui-dist "$ROOT/ui/dist"
}

cmd_check() {
    local file="${1:-}"
    [ -n "$file" ] && [ -f "$file" ] || { echo "usage: scripts/dev.sh check <file.md>" >&2; exit 1; }
    wait_for_serve
    curl -s -X POST "$BASE/api/v1/check" \
        -H "Content-Type: text/plain" \
        --data-binary @"$file" | pp
}

cmd_glossary() {
    wait_for_serve
    curl -s "$BASE/api/v1/glossary" | pp
}

cmd_lifecycle() {
    wait_for_serve
    curl -s "$BASE/api/v1/lifecycle" | pp
}

cmd_reconcile() {
    wait_for_serve
    curl -s "$BASE/api/v1/reconcile" | pp
}

cmd_mcp() {
    need_bin
    wait_for_serve
    echo "MCP stdio server against $BASE — type JSON-RPC lines, Ctrl-D to end. Try:" >&2
    echo '  {"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' >&2
    exec "$BIN" mcp --api-url "$BASE"
}

cmd_mcp_call() {
    need_bin
    wait_for_serve
    local tool="${1:-}"
    local args="${2:-}"
    [ -n "$args" ] || args='{}'
    [ -n "$tool" ] || { echo "usage: scripts/dev.sh mcp-call <tool> [json-args]" >&2; exit 1; }
    local out
    out="$(printf '%s\n%s\n' \
        '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
        "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/call\",\"params\":{\"name\":\"$tool\",\"arguments\":$args}}" \
        | "$BIN" mcp --api-url "$BASE" 2>/dev/null)"
    if command -v python3 >/dev/null; then
        printf '%s\n' "$out" | python3 -c '
import json, sys
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        d = json.loads(line)
    except json.JSONDecodeError:
        continue
    if d.get("id") == 2:
        r = d.get("result", {})
        c = r.get("content", [{}])
        prefix = "[isError] " if r.get("isError") else ""
        print(prefix + (c[0].get("text", "") if c else json.dumps(r)))
'
    else
        printf '%s\n' "$out"
    fi
}

cmd_demo() {
    wait_for_serve
    echo "=== seeded rules ==="
    curl -s "$BASE/api/v1/contracts" | pp
    echo
    echo "=== domain glossary ==="
    curl -s "$BASE/api/v1/glossary" | pp
    for f in "$ROOT/examples/proposed/compatible-refinance.md" \
             "$ROOT/examples/proposed/conflicting-reject.md" \
             "$ROOT/examples/proposed/cross-entity-mixup.md"; do
        echo
        echo "=== check_rule: $(basename "$f") ==="
        cmd_mcp_call check_rule "$(python3 -c 'import json,sys;print(json.dumps({"contract":open(sys.argv[1]).read()}))' "$f")"
    done
    echo
    echo "=== check_rule: malformed body ==="
    cmd_mcp_call check_rule '{"contract":"just prose, no contract block"}'
}

case "${1:-}" in
    build)     cmd_build ;;
    serve)     cmd_serve ;;
    check)     shift; cmd_check "$@" ;;
    glossary)  cmd_glossary ;;
    lifecycle) cmd_lifecycle ;;
    reconcile) cmd_reconcile ;;
    mcp)       cmd_mcp ;;
    mcp-call)  shift; cmd_mcp_call "$@" ;;
    demo)      cmd_demo ;;
    *)
        grep '^#' "$0" | sed 's/^# \{0,1\}//' | sed '/^!/d'
        exit 1
        ;;
esac
