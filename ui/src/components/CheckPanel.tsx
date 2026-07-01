import { useState } from 'react';
import type { CheckResponse, ContractTestSuite, TestCase } from '../api/types';
import { checkRule } from '../api/client';

type State =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'done'; data: CheckResponse }
  | { kind: 'error'; message: string };

const PLACEHOLDER = `\`\`\`contract
case MyNewRule
entity: Order
operation: cancelOrder
requires: status == Pending, amount <= 1000
ensures: status == Cancelled
forbidden: shipOrder(order.id)
preserves: order.customer_id
\`\`\``;

function CompatBadge({ compatible }: { compatible: boolean }) {
  return compatible
    ? <span className="check-badge check-badge--ok">compatible</span>
    : <span className="check-badge check-badge--conflict">conflict detected</span>;
}

function GivenRow({ field, value, note }: { field: string; value: unknown; note?: string }) {
  const isSymbolic = typeof value === 'string' && value.startsWith('<');
  return (
    <tr>
      <td className="check-td-field">{field}</td>
      <td className={`check-td-value ${isSymbolic ? 'check-symbolic' : ''}`}>
        {String(value)}
      </td>
      {note && <td className="check-td-note">{note}</td>}
    </tr>
  );
}

function CaseCard({ tc }: { tc: TestCase }) {
  const [open, setOpen] = useState(false);
  const isHappy = tc.kind === 'happy_path';

  return (
    <div className={`check-case ${isHappy ? 'check-case--happy' : 'check-case--violation'}`}>
      <button className="check-case-header" onClick={() => setOpen(o => !o)}>
        <span className={`check-pass-badge ${tc.should_pass ? 'check-pass-badge--pass' : 'check-pass-badge--fail'}`}>
          {tc.should_pass ? 'PASS' : 'FAIL'}
        </span>
        <span className="check-case-id">{tc.id}</span>
        <span className="check-case-desc">{tc.description}</span>
        <span className="check-chevron">{open ? '▾' : '▸'}</span>
      </button>

      {open && (
        <div className="check-case-body">
          <div className="check-section-label">Given</div>
          <table className="check-table">
            <tbody>
              {tc.given.map((g, i) => (
                <GivenRow key={i} field={g.field} value={g.value} note={g.note} />
              ))}
            </tbody>
          </table>

          {tc.expect_ensures && tc.expect_ensures.length > 0 && (
            <>
              <div className="check-section-label">Expect ensures</div>
              <table className="check-table">
                <tbody>
                  {tc.expect_ensures.map((e, i) => (
                    <GivenRow key={i} field={e.field} value={e.value} />
                  ))}
                </tbody>
              </table>
            </>
          )}

          {tc.expect_forbidden_not_called && tc.expect_forbidden_not_called.length > 0 && (
            <div className="check-forbidden">
              Forbidden not called: {tc.expect_forbidden_not_called.join(', ')}
            </div>
          )}

          {tc.expect_preserved && tc.expect_preserved.length > 0 && (
            <div className="check-preserved">
              Preserved: {tc.expect_preserved.join(', ')}
            </div>
          )}

          {tc.violated_precondition && (
            <div className="check-violated">
              Violates: <code>{tc.violated_precondition}</code>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function SuiteSection({ suite, label }: { suite: ContractTestSuite; label?: string }) {
  const [open, setOpen] = useState(true);
  return (
    <div className="check-suite">
      <button className="check-suite-header" onClick={() => setOpen(o => !o)}>
        {label && <span className="check-suite-label-tag">{label}</span>}
        <span className="check-suite-key">{suite.contract}</span>
        <span className="check-suite-count">{suite.cases.length} cases</span>
        <span className="check-chevron">{open ? '▾' : '▸'}</span>
      </button>
      {open && (
        <div className="check-suite-cases">
          {suite.cases.map(tc => <CaseCard key={tc.id} tc={tc} />)}
        </div>
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
      .then(data => setState({ kind: 'done', data }))
      .catch((e: unknown) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  };

  return (
    <div className="check-panel">
      <div className="check-editor-pane">
        <div className="check-editor-label">Paste proposed contract rule</div>
        <textarea
          className="check-textarea"
          value={content}
          onChange={e => setContent(e.target.value)}
          placeholder={PLACEHOLDER}
          spellCheck={false}
        />
        <button
          className="check-run-btn"
          onClick={run}
          disabled={state.kind === 'loading' || !content.trim()}
        >
          {state.kind === 'loading' ? 'Checking…' : 'Check rule'}
        </button>
      </div>

      <div className="check-result-pane">
        {state.kind === 'idle' && (
          <div className="check-idle">
            Paste a contract rule and click <strong>Check rule</strong> to validate compatibility
            and get a JSON test suite.
          </div>
        )}

        {state.kind === 'error' && (
          <div className="check-error">Error: {state.message}</div>
        )}

        {state.kind === 'done' && (() => {
          const { data } = state;
          return (
            <div className="check-results">
              <div className="check-summary">
                <CompatBadge compatible={data.compatible} />
                <span className="check-summary-stat">
                  {data.proposed_contracts} proposed · {data.tests.total_cases} test cases
                </span>
                {data.parse_errors > 0 && (
                  <span className="check-summary-warn">{data.parse_errors} parse error{data.parse_errors !== 1 ? 's' : ''}</span>
                )}
              </div>

              {data.conflicts.length > 0 && (
                <div className="check-conflicts">
                  <div className="check-conflicts-title">Conflicts</div>
                  {data.conflicts.map((c, i) => (
                    <div key={i} className="check-conflict-item">
                      <span className="check-conflict-keys">{c.key_a} ↔ {c.key_b}</span>
                      <span className="check-conflict-reason">{c.reason}</span>
                    </div>
                  ))}
                </div>
              )}

              <div className="check-tests">
                {data.tests.proposed.length > 0 && (
                  <div className="check-tests-section">
                    <div className="check-tests-title">Proposed rule tests</div>
                    {data.tests.proposed.map(s => (
                      <SuiteSection key={s.contract} suite={s} label="new" />
                    ))}
                  </div>
                )}

                {data.tests.regression.length > 0 && (
                  <div className="check-tests-section">
                    <div className="check-tests-title">Regression tests (existing rules)</div>
                    {data.tests.regression.map(s => (
                      <SuiteSection key={s.contract} suite={s} label="existing" />
                    ))}
                  </div>
                )}
              </div>
            </div>
          );
        })()}
      </div>
    </div>
  );
}
