import { useMemo, useCallback } from 'react';
import {
  ReactFlow,
  type Node,
  type Edge,
  type NodeTypes,
  Background,
  Controls,
  Handle,
  Position,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { GraphResponse, GraphEdge, ContractItem, Condition } from '../api/types';

/* Decision-point view: nodes are entity·operation (where decisions happen), not
   individual rules. Edges are *gates* derived from snapshot guards — a rule on
   Order guarding `paymentStatusSnapshot == Captured` is gated by the Payment
   operation whose ensures sets `paymentStatus == Captured`. (DMN DRD-style,
   one level above rules — replaces the per-rule hairball.) */

interface OpNodeData extends Record<string, unknown> {
  entity: string;
  operation: string;
  rules: number;
  conflict: boolean;
  selected: boolean;
}

function OpNode({ data }: { data: OpNodeData }) {
  return (
    <div
      className={`rf-node${data.conflict ? ' conflict' : ''}`}
      style={{
        minWidth: 132,
        boxShadow: data.selected ? '0 0 0 3px var(--accent-soft)' : 'none',
        borderColor: data.selected ? 'var(--accent)' : undefined,
        cursor: 'pointer',
      }}
    >
      <Handle type="target" position={Position.Left} style={{ background: '#8a919c', width: 5, height: 5 }} />
      <div className="rf-node-title" style={{ fontWeight: 600 }}>
        {data.operation}
        {data.conflict ? ' ⊥' : ''}
      </div>
      <div className="rf-node-sub">
        {data.entity} · {data.rules} rule{data.rules !== 1 ? 's' : ''}
      </div>
      <Handle type="source" position={Position.Right} style={{ background: '#8a919c', width: 5, height: 5 }} />
    </div>
  );
}

const nodeTypes = { op: OpNode as NodeTypes['string'] } as NodeTypes;

const leftPath = (c: Condition): string => String(c.left.value);
const shortField = (p: string): string => (p.includes('.') ? p.slice(p.indexOf('.') + 1) : p);

interface Gate {
  from: string; // op id `entity/operation`
  to: string;
  label: string;
}

function deriveGates(contracts: ContractItem[]): Gate[] {
  // Producer index: `field=value` (short field from ensures) → op ids that set it.
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
      // `paymentStatusSnapshot` mirrors another entity's `paymentStatus`.
      const mirrored = field.endsWith('Snapshot') ? field.slice(0, -'Snapshot'.length) : field;
      if (mirrored === field) continue; // only snapshot fields gate across entities
      const key = `${mirrored}=${String(g.right.value)}`;
      for (const fromId of producers.get(key) ?? []) {
        if (fromId === toId) continue;
        const id = `${fromId}->${toId}`;
        if (!gates.has(id)) gates.set(id, { from: fromId, to: toId, label: String(g.right.value) });
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

  const { nodes, edges } = useMemo(() => {
    // Ops + rule counts.
    const ops = new Map<string, { entity: string; operation: string; rules: ContractItem[] }>();
    for (const c of contracts) {
      const id = `${c.entity}/${c.operation}`;
      if (!ops.has(id)) ops.set(id, { entity: c.entity, operation: c.operation, rules: [] });
      ops.get(id)!.rules.push(c);
    }

    // Layer = longest gate-path from a source (workflow stage, left to right).
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

    // Swimlane per entity; stack ops sharing lane+layer.
    const entities = [...new Set(contracts.map((c) => c.entity))].sort();
    const laneOf = new Map(entities.map((e, i) => [e, i]));
    const taken = new Map<string, number>();
    const selectedOp = selected
      ? (() => {
          const c = contracts.find((x) => x.key === selected);
          return c ? `${c.entity}/${c.operation}` : null;
        })()
      : null;

    const nodes: Node<OpNodeData>[] = [...ops.entries()].map(([id, op]) => {
      const l = layer.get(id) ?? 0;
      const lane = laneOf.get(op.entity) ?? 0;
      const slot = `${lane}/${l}`;
      const bump = taken.get(slot) ?? 0;
      taken.set(slot, bump + 1);
      return {
        id,
        type: 'op',
        position: { x: l * 190, y: lane * 110 + bump * 58 },
        data: {
          entity: op.entity,
          operation: op.operation,
          rules: op.rules.length,
          conflict: conflictOps.has(id),
          selected: selectedOp === id,
        },
      };
    });

    const edges: Edge[] = gates.map((g) => ({
      id: `${g.from}->${g.to}`,
      source: g.from,
      target: g.to,
      label: g.label,
      style: { stroke: '#818cf8', strokeWidth: 1.4 },
      labelStyle: { fill: '#5a616b', fontSize: 10, fontFamily: 'IBM Plex Mono, monospace' },
      labelBgStyle: { fill: '#ffffff' },
    }));

    return { nodes, edges };
  }, [contracts, gates, conflictOps, selected]);

  const onNodeClick = useCallback(
    (_: React.MouseEvent, node: Node) => {
      const first = contracts.find((c) => `${c.entity}/${c.operation}` === node.id);
      if (first) onSelect(first.key);
    },
    [contracts, onSelect],
  );

  const nConf = conflictOps.size;
  return (
    <figure className="figure">
      <div className="figure-frame graph-canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={onNodeClick}
          fitView
          fitViewOptions={{ padding: 0.15 }}
          proOptions={{ hideAttribution: true }}
          colorMode="light"
          aria-label="Decision-point dependency graph"
        >
          <Background color="#e6e8ec" gap={26} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
      <figcaption className="figcaption">
        Decision points for <em>{slug || 'default'}</em>: {nodes.length} operations, {edges.length} gates
        (derived from snapshot guards){nConf > 0 ? `, ${nConf} with contradictory rules (⊥)` : ''}. Click an
        operation to open its decision table.
      </figcaption>
    </figure>
  );
}
