import { useState } from 'react';
import type { CheckResponse, TestCase } from '../api/types';
import { checkRule } from '../api/client';

type State =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'done'; data: CheckResponse }
  | { kind: 'error'; message: string };

const PLACEHOLDER = `\`\`\`contract
case MyNewRule
entity:
  Order
operation:
  cancel
requires:
  order.status == Pending
ensures:
  result.status == Cancelled
intent:
  A pending order may be cancelled.
examples:
  - given: status=Pending; then: status=Cancelled
\`\`\``;

const caseOf = (key: string): string => key.split('/').pop() ?? key;

function CaseBlock({ tc, num }: { tc: TestCase; num: number }) {
  const happy = tc.kind === 'happy_path';
  return (
    <div className={`tc${happy ? '' : ' reject'}`}>
      <div>
        <span className="tc-tag" style={happy ? undefined : { color: 'var(--red)' }}>
          {happy ? 'happy path — must pass' : 'precondition violation — must be rejected'}
        </span>{' '}
        <span className="tc-id">Case {num} · {tc.id}</span>
      </div>
      <div style={{ fontSize: 14.5, margin: '2px 0' }}>{tc.description}</div>
      {tc.given.length > 0 && (
        <table>
          <tbody>
            {tc.given.map((g, i) => (
              <tr key={i}>
                <td>{g.field}</td>
                <td>{String(g.value)}</td>
                {g.note && <td className="note">{g.note}</td>}
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {tc.violated_precondition && (
        <div className="expect">Violates: {tc.violated_precondition}</div>
      )}
    </div>
  );
}

export default function CheckPanel() {
  const [content, setContent] = useState('');
  const [state, setState] = useState<State>({ kind: 'idle' });

  const run = () => {
    if (!content.trim()) return;
    setState({ kind: 'loading' });
    checkRule(content)
      .then((data) => setState({ kind: 'done', data }))
      .catch((e: unknown) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  };

  return (
    <div>
      <div className="referee-form">
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
          <span className="small-caps">proposed rule</span>
          {!content && (
            <button className="link-btn" style={{ fontSize: 13 }} onClick={() => setContent(PLACEHOLDER)}>
              insert a template
            </button>
          )}
        </div>
        <textarea
          className="referee-textarea"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder={PLACEHOLDER}
          spellCheck={false}
          aria-label="Proposed contract rule"
        />
        <div>
          <button className="outline-btn" onClick={run} disabled={state.kind === 'loading' || !content.trim()}>
            {state.kind === 'loading' ? 'Reviewing…' : 'Submit for review'}
          </button>
        </div>
      </div>

      {state.kind === 'error' && <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {state.message}</p>}

      {state.kind === 'done' && (() => {
        const { data } = state;
        const conformFails = (data.conformance ?? []).filter((o) => o.status === 'fail');
        const conformUnder = (data.conformance ?? []).filter((o) => o.status === 'underspecified');
        const dead = data.dead_rules ?? [];
        const gloss = data.glossary_warnings ?? [];
        const reject = !data.compatible || data.conformant === false || dead.length > 0 || data.parse_errors > 0;
        return (
          <div>
            <p className="verdict">
              <span className={`v ${reject ? 'reject' : 'accept'}`}>{reject ? 'Reject' : 'Accept'}.</span>{' '}
              {reject
                ? 'The proposed rule does not pass review as stated.'
                : `Compatible with the corpus; ${data.tests.total_cases} conformance cases generated.`}
            </p>

            <ol className="findings">
              {data.conflicts.map((c, i) => (
                <li key={`cf${i}`}>
                  <span className="finding-head red">Contradiction.</span>{' '}
                  <span className="finding-body">
                    {caseOf(c.key_a)} ↔ {caseOf(c.key_b)} — {c.reason}
                  </span>
                </li>
              ))}
              {dead.map((d, i) => (
                <li key={`dr${i}`}>
                  <span className="finding-head red" style={{ fontStyle: 'italic' }}>Dead rule.</span>{' '}
                  <span className="finding-body">{caseOf(d.key)} — unsatisfiable on <code>{d.field}</code> ({d.reason})</span>
                </li>
              ))}
              {conformFails.map((o, i) => (
                <li key={`co${i}`}>
                  <span className="finding-head red">Non-conformance.</span>{' '}
                  <span className="finding-body">{o.case} — {o.message}</span>
                </li>
              ))}
              {data.parse_errors > 0 && (
                <li>
                  <span className="finding-head red">Parsing.</span>{' '}
                  <span className="finding-body">{data.parse_errors} parse error{data.parse_errors !== 1 ? 's' : ''}.</span>
                </li>
              )}
              {gloss.map((w, i) => (
                <li key={`gl${i}`}>
                  <span className="finding-head">Vocabulary.</span>{' '}
                  <span className="finding-body">{caseOf(w.contract)} — {w.message}</span>
                </li>
              ))}
              {conformUnder.map((o, i) => (
                <li key={`cu${i}`}>
                  <span className="finding-head">Underspecified.</span>{' '}
                  <span className="finding-body">{o.case} — {o.message}</span>
                </li>
              ))}
              {!reject && data.conflicts.length === 0 && gloss.length === 0 && conformUnder.length === 0 && (
                <li><span className="finding-body" style={{ fontStyle: 'italic' }}>No objections.</span></li>
              )}
            </ol>

            {data.tests.proposed.some((s) => s.cases.length > 0) && (
              <>
                <h3 style={{ marginTop: 24 }}><span className="secnum">§5.1</span>Conformance suite</h3>
                {data.tests.proposed.map((s) => (
                  <div key={s.contract}>
                    <p className="section-desc" style={{ fontSize: 15, margin: '8px 0 0' }}>
                      <code>{caseOf(s.contract)}</code>
                    </p>
                    {s.cases.map((tc, i) => (
                      <CaseBlock key={tc.id} tc={tc} num={i + 1} />
                    ))}
                  </div>
                ))}
              </>
            )}
          </div>
        );
      })()}
    </div>
  );
}
