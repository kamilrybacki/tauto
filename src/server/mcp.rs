//! Model Context Protocol (MCP) server exposing tauto contracts to LLMs.
//!
//! This is a stdio JSON-RPC 2.0 server (newline-delimited messages), the
//! transport the homelab `mcp-bridge` wraps and re-exposes over HTTP/SSE for
//! cellarette. It does not read contract files directly: the MCP pod runs in
//! the `mcp-servers` namespace and cannot mount tauto's contracts PVC (which
//! lives in the `tauto` namespace). Instead every tool composes the existing
//! `tauto serve` HTTP API (`/api/v1/contracts`, `/api/v1/graph`, and
//! `/api/v1/check` for dry-run rule validation) over the cluster network,
//! configured via `TAUTO_API_URL`.

use std::io::{self, BufRead, Write};
use std::time::Duration;

use serde_json::{json, Value};

/// Protocol version echoed when the client does not request one.
const DEFAULT_PROTOCOL_VERSION: &str = "2025-06-18";

struct Ctx {
    client: reqwest::blocking::Client,
    api_url: String,
}

impl Ctx {
    /// GET `{api_url}{path}` and parse the JSON body.
    fn get_json(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.api_url.trim_end_matches('/'), path);
        let resp = self
            .client
            .get(&url)
            .send()
            .map_err(|e| format!("request to {url} failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("{url} returned HTTP {}", resp.status()));
        }
        resp.json::<Value>()
            .map_err(|e| format!("invalid JSON from {url}: {e}"))
    }

    /// POST a raw text body to `{api_url}{path}` and parse the JSON response,
    /// returning both the status and the (possibly error) body. The check
    /// endpoint returns JSON on both 200 (verdict) and 422 (unparseable), so
    /// callers inspect the status rather than treating non-2xx as opaque.
    fn post_text(&self, path: &str, body: &str) -> Result<(reqwest::StatusCode, Value), String> {
        let url = format!("{}{}", self.api_url.trim_end_matches('/'), path);
        let resp = self
            .client
            .post(&url)
            .header("Content-Type", "text/plain")
            .body(body.to_owned())
            .send()
            .map_err(|e| format!("request to {url} failed: {e}"))?;
        let status = resp.status();
        let value = resp
            .json::<Value>()
            .map_err(|e| format!("invalid JSON from {url}: {e}"))?;
        Ok((status, value))
    }

    fn contracts(&self) -> Result<Vec<Value>, String> {
        let body = self.get_json("/api/v1/contracts")?;
        Ok(body
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    fn graph(&self) -> Result<Value, String> {
        self.get_json("/api/v1/graph")
    }
}

/// Synchronous entry point. Reads JSON-RPC messages from stdin until EOF.
pub fn run_mcp(api_url: String) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;
    let ctx = Ctx { client, api_url };

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            // Malformed input without a recoverable id: ignore per JSON-RPC.
            Err(_) => continue,
        };
        if let Some(resp) = handle_message(&ctx, &req) {
            writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Dispatch a single JSON-RPC message. Returns `None` for notifications
/// (messages without an `id`), which must not be answered.
fn handle_message(ctx: &Ctx, req: &Value) -> Option<Value> {
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let id = req.get("id").cloned();

    // Notifications carry no id — never respond.
    if id.is_none() {
        return None;
    }
    let id = id.unwrap();

    match method {
        "initialize" => Some(ok(id, initialize_result(req))),
        "ping" => Some(ok(id, json!({}))),
        "tools/list" => Some(ok(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => Some(handle_tool_call(ctx, id, req)),
        other => Some(err(
            id,
            -32601,
            &format!("method not found: {other}"),
        )),
    }
}

fn initialize_result(req: &Value) -> Value {
    // Echo the client's protocol version when present; both sides then agree.
    let protocol = req
        .get("params")
        .and_then(|p| p.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": {} },
        "serverInfo": {
            "name": "tauto",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "instructions": "Browse business-logic contracts as a theorem graph. \
Contracts are keyed by entity/operation/case. Use list_contracts and \
search_contracts to find them, find_conflicts and graph_neighbors to explore \
relations, and verify_contract for a static safety summary. To validate a NEW \
proposed rule before it is saved, use check_rule: it dry-runs the rule against \
the current set (writing nothing) and returns a compatibility verdict, a \
generated test suite, and advisory glossary warnings. Call get_glossary first \
to learn the domain vocabulary (entities, their field prefixes, enums, \
operations) so a rule stays consistent, and state_coverage to see each \
entity's lifecycle (states, transitions, and unhandled/isolated states).",
    })
}

// ── tool definitions ──────────────────────────────────────────────────────────

fn tool_definitions() -> Value {
    json!([
        {
            "name": "list_contracts",
            "title": "List contracts",
            "description": "List business-logic contracts, optionally filtered by entity and/or operation. Returns each contract's key (entity/operation/case) and condition counts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity": { "type": "string", "description": "Only contracts for this entity (exact match)." },
                    "operation": { "type": "string", "description": "Only contracts for this operation (exact match)." }
                }
            }
        },
        {
            "name": "search_contracts",
            "title": "Search contracts",
            "description": "Full-text substring search across contract keys and their requires/ensures/forbidden conditions. Returns full detail for each match.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Case-insensitive substring to match." }
                },
                "required": ["query"]
            }
        },
        {
            "name": "find_conflicts",
            "title": "Find conflicts",
            "description": "Return heuristic conflict candidates between contracts (contradictory pre/postconditions). Optionally restrict to conflicts involving one contract key. Heuristic only — a Lean proof is required for confirmation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "Restrict to conflicts touching this entity/operation/case key." }
                }
            }
        },
        {
            "name": "graph_neighbors",
            "title": "Graph neighbors",
            "description": "Return contracts related to a given contract in the theorem graph: same-entity/same-operation siblings and conflict counterparts.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "The contract key (entity/operation/case) to find neighbors of." }
                },
                "required": ["key"]
            }
        },
        {
            "name": "verify_contract",
            "title": "Verify contract (static)",
            "description": "Static safety summary for one contract: its conditions plus any heuristic conflicts. NOTE: the Lean proof is NOT evaluated here (the in-cluster server runs without a Lean toolchain); use `tauto verify --lean-check` for a real proof.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "key": { "type": "string", "description": "The contract key (entity/operation/case) to summarize." }
                },
                "required": ["key"]
            }
        },
        {
            "name": "check_rule",
            "title": "Check a proposed rule",
            "description": "Dry-run a NEW proposed contract against the current rule set WITHOUT saving it. Returns a compatibility verdict, any conflicts the rule would introduce, and a generated JSON test suite. The `contract` argument is the rule in tauto DSL: one or more ```contract fenced blocks with case/entity/operation and requires/ensures/forbidden/preserves/assumes sections. A body with no parseable contract is rejected (isError) so you can correct the DSL and retry.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contract": { "type": "string", "description": "The proposed rule as markdown containing one or more ```contract blocks in tauto DSL." }
                },
                "required": ["contract"]
            }
        },
        {
            "name": "get_glossary",
            "title": "Get the domain glossary",
            "description": "Return the domain vocabulary: each entity's canonical name, `aka` instance prefixes (how its fields are addressed in rules, e.g. `loan.credit_score` for Mortgage), declared fields and enum values, operations, and prose. Consult this BEFORE translating a rule so you use the right entity and don't confuse one entity's terms (e.g. Order) with another's (e.g. Package).",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "state_coverage",
            "title": "State-machine coverage",
            "description": "For each entity state field, return the declared state domain, the transitions the current rules define over it (source state from a requires guard → target state from ensures), and coverage gaps: states with no incoming transition (candidate initial/unreachable), no outgoing (candidate terminal/dead-end), isolated (touched by no rule — a likely gap), and undeclared states used by a rule but missing from the glossary. Use it to see whether an entity's lifecycle is fully handled before adding a rule.",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

// ── tool dispatch ──────────────────────────────────────────────────────────────

fn handle_tool_call(ctx: &Ctx, id: Value, req: &Value) -> Value {
    let params = req.get("params").cloned().unwrap_or(Value::Null);
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match name {
        "list_contracts" => tool_list_contracts(ctx, &args),
        "search_contracts" => tool_search_contracts(ctx, &args),
        "find_conflicts" => tool_find_conflicts(ctx, &args),
        "graph_neighbors" => tool_graph_neighbors(ctx, &args),
        "verify_contract" => tool_verify_contract(ctx, &args),
        "check_rule" => tool_check_rule(ctx, &args),
        "get_glossary" => tool_get_glossary(ctx, &args),
        "state_coverage" => tool_state_coverage(ctx, &args),
        other => Err(format!("unknown tool: {other}")),
    };

    match result {
        Ok(text) => ok(id, tool_content(&text, false)),
        // Tool execution errors are reported in-band (isError) so the LLM can
        // see and recover from them, per the MCP spec.
        Err(msg) => ok(id, tool_content(&msg, true)),
    }
}

fn tool_content(text: &str, is_error: bool) -> Value {
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": is_error,
    })
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

// ── individual tools ───────────────────────────────────────────────────────────

fn tool_list_contracts(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let entity = args.get("entity").and_then(Value::as_str);
    let operation = args.get("operation").and_then(Value::as_str);
    let items = ctx.contracts()?;
    let filtered: Vec<Value> = items
        .into_iter()
        .filter(|c| entity.is_none_or(|e| c.get("entity").and_then(Value::as_str) == Some(e)))
        .filter(|c| operation.is_none_or(|o| c.get("operation").and_then(Value::as_str) == Some(o)))
        .map(|c| {
            json!({
                "key": c.get("key"),
                "entity": c.get("entity"),
                "operation": c.get("operation"),
                "case": c.get("case"),
                "requires_count": c.get("requires_count"),
                "ensures_count": c.get("ensures_count"),
            })
        })
        .collect();
    Ok(pretty(&json!({ "count": filtered.len(), "contracts": filtered })))
}

fn tool_search_contracts(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or("missing required argument: query")?
        .to_lowercase();
    let items = ctx.contracts()?;
    let matches: Vec<Value> = items
        .into_iter()
        .filter(|c| {
            // Match against the whole serialized contract (key + all conditions).
            c.to_string().to_lowercase().contains(&query)
        })
        .collect();
    Ok(pretty(&json!({ "count": matches.len(), "matches": matches })))
}

fn tool_find_conflicts(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let key = args.get("key").and_then(Value::as_str);
    let graph = ctx.graph()?;
    let empty = vec![];
    let edges = graph.get("edges").and_then(Value::as_array).unwrap_or(&empty);
    let conflicts: Vec<Value> = edges
        .iter()
        .filter(|e| e.get("kind").and_then(Value::as_str) == Some("conflict"))
        .filter(|e| match key {
            None => true,
            Some(k) => {
                e.get("source").and_then(Value::as_str) == Some(k)
                    || e.get("target").and_then(Value::as_str) == Some(k)
            }
        })
        .map(|e| {
            json!({
                "a": e.get("source"),
                "b": e.get("target"),
                "reason": e.get("label"),
            })
        })
        .collect();
    Ok(pretty(&json!({
        "count": conflicts.len(),
        "conflicts": conflicts,
        "note": "Heuristic conflict candidates — a Lean proof is required for confirmation.",
    })))
}

fn tool_graph_neighbors(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let key = args
        .get("key")
        .and_then(Value::as_str)
        .ok_or("missing required argument: key")?;
    let graph = ctx.graph()?;
    let empty = vec![];
    let edges = graph.get("edges").and_then(Value::as_array).unwrap_or(&empty);

    let mut same_op = Vec::new();
    let mut conflicts = Vec::new();
    for e in edges {
        let src = e.get("source").and_then(Value::as_str);
        let tgt = e.get("target").and_then(Value::as_str);
        let other = if src == Some(key) {
            tgt
        } else if tgt == Some(key) {
            src
        } else {
            continue;
        };
        let kind = e.get("kind").and_then(Value::as_str).unwrap_or("");
        let entry = json!({ "key": other, "reason": e.get("label") });
        if kind == "conflict" {
            conflicts.push(entry);
        } else {
            same_op.push(json!({ "key": other }));
        }
    }
    Ok(pretty(&json!({
        "key": key,
        "same_operation": same_op,
        "conflicts": conflicts,
    })))
}

fn tool_verify_contract(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let key = args
        .get("key")
        .and_then(Value::as_str)
        .ok_or("missing required argument: key")?;

    let items = ctx.contracts()?;
    let contract = items
        .into_iter()
        .find(|c| c.get("key").and_then(Value::as_str) == Some(key));
    let Some(contract) = contract else {
        return Err(format!("no contract found with key: {key}"));
    };

    // Reuse the conflict tool's data path for this key.
    let graph = ctx.graph()?;
    let empty = vec![];
    let edges = graph.get("edges").and_then(Value::as_array).unwrap_or(&empty);
    let conflicts: Vec<Value> = edges
        .iter()
        .filter(|e| e.get("kind").and_then(Value::as_str) == Some("conflict"))
        .filter(|e| {
            e.get("source").and_then(Value::as_str) == Some(key)
                || e.get("target").and_then(Value::as_str) == Some(key)
        })
        .map(|e| {
            let other = if e.get("source").and_then(Value::as_str) == Some(key) {
                e.get("target")
            } else {
                e.get("source")
            };
            json!({ "with": other, "reason": e.get("label") })
        })
        .collect();

    let static_status = if conflicts.is_empty() {
        "clean"
    } else {
        "conflicts_found"
    };

    Ok(pretty(&json!({
        "key": key,
        "conditions": {
            "requires": contract.get("requires"),
            "ensures": contract.get("ensures"),
            "forbidden": contract.get("forbidden"),
            "preserves": contract.get("preserves"),
            "assumes": contract.get("assumes"),
        },
        "conflicts": conflicts,
        "static_status": static_status,
        "lean_proof": "not evaluated in this environment (no Lean toolchain); run `tauto verify --lean-check` for a real proof",
    })))
}

fn tool_check_rule(ctx: &Ctx, args: &Value) -> Result<String, String> {
    let contract = args
        .get("contract")
        .and_then(Value::as_str)
        .ok_or("missing required argument: contract")?;

    let (status, body) = ctx.post_text("/api/v1/check", contract)?;

    // 422: the server understood no contract in the submission. Surface as a
    // tool error (isError) with the server's detail so the caller fixes the DSL.
    if status.as_u16() == 422 {
        let msg = body
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("no parseable contract block found");
        return Err(format!(
            "{msg} (parse_errors={}, blocks_seen={}). Check the ```contract fencing and the case/entity/operation sections, then retry.",
            body.get("parse_errors").and_then(Value::as_u64).unwrap_or(0),
            body.get("blocks_seen").and_then(Value::as_u64).unwrap_or(0),
        ));
    }
    if !status.is_success() {
        return Err(format!("check_rule failed: HTTP {status}: {}", pretty(&body)));
    }

    // Summarize: keep the verdict and conflicts verbatim (small), but compact the
    // test suites to case indices so the result stays bounded regardless of size.
    let summarize = |suites: &Value| -> Value {
        let arr = suites.as_array().cloned().unwrap_or_default();
        Value::Array(
            arr.iter()
                .map(|s| {
                    let cases: Vec<Value> = s
                        .get("cases")
                        .and_then(Value::as_array)
                        .map(|cs| {
                            cs.iter()
                                .map(|c| {
                                    json!({
                                        "id": c.get("id"),
                                        "kind": c.get("kind"),
                                        "should_pass": c.get("should_pass"),
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    json!({ "contract": s.get("contract"), "cases": cases })
                })
                .collect(),
        )
    };

    let tests = body.get("tests").cloned().unwrap_or(json!({}));
    let out = json!({
        "compatible": body.get("compatible"),
        "proposed_contracts": body.get("proposed_contracts"),
        "parse_errors": body.get("parse_errors"),
        "conflicts": body.get("conflicts"),
        "glossary_warnings": body.get("glossary_warnings"),
        "tests": {
            "total_cases": tests.get("total_cases"),
            "proposed": summarize(tests.get("proposed").unwrap_or(&json!([]))),
            "regression_suites": tests.get("regression").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        },
        "note": "Conflicts are heuristic candidates; a Lean proof confirms them. glossary_warnings are advisory vocabulary checks. Nothing was saved — this was a dry run.",
    });
    Ok(pretty(&out))
}

fn tool_get_glossary(ctx: &Ctx, _args: &Value) -> Result<String, String> {
    let glossary = ctx.get_json("/api/v1/glossary")?;
    let entities = glossary
        .get("entities")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(pretty(&json!({
        "count": entities.len(),
        "entities": entities,
        "note": "The domain vocabulary. Use the canonical entity names, their `aka` instance prefixes (how fields are addressed, e.g. loan.credit_score), declared fields/enums, and operations when authoring a rule so it stays consistent with this domain.",
    })))
}

fn tool_state_coverage(ctx: &Ctx, _args: &Value) -> Result<String, String> {
    let reports = ctx.get_json("/api/v1/lifecycle")?;
    Ok(pretty(&json!({
        "coverage": reports,
        "note": "Per entity state field: declared states, the transitions the rules define, and gaps (no_incoming = candidate initial/unreachable, no_outgoing = candidate terminal, isolated = untouched → likely a missing rule, undeclared_states = used but not in the glossary → completable from data).",
    })))
}

// ── JSON-RPC envelope helpers ──────────────────────────────────────────────────

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i32, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
