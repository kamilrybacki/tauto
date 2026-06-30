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
