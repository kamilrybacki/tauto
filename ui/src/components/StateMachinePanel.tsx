import { useEffect, useState, useMemo } from 'react';
import type { StateCoverage } from '../api/types';
import { fetchLifecycle } from '../api/client';

/* Lifecycles without a canvas: the classic FSM *state-transition table* form —
   one row per state, its outgoing transitions as links. Concise, complete,
   unambiguous (Wiegers), and it reads top-to-bottom on a phone. */

type Load =
  | { kind: 'loading' }
  | { kind: 'done'; data: StateCoverage[] }
  | { kind: 'error'; message: string };

type Category = 'initial' | 'terminal' | 'isolated' | 'undeclared' | 'normal';

function categorize(state: string, c: StateCoverage): Category {
  if (c.undeclared_states.includes(state)) return 'undeclared';
  if (c.isolated.includes(state)) return 'isolated';
  if (c.no_incoming.includes(state)) return 'initial';
  if (c.no_outgoing.includes(state)) return 'terminal';
  return 'normal';
}

const CATEGORY_LABEL: Record<Category, string | null> = {
  initial: 'initial',
  terminal: 'terminal',
  isolated: 'isolated — no rule',
  undeclared: 'undeclared — seen only in data',
  normal: null,
};

const caseOf = (key: string): string => key.split('/').pop() ?? key;

interface Outgoing {
  to: string;
  rules: string[];
}

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

  // One row per state, in workflow order (initial states first, by reachability).
  const rows = useMemo(() => {
    if (!current) return [];
    const all = [...current.states, ...current.undeclared_states.filter((s) => !current.states.includes(s))];
    const outgoing = new Map<string, Map<string, string[]>>();
    for (const t of current.transitions) {
      if (!t.from || !t.to) continue;
      if (!outgoing.has(t.from)) outgoing.set(t.from, new Map());
      const m = outgoing.get(t.from)!;
      if (!m.has(t.to)) m.set(t.to, []);
      m.get(t.to)!.push(t.contract);
    }
    // Order: BFS layers from initial states, then leftovers (isolated last).
    const layer = new Map<string, number>();
    current.no_incoming.forEach((s) => layer.set(s, 0));
    for (let pass = 0; pass < all.length + 1; pass++) {
      for (const t of current.transitions) {
        if (!t.from || !t.to) continue;
        const lf = layer.get(t.from) ?? 0;
        if ((layer.get(t.to) ?? -1) < lf + 1) layer.set(t.to, lf + 1);
      }
    }
    const ordered = [...all].sort((a, b) => {
      const ia = current.isolated.includes(a) ? 1 : 0;
      const ib = current.isolated.includes(b) ? 1 : 0;
      if (ia !== ib) return ia - ib;
      return (layer.get(a) ?? 99) - (layer.get(b) ?? 99) || a.localeCompare(b);
    });
    return ordered.map((s) => ({
      state: s,
      category: categorize(s, current),
      out: [...(outgoing.get(s) ?? new Map<string, string[]>()).entries()].map(
        ([to, rules]): Outgoing => ({ to, rules }),
      ),
    }));
  }, [current]);

  if (load.kind === 'loading') return <p className="empty-note">Tracing lifecycles…</p>;
  if (load.kind === 'error') return <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {load.message}</p>;
  if (load.kind === 'done' && load.data.length === 0)
    return (
      <p className="empty-note">
        No state fields declared. Mark an enum field as a <code>state</code> in the glossary
        (a <code>states:</code> section) to trace its lifecycle here.
      </p>
    );

  const nTrans = current
    ? new Set(current.transitions.filter((t) => t.from && t.to).map((t) => `${t.from}->${t.to}`)).size
    : 0;

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
            <span><b>{nTrans}</b> transitions</span>
            {current.isolated.length > 0 && (
              <span className="lc-bad">isolated: <code>{current.isolated.join(', ')}</code></span>
            )}
            {current.undeclared_states.length > 0 && (
              <span className="lc-bad">undeclared: <code>{current.undeclared_states.join(', ')}</code></span>
            )}
          </div>

          <div className="state-rows">
            {rows.map((r) => (
              <div key={r.state} className={`state-row cat-${r.category}`}>
                <div className="state-cell">
                  <code className="state-name">{r.state}</code>
                  {CATEGORY_LABEL[r.category] && (
                    <span className={`state-tag cat-${r.category}`}>{CATEGORY_LABEL[r.category]}</span>
                  )}
                </div>
                <div className="state-out">
                  {r.out.length === 0 ? (
                    <span className="state-none">
                      {r.category === 'isolated' || r.category === 'undeclared'
                        ? 'no rule reaches or leaves this state'
                        : 'no outgoing transitions'}
                    </span>
                  ) : (
                    r.out.map((o) => (
                      <div key={o.to} className="state-edge">
                        <code>→ {o.to}</code>
                        <span className="state-via">
                          {o.rules.map((rk, i) => (
                            <button
                              key={rk}
                              className="link-btn"
                              onClick={() => onOpenRule?.(rk)}
                              title="Open in the decision tables"
                            >
                              {caseOf(rk)}{i < o.rules.length - 1 ? ',' : ''}
                            </button>
                          ))}
                        </span>
                      </div>
                    ))
                  )}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
