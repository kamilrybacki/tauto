import type { ContractItem, Condition, ForbiddenOperation, GraphEdge } from '../api/types';

interface Props {
  contracts: ContractItem[];
  conflicts: GraphEdge[];
  selected: string | null;
  onSelect: (item: ContractItem) => void;
  onJump: (key: string) => void;
}

const cond = (c: Condition): string =>
  `${String(c.left.value)} ${c.operator} ${String(c.right.value)}`;
const forbidden = (f: ForbiddenOperation): string =>
  `${f.operation}(${(f.args ?? []).map((a) => String(a.value)).join(', ')})`;
const caseOf = (key: string): string => key.split('/').pop() ?? key;

function Row({ label, red, children }: { label: string; red?: boolean; children: React.ReactNode }) {
  return (
    <>
      <span className={`cc-label${red ? ' red' : ''}`}>{label}</span>
      <div className={`cc-vals${red ? ' red' : ''}`}>{children}</div>
    </>
  );
}

export default function ContractList({ contracts, conflicts, selected, onSelect, onJump }: Props) {
  const conflictEdges = conflicts.filter((e) => e.kind === 'conflict');

  return (
    <div>
      {contracts.map((c) => {
        const clashes = conflictEdges
          .filter((e) => e.source === c.key || e.target === c.key)
          .map((e) => ({ other: e.source === c.key ? e.target : e.source, reason: e.label }));
        return (
          <article
            key={c.key}
            className={`contract-card${selected === c.key ? ' selected' : ''}${clashes.length ? ' has-conflict' : ''}`}
            onClick={() => onSelect(c)}
          >
            <div className="cc-head">
              <span className="cc-title">{c.case}</span>
              <span className="cc-path">{c.entity} · {c.operation}</span>
              {clashes.length > 0 && <span className="pill conflict">conflict</span>}
            </div>

            <div className="cc-grid">
              <Row label="requires">
                {c.requires.length ? (
                  c.requires.map((r, j) => <code key={j}>{cond(r)}</code>)
                ) : (
                  <span className="muted">—</span>
                )}
              </Row>
              <Row label="ensures">
                {c.ensures.length ? (
                  c.ensures.map((e, j) => <code key={j}>{cond(e)}</code>)
                ) : (
                  <span className="muted">—</span>
                )}
              </Row>
              {c.forbidden.length > 0 && (
                <Row label="forbidden" red>
                  {c.forbidden.map((f, j) => <code key={j}>{forbidden(f)}</code>)}
                </Row>
              )}
              {c.preserves.length > 0 && (
                <Row label="preserves">
                  <code>{c.preserves.join(', ')}</code>
                </Row>
              )}
              {c.assumes.length > 0 && (
                <Row label="assumes">
                  <span className="muted" style={{ fontFamily: 'var(--sans)', fontSize: 13 }}>{c.assumes.join('; ')}</span>
                </Row>
              )}
            </div>

            {(c.intent || c.source) && (
              <p className="cc-intent">
                {c.intent && (
                  <>
                    <span className="lbl">Intent.</span> {c.intent}{' '}
                  </>
                )}
                {c.source && <span className="src">{c.source}</span>}
              </p>
            )}

            {clashes.map((cl, j) => (
              <div className="cc-conflict" key={j}>
                Conflicts with{' '}
                <button
                  onClick={(ev) => {
                    ev.stopPropagation();
                    onJump(cl.other);
                  }}
                >
                  {caseOf(cl.other)}
                </button>
                {cl.reason ? ` — ${cl.reason}` : ''}
              </div>
            ))}
          </article>
        );
      })}
    </div>
  );
}
