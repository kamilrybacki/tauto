export interface Expression {
  kind: string;
  value: string | number | boolean;
}

export interface Condition {
  left: Expression;
  operator: string;
  right: Expression;
}

export interface ForbiddenOperation {
  operation: string;
  args?: Expression[];
}

export interface ContractItem {
  key: string;
  entity: string;
  operation: string;
  case: string;
  requires: Condition[];
  ensures: Condition[];
  forbidden: ForbiddenOperation[];
  preserves: string[];
  assumes: string[];
  intent?: string;
  source: string | null;
  requires_count: number;
  ensures_count: number;
}

export interface ContractsResponse {
  contracts: number;
  files: number;
  items: ContractItem[];
}

export interface GraphNodeData {
  entity: string;
  operation: string;
  case: string;
  source: string | null;
  requires_count: number;
  ensures_count: number;
}

export interface RawGraphNode {
  id: string;
  data: GraphNodeData;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  kind: 'same_op' | 'conflict';
  label?: string;
}

export interface GraphResponse {
  nodes: RawGraphNode[];
  edges: GraphEdge[];
}

// ── history ───────────────────────────────────────────────────────────────────

export interface ConflictInfo {
  key_a: string;
  key_b: string;
  reason: string;
}

export interface HistoryEntry {
  id: number;
  timestamp_unix: number;
  filename: string;
  outcome: 'accepted' | 'rejected';
  contracts_count: number;
  parse_errors: number;
  conflicts: ConflictInfo[];
}

export interface HistoryResponse {
  entries: HistoryEntry[];
}

// ── check / test-gen ─────────────────────────────────────────────────────────

export interface FieldAssignment {
  field: string;
  value: string | number | boolean;
  note?: string;
}

export interface TestCase {
  id: string;
  kind: 'happy_path' | 'precondition_violation';
  description: string;
  operation: string;
  given: FieldAssignment[];
  expect_ensures?: FieldAssignment[];
  expect_forbidden_not_called?: string[];
  expect_preserved?: string[];
  should_pass: boolean;
  violated_precondition?: string;
}

export interface ContractTestSuite {
  contract: string;
  entity: string;
  operation: string;
  case_name: string;
  cases: TestCase[];
}

export interface DeadRule {
  key: string;
  field: string;
  reason: string;
}

export interface ExampleOutcome {
  case: string;
  index: number;
  status: 'pass' | 'fail' | 'underspecified';
  message: string;
}

export interface GlossaryWarning {
  contract: string;
  category: string;
  message: string;
}

export interface CheckResponse {
  compatible: boolean;
  conformant?: boolean;
  proposed_contracts: number;
  parse_errors: number;
  conflicts: ConflictInfo[];
  conformance?: ExampleOutcome[];
  dead_rules?: DeadRule[];
  glossary_warnings?: GlossaryWarning[];
  tests: {
    total_cases: number;
    proposed: ContractTestSuite[];
    regression: ContractTestSuite[];
  };
}

// ── verification report ─────────────────────────────────────────────────────────

export interface ReportObligation {
  theorem: string;
  kind: 'satisfiability' | 'guards_disjoint' | 'outcome_conflict' | 'dead_rule';
  statement: string;
  pair?: string;
  discharged: boolean;
}

export interface ReportRule {
  key: string;
  entity: string;
  operation: string;
  case: string;
  obligations: ReportObligation[];
  tests: TestCase[];
  conformance: ExampleOutcome[];
  dead_rule?: DeadRule;
  conflicts?: ConflictInfo[];
}

export interface ReportResponse {
  build_available: boolean;
  build_success: boolean;
  build_stderr: string;
  rules: ReportRule[];
  obligations_total: number;
  files: LeanFile[];
}

// ── projects ────────────────────────────────────────────────────────────────────

export interface ProjectInfo {
  slug: string;
  contracts: number;
  is_default: boolean;
}

export interface ProjectsResponse {
  projects: ProjectInfo[];
  default_project: string;
}

// ── lifecycle (state machine) ──────────────────────────────────────────────────

export interface StateTransition {
  from?: string;
  to?: string;
  contract: string;
}

export interface StateCoverage {
  entity: string;
  state_field: string;
  states: string[];
  transitions: StateTransition[];
  no_incoming: string[];
  no_outgoing: string[];
  isolated: string[];
  undeclared_states: string[];
}

// ── proofs ────────────────────────────────────────────────────────────────────

export interface LeanFile {
  path: string;
  content: string;
}

export interface ProofsResponse {
  contracts: number;
  sorry_count: number;
  files: LeanFile[];
  build_available: boolean;
  build_success: boolean;
  build_stdout: string;
  build_stderr: string;
}
