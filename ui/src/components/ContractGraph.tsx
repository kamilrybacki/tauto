import { useMemo } from 'react';
import type { GraphResponse, GraphEdge, ContractItem, Condition } from '../api/types';

/* Flow view: the workflow as a vertical stage stepper — no canvas. Operations
   are grouped into topological stages computed from *gates*: a rule on Order
   guarding `paymentStatusSnapshot == Captured` is gated by the Payment
   operation whose ensures sets `paymentStatus = Captured`. Mobile-first,
   research-backed (linear/stepper over node-link for non-editing workflows). */

const leftPath = (c: Condition): string => String(c.left.value);
const shortField = (p: string): string => (p.includes('.') ? p.slice(p.indexOf('.') + 1) : p);

interface Gate {
  from: string; // op id `entity/operation`
  to: string;
  value: string;
}

function deriveGates(contracts: ContractItem[]): Gate[] {
  const producers = new Map<string, Set<string>>();
  for (const c of contracts) {
    for (const e of c.ensures) {
      const key = `${shortField(leftPath(e))}=${String(e.right.value)}`;
      if (!producers.has(key)) producers.set(key, new Set());
      producers.get(key)!.add(`${c.entity}/${c.operation}`);
    }
  }
  const gates = new Map<string, Gate>();
  for (const c of contracts) {
    const toId = `${c.entity}/${c.operation}`;
    for (const g of c.requires) {
      if (g.operator !== '==') continue;
      const field = shortField(leftPath(g));
      const mirrored = field.endsWith('Snapshot') ? field.slice(0, -'Snapshot'.length) : field;
      if (mirrored === field) continue;
      const key = `${mirrored}=${String(g.right.value)}`;
      for (const fromId of producers.get(key) ?? []) {
        if (fromId === toId) continue;
        const id = `${fromId}->${toId}`;
        if (!gates.has(id)) gates.set(id, { from: fromId, to: toId, value: String(g.right.value) });
      }
    }
  }
  return [...gates.values()];
}

interface ContractGraphProps {
  graph: GraphResponse;
  contracts: ContractItem[];
  slug: string;
  selected: string | null;
  onSelect: (ruleKey: string) => void;
}

export default function ContractGraph({ graph, contracts, slug, selected, onSelect }: ContractGraphProps) {
  const gates = useMemo(() => deriveGates(contracts), [contracts]);

  const conflictOps = useMemo(() => {
    const s = new Set<string>();
    const byKey = new Map(contracts.map((c) => [c.key, c]));
    graph.edges
      .filter((e: GraphEdge) => e.kind === 'conflict')
      .forEach((e) => {
        const a = byKey.get(e.source);
        if (a) s.add(`${a.entity}/${a.operation}`);
        const b = byKey.get(e.target);
        if (b) s.add(`${b.entity}/${b.operation}`);
      });
    return s;
  }, [graph.edges, contracts]);

  const stages = useMemo(() => {
    const ops = new Map<string, { entity: string; operation: string; rules: ContractItem[] }>();
    for (const c of contracts) {
      const id = `${c.entity}/${c.operation}`;
      if (!ops.has(id)) ops.set(id, { entity: c.entity, operation: c.operation, rules: [] });
      ops.get(id)!.rules.push(c);
    }
    // Topological layer = longest gate-path from any source.
    const layer = new Map<string, number>();
    ops.forEach((_, id) => layer.set(id, 0));
    for (let pass = 0; pass < ops.size; pass++) {
      let changed = false;
      for (const g of gates) {
        const want = (layer.get(g.from) ?? 0) + 1;
        if ((layer.get(g.to) ?? 0) < want) {
          layer.set(g.to, want);
          changed = true;
        }
      }
      if (!changed) break;
    }
    const inbound = new Map<string, Gate[]>();
    gates.forEach((g) => {
      if (!inbound.has(g.to)) inbound.set(g.to, []);
      inbound.get(g.to)!.push(g);
    });
    const maxLayer = Math.max(0, ...[...layer.values()]);
    const out: { ops: { id: string; entity: string; operation: string; rules: ContractItem[]; gates: Gate[] }[] }[] = [];
    for (let l = 0; l <= maxLayer; l++) {
      const list = [...ops.entries()]
        .filter(([id]) => layer.get(id) === l)
        .map(([id, op]) => ({ id, ...op, gates: inbound.get(id) ?? [] }))
        .sort((a, b) => a.entity.localeCompare(b.entity) || a.operation.localeCompare(b.operation));
      if (list.length) out.push({ ops: list });
    }
    return out;
  }, [contracts, gates]);

  const selectedOp = useMemo(() => {
    if (!selected) return null;
    const c = contracts.find((x) => x.key === selected);
    return c ? `${c.entity}/${c.operation}` : null;
  }, [selected, contracts]);

  const open = (id: string) => {
    const first = contracts.find((c) => `${c.entity}/${c.operation}` === id);
    if (first) onSelect(first.key);
  };

  return (
    <div>
      <p className="figcaption" style={{ margin: '0 0 14px' }}>
        {contracts.length} rules across {stages.reduce((n, s) => n + s.ops.length, 0)} operations in{' '}
        <em>{slug || 'default'}</em>, ordered by their gates ({gates.length} derived from snapshot guards).
      </p>
      <ol className="flow">
        {stages.map((stage, i) => (
          <li className="flow-stage" key={i}>
            <div className="flow-marker">
              <span className="flow-num">{i + 1}</span>
              {i < stages.length - 1 && <span className="flow-line" />}
            </div>
            <div className="flow-ops">
              {stage.ops.map((op) => (
                <button
                  key={op.id}
                  className={`flow-op${conflictOps.has(op.id) ? ' conflict' : ''}${selectedOp === op.id ? ' selected' : ''}`}
                  onClick={() => open(op.id)}
                  title="Open in the decision tables"
                >
                  <span className="flow-op-head">
                    <span className="flow-op-name">{op.operation}</span>
                    <span className="flow-op-entity">{op.entity}</span>
                    {conflictOps.has(op.id) && <span className="pill conflict">⊥</span>}
                    <span className="flow-op-rules">
                      {op.rules.length} rule{op.rules.length !== 1 ? 's' : ''}
                    </span>
                  </span>
                  {op.gates.length > 0 && (
                    <span className="flow-needs">
                      {op.gates.map((g) => (
                        <span className="need" key={`${g.from}-${g.value}`}>
                          needs <code>{g.value}</code> ← {g.from.replace('/', '·')}
                        </span>
                      ))}
                    </span>
                  )}
                </button>
              ))}
            </div>
          </li>
        ))}
      </ol>
    </div>
  );
}
