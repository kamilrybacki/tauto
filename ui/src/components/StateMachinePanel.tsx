import { useEffect, useState, useMemo } from 'react';
import { ReactFlow, Background, MarkerType, type Node, type Edge } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import type { StateCoverage } from '../api/types';
import { fetchLifecycle } from '../api/client';

type Load =
  | { kind: 'loading' }
  | { kind: 'done'; data: StateCoverage[] }
  | { kind: 'error'; message: string };

type Category = 'initial' | 'terminal' | 'isolated' | 'undeclared' | 'normal';

const RED = '#dc2626';
const INK = '#191c21';

const CATEGORY_STYLE: Record<Category, React.CSSProperties> = {
  initial: { border: `2px solid ${INK}` },
  terminal: { border: `3px double ${INK}` },
  isolated: { border: `2px dashed ${RED}`, color: RED },
  undeclared: { border: `2px dotted ${RED}`, color: RED },
  normal: { border: '1px solid #d3d7de' },
};

function categorize(state: string, c: StateCoverage): Category {
  if (c.undeclared_states.includes(state)) return 'undeclared';
  if (c.isolated.includes(state)) return 'isolated';
  if (c.no_incoming.includes(state)) return 'initial';
  if (c.no_outgoing.includes(state)) return 'terminal';
  return 'normal';
}

interface Transition {
  from: string;
  to: string;
  rules: string[]; // full contract keys
}

function transitionsOf(c: StateCoverage): Transition[] {
  const m = new Map<string, Transition>();
  for (const t of c.transitions) {
    if (!t.from || !t.to) continue;
    const k = `${t.from}->${t.to}`;
    if (!m.has(k)) m.set(k, { from: t.from, to: t.to, rules: [] });
    m.get(k)!.rules.push(t.contract);
  }
  return [...m.values()];
}

function layout(c: StateCoverage): Node[] {
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
  return allStates.map((s) => {
    const l = layer.get(s) ?? 0;
    const idx = perLayer.get(l) ?? 0;
    perLayer.set(l, idx + 1);
    return {
      id: s,
      position: { x: l * 190 + 20, y: idx * 72 + 20 },
      data: { label: s },
      style: {
        ...CATEGORY_STYLE[categorize(s, c)],
        background: '#ffffff',
        color: CATEGORY_STYLE[categorize(s, c)].color ?? INK,
        borderRadius: 8,
        fontFamily: 'IBM Plex Mono, monospace',
        fontSize: 12,
        padding: '6px 10px',
        width: 140,
      },
    };
  });
}

const caseOf = (key: string): string => key.split('/').pop() ?? key;

export default function StateMachinePanel({ onOpenRule }: { onOpenRule?: (key: string) => void }) {
  const [load, setLoad] = useState<Load>({ kind: 'loading' });
  const [active, setActive] = useState<string>('');

  useEffect(() => {
    fetchLifecycle()
      .then((data) => {
        setLoad({ kind: 'done', data });
        if (data.length) setActive(`${data[0].entity}.${data[0].state_field}`);
      })
      .catch((e: unknown) => setLoad({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  }, []);

  const current = load.kind === 'done' ? load.data.find((c) => `${c.entity}.${c.state_field}` === active) : undefined;
  const transitions = useMemo(() => (current ? transitionsOf(current) : []), [current]);
  const nodes = useMemo(() => (current ? layout(current) : []), [current]);
  const edges: Edge[] = useMemo(
    () =>
      transitions.map((t) => ({
        id: `${t.from}->${t.to}`,
        source: t.from,
        target: t.to,
        style: { stroke: '#8a919c', strokeWidth: 1.3 },
        markerEnd: { type: MarkerType.ArrowClosed, color: '#8a919c', width: 16, height: 16 },
      })),
    [transitions],
  );

  if (load.kind === 'loading') return <p className="empty-note">Tracing lifecycles…</p>;
  if (load.kind === 'error') return <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {load.message}</p>;
  if (load.kind === 'done' && load.data.length === 0)
    return (
      <p className="empty-note">
        No state fields declared. Mark an enum field as a <code>state</code> in the glossary
        (a <code>states:</code> section) to trace its lifecycle here.
      </p>
    );

  return (
    <div>
      <div className="chips">
        {load.kind === 'done' &&
          load.data.map((c) => {
            const id = `${c.entity}.${c.state_field}`;
            const bad = c.isolated.length + c.undeclared_states.length;
            return (
              <button key={id} className={`chip${active === id ? ' active' : ''}`} onClick={() => setActive(id)}>
                {c.entity}
                <span className="chip-sub">.{c.state_field}</span>
                {bad > 0 && <span className="chip-warn">{bad}</span>}
              </button>
            );
          })}
      </div>

      {current && (
        <>
          <div className="lc-summary">
            <span><b>{current.states.length}</b> states</span>
            <span><b>{transitions.length}</b> transitions</span>
            {current.no_incoming.length > 0 && (
              <span>initial: <code>{current.no_incoming.join(', ')}</code></span>
            )}
            {current.no_outgoing.length > 0 && (
              <span>terminal: <code>{current.no_outgoing.join(', ')}</code></span>
            )}
            {current.isolated.length > 0 && (
              <span className="lc-bad">isolated: <code>{current.isolated.join(', ')}</code></span>
            )}
            {current.undeclared_states.length > 0 && (
              <span className="lc-bad">undeclared: <code>{current.undeclared_states.join(', ')}</code></span>
            )}
          </div>

          <div className="figure-frame sm-canvas">
            <ReactFlow
              key={active}
              nodes={nodes}
              edges={edges}
              fitView
              nodesConnectable={false}
              proOptions={{ hideAttribution: true }}
              colorMode="light"
              aria-label={`Lifecycle of ${current.entity}.${current.state_field}`}
            >
              <Background color="#e6e8ec" gap={20} />
            </ReactFlow>
          </div>

          <div className="trans-list">
            {transitions.map((t) => (
              <div key={`${t.from}->${t.to}`} className="trans-row">
                <code className="trans-arrow">{t.from} → {t.to}</code>
                <span className="trans-rules">
                  {t.rules.map((r, i) => (
                    <button
                      key={r}
                      className="link-btn"
                      onClick={() => onOpenRule?.(r)}
                      title="Open in the decision tables"
                    >
                      {caseOf(r)}{i < t.rules.length - 1 ? ',' : ''}
                    </button>
                  ))}
                </span>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
