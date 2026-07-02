import { useEffect, useState, useMemo } from 'react';
import { ReactFlow, Background, type Node, type Edge } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { StateCoverage } from '../api/types';
import { fetchLifecycle } from '../api/client';

type Load =
  | { kind: 'loading' }
  | { kind: 'done'; data: StateCoverage[] }
  | { kind: 'error'; message: string };

type Category = 'initial' | 'terminal' | 'isolated' | 'undeclared' | 'normal';

const INK = '#1c1814';
const RED = '#8c2f22';
const PAPER = '#faf7ef';

const CATEGORY_STYLE: Record<Category, React.CSSProperties> = {
  initial: { border: `2px solid ${INK}` },
  terminal: { border: `3px double ${INK}` },
  isolated: { border: `2px dashed ${RED}`, color: RED, fontStyle: 'italic' },
  undeclared: { border: `2px dotted ${RED}`, color: RED },
  normal: { border: `1px solid rgba(28,24,20,0.55)` },
};

function categorize(state: string, c: StateCoverage): Category {
  if (c.undeclared_states.includes(state)) return 'undeclared';
  if (c.isolated.includes(state)) return 'isolated';
  if (c.no_incoming.includes(state)) return 'initial';
  if (c.no_outgoing.includes(state)) return 'terminal';
  return 'normal';
}

function layout(c: StateCoverage): { nodes: Node[]; edges: Edge[] } {
  const allStates = [...c.states, ...c.undeclared_states.filter((s) => !c.states.includes(s))];
  const layer = new Map<string, number>();
  c.no_incoming.forEach((s) => layer.set(s, 0));
  for (let pass = 0; pass < allStates.length + 1; pass++) {
    for (const t of c.transitions) {
      if (!t.from || !t.to) continue;
      const lf = layer.get(t.from) ?? 0;
      if ((layer.get(t.to) ?? -1) < lf + 1) layer.set(t.to, lf + 1);
    }
  }
  const maxLayer = Math.max(0, ...[...layer.values()]);
  allStates.forEach((s) => {
    if (!layer.has(s)) layer.set(s, c.isolated.includes(s) ? maxLayer + 1 : 0);
  });

  const perLayer = new Map<number, number>();
  const nodes: Node[] = allStates.map((s) => {
    const l = layer.get(s) ?? 0;
    const idx = perLayer.get(l) ?? 0;
    perLayer.set(l, idx + 1);
    return {
      id: s,
      position: { x: l * 200 + 24, y: idx * 78 + 24 },
      data: { label: s },
      style: {
        ...CATEGORY_STYLE[categorize(s, c)],
        background: PAPER,
        color: CATEGORY_STYLE[categorize(s, c)].color ?? INK,
        borderRadius: 6,
        fontFamily: 'IBM Plex Mono, monospace',
        fontSize: 12,
        padding: '6px 10px',
        width: 148,
      },
    };
  });

  const edges: Edge[] = c.transitions
    .filter((t) => t.from && t.to)
    .map((t, i) => ({
      id: `${t.from}->${t.to}-${i}`,
      source: t.from!,
      target: t.to!,
      label: t.contract.split('/').pop(),
      labelStyle: { fill: 'rgba(28,24,20,0.7)', fontSize: 11, fontStyle: 'italic', fontFamily: 'IBM Plex Mono, monospace' },
      labelBgStyle: { fill: PAPER },
      style: { stroke: 'rgba(28,24,20,0.45)' },
    }));

  return { nodes, edges };
}

function CoverageFigure({ c, num }: { c: StateCoverage; num: number }) {
  const { nodes, edges } = useMemo(() => layout(c), [c]);
  const transitions = c.transitions.filter((t) => t.from && t.to).length;
  const flags: string[] = [];
  if (c.isolated.length) flags.push(`isolated: ${c.isolated.join(', ')}`);
  if (c.undeclared_states.length) flags.push(`undeclared (from data): ${c.undeclared_states.join(', ')}`);
  return (
    <figure className="figure">
      <div className="figure-frame sm-canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          fitView
          nodesConnectable={false}
          proOptions={{ hideAttribution: true }}
          colorMode="light"
          aria-label={`Lifecycle of ${c.entity}.${c.state_field}`}
        >
          <Background color="rgba(28,24,20,0.08)" gap={20} />
        </ReactFlow>
      </div>
      <figcaption className="figcaption">
        <b>Figure 3.{num}.</b> Lifecycle of <code>{c.entity}.{c.state_field}</code> — {c.states.length} declared
        state{c.states.length !== 1 ? 's' : ''}, {transitions} transition{transitions !== 1 ? 's' : ''}.
      </figcaption>
      {flags.length > 0 && <p className="figure-note">{flags.join(' · ')}</p>}
    </figure>
  );
}

export default function StateMachinePanel() {
  const [load, setLoad] = useState<Load>({ kind: 'loading' });

  useEffect(() => {
    fetchLifecycle()
      .then((data) => setLoad({ kind: 'done', data }))
      .catch((e: unknown) => setLoad({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  }, []);

  if (load.kind === 'loading') return <p className="empty-note">Tracing lifecycles…</p>;
  if (load.kind === 'error') return <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {load.message}</p>;
  if (load.data.length === 0)
    return (
      <p className="empty-note">
        No state fields declared. Mark an enum field as a <code>state</code> in the glossary
        (a <code>states:</code> section) to trace its lifecycle here.
      </p>
    );

  return (
    <div>
      <p className="prose" style={{ fontSize: 15, color: 'var(--ink-70)' }}>
        Legend: solid border — initial (no incoming); double border — terminal (no outgoing);{' '}
        <span style={{ color: 'var(--red)', fontStyle: 'italic' }}>dashed red — isolated</span> (no rule);{' '}
        <span style={{ color: 'var(--red)' }}>dotted red — undeclared</span> (seen only in data).
      </p>
      {load.data.map((c, i) => (
        <CoverageFigure key={`${c.entity}.${c.state_field}`} c={c} num={i + 1} />
      ))}
    </div>
  );
}
