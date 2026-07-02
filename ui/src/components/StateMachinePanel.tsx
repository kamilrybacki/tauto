import { useEffect, useState, useMemo } from 'react';
import { ReactFlow, Background, Controls, type Node, type Edge } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { StateCoverage } from '../api/types';
import { fetchLifecycle } from '../api/client';

type Load =
  | { kind: 'loading' }
  | { kind: 'done'; data: StateCoverage[] }
  | { kind: 'error'; message: string };

type Category = 'initial' | 'terminal' | 'isolated' | 'undeclared' | 'normal';

const CATEGORY_STYLE: Record<Category, React.CSSProperties> = {
  initial:    { border: '2px solid #22c55e' },
  terminal:   { border: '2px solid #f59e0b' },
  isolated:   { border: '2px dashed #f87171', opacity: 0.85 },
  undeclared: { border: '2px solid #a78bfa' },
  normal:     { border: '1px solid #4b5563' },
};

function categorize(state: string, c: StateCoverage): Category {
  if (c.undeclared_states.includes(state)) return 'undeclared';
  if (c.isolated.includes(state)) return 'isolated';
  if (c.no_incoming.includes(state)) return 'initial';
  if (c.no_outgoing.includes(state)) return 'terminal';
  return 'normal';
}

/** Layered left-to-right layout: BFS distance from the initial (no-incoming)
 *  states along transitions; isolated states go in a trailing lane. Bounded so
 *  cycles can't loop forever. */
function layout(c: StateCoverage): { nodes: Node[]; edges: Edge[] } {
  const allStates = [...c.states, ...c.undeclared_states.filter(s => !c.states.includes(s))];
  const layer = new Map<string, number>();
  c.no_incoming.forEach(s => layer.set(s, 0));
  // Relaxation passes (cap = #states) push targets one layer past their source.
  for (let pass = 0; pass < allStates.length + 1; pass++) {
    for (const t of c.transitions) {
      if (!t.from || !t.to) continue;
      const lf = layer.get(t.from) ?? 0;
      if ((layer.get(t.to) ?? -1) < lf + 1) layer.set(t.to, lf + 1);
    }
  }
  const maxLayer = Math.max(0, ...[...layer.values()]);
  // Unplaced (unreachable & not initial): isolated → trailing lane; others → col 0.
  allStates.forEach(s => {
    if (!layer.has(s)) layer.set(s, c.isolated.includes(s) ? maxLayer + 1 : 0);
  });

  const perLayer = new Map<number, number>();
  const nodes: Node[] = allStates.map(s => {
    const l = layer.get(s) ?? 0;
    const idx = perLayer.get(l) ?? 0;
    perLayer.set(l, idx + 1);
    return {
      id: s,
      position: { x: l * 210 + 30, y: idx * 84 + 30 },
      data: { label: s },
      style: {
        ...CATEGORY_STYLE[categorize(s, c)],
        background: '#111827',
        color: '#e5e7eb',
        borderRadius: 8,
        fontSize: 12,
        padding: '6px 10px',
        width: 150,
      },
    };
  });

  const edges: Edge[] = c.transitions
    .filter(t => t.from && t.to)
    .map((t, i) => ({
      id: `${t.from}->${t.to}-${i}`,
      source: t.from!,
      target: t.to!,
      label: t.contract.split('/').pop(),
      labelStyle: { fill: '#9ca3af', fontSize: 10 },
      labelBgStyle: { fill: '#0f172a' },
      style: { stroke: '#4b5563' },
    }));

  return { nodes, edges };
}

function CoverageDiagram({ c }: { c: StateCoverage }) {
  const { nodes, edges } = useMemo(() => layout(c), [c]);
  return (
    <div className="sm-entity">
      <div className="sm-entity-head">
        <span className="sm-entity-title">{c.entity}<span className="sm-field">.{c.state_field}</span></span>
        <span className="sm-counts">
          {c.states.length} states · {c.transitions.filter(t => t.from && t.to).length} transitions
          {c.isolated.length > 0 && <span className="sm-warn"> · {c.isolated.length} isolated</span>}
        </span>
      </div>
      <div className="sm-canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          fitView
          nodesConnectable={false}
          nodesDraggable
          proOptions={{ hideAttribution: true }}
        >
          <Background color="#1f2937" gap={18} />
          <Controls showInteractive={false} />
        </ReactFlow>
      </div>
    </div>
  );
}

export default function StateMachinePanel() {
  const [load, setLoad] = useState<Load>({ kind: 'loading' });

  useEffect(() => {
    fetchLifecycle()
      .then(data => setLoad({ kind: 'done', data }))
      .catch((e: unknown) => setLoad({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  }, []);

  if (load.kind === 'loading') return <div className="sm-empty">Loading lifecycles…</div>;
  if (load.kind === 'error') return <div className="sm-empty sm-error">Error: {load.message}</div>;
  if (load.data.length === 0)
    return (
      <div className="sm-empty">
        No state fields declared. Mark an enum field as a <code>state</code> in the glossary
        (a <code>states:</code> section) to see its lifecycle.
      </div>
    );

  return (
    <div className="sm-panel">
      <div className="sm-legend">
        <span><i className="sm-dot sm-dot--initial" /> initial (no incoming)</span>
        <span><i className="sm-dot sm-dot--terminal" /> terminal (no outgoing)</span>
        <span><i className="sm-dot sm-dot--isolated" /> isolated (no rule)</span>
        <span><i className="sm-dot sm-dot--undeclared" /> undeclared (from data)</span>
      </div>
      {load.data.map(c => (
        <CoverageDiagram key={`${c.entity}.${c.state_field}`} c={c} />
      ))}
    </div>
  );
}
