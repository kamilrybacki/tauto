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

const NOMEN: [string, string][] = [
  ['requires', 'Preconditions — all must hold for the operation to apply.'],
  ['⊢', 'Turnstile — the requires above entail the ensures below.'],
  ['ensures', 'Postconditions guaranteed of the result when it applies.'],
  ['forbidden', 'Operations that must not occur.'],
  ['preserves', 'Field paths whose value is unchanged.'],
  ['assumes', 'Ambient facts taken as given, not checked.'],
  ['⊥', 'Contradiction — two rules that cannot both hold.'],
];

export default function ContractList({ contracts, conflicts, selected, onSelect, onJump }: Props) {
  const conflictEdges = conflicts.filter((e) => e.kind === 'conflict');

  return (
    <div>
      {contracts.map((c, i) => {
        const clashes = conflictEdges
          .filter((e) => e.source === c.key || e.target === c.key)
          .map((e) => ({ other: e.source === c.key ? e.target : e.source, reason: e.label }));
        return (
          <article
            key={c.key}
            className={`proposition${selected === c.key ? ' selected' : ''}`}
            onClick={() => onSelect(c)}
          >
            <p className="prop-statement">
              <strong>Proposition 2.{i + 1}</strong>
              <strong>&nbsp;({c.case}).</strong>
              <span className="lead">&nbsp;For a {c.entity}, operation </span>
              <code>{c.operation}</code>
              <span className="lead">:</span>
            </p>

            <div className="prop-body">
              <span className="prop-label">requires</span>
              <div className="prop-values">
                {c.requires.length ? (
                  c.requires.map((r, j) => (
                    <div className="val" key={j}>
                      <code>{cond(r)}</code>
                    </div>
                  ))
                ) : (
                  <div className="val" style={{ color: 'var(--ink-60)', fontStyle: 'italic' }}>—</div>
                )}
              </div>

              <span className="turnstile" aria-hidden="true">⊢</span>
              <span className="prop-hr" />

              <span className="prop-label">ensures</span>
              <div className="prop-values">
                {c.ensures.length ? (
                  c.ensures.map((e, j) => (
                    <div className="val" key={j}>
                      <code>{cond(e)}</code>
                    </div>
                  ))
                ) : (
                  <div className="val" style={{ color: 'var(--ink-60)', fontStyle: 'italic' }}>—</div>
                )}
              </div>

              {c.forbidden.length > 0 && (
                <>
                  <span className="prop-label red">forbidden</span>
                  <div className="prop-values red">
                    {c.forbidden.map((f, j) => (
                      <div className="val" key={j}>
                        <code>{forbidden(f)}</code>
                      </div>
                    ))}
                  </div>
                </>
              )}
              {c.preserves.length > 0 && (
                <>
                  <span className="prop-label">preserves</span>
                  <div className="prop-values">
                    <code>{c.preserves.join(', ')}</code>
                  </div>
                </>
              )}
              {c.assumes.length > 0 && (
                <>
                  <span className="prop-label">assumes</span>
                  <div className="prop-values" style={{ fontStyle: 'italic' }}>
                    {c.assumes.join('; ')}
                  </div>
                </>
              )}
            </div>

            {(c.intent || c.source) && (
              <p className="prop-intent">
                {c.intent && (
                  <>
                    <span className="lbl">Intent.</span> {c.intent}{' '}
                  </>
                )}
                {c.source && <span className="src">[{c.source}]</span>}
              </p>
            )}

            {clashes.map((cl, j) => (
              <div className="bot-callout" key={j}>
                ⊥ In contradiction with{' '}
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

      <h3 style={{ marginTop: 34 }}>
        <span className="secnum">§2.1</span>
        Nomenclature
      </h3>
      <table className="nomen">
        <thead>
          <tr>
            <th scope="col">Term</th>
            <th scope="col">Meaning</th>
          </tr>
        </thead>
        <tbody>
          {NOMEN.map(([term, meaning]) => (
            <tr key={term}>
              <td className="term">{term}</td>
              <td>{meaning}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
