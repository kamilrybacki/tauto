import { useState } from 'react';
import type { ReportResponse, ReportRule, LeanFile } from '../api/types';
import { fetchReport } from '../api/client';

/* Proofs = the verification report: per rule, its machine-checked obligations
   (with kind + statement + build status), generated tests, and findings — the
   same JSON the MCP get_verification_report tool serves to LLMs. Lean sources
   stay one toggle away. */

type State =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'done'; data: ReportResponse }
  | { kind: 'error'; message: string };

const KIND_LABEL: Record<string, string> = {
  satisfiability: 'satisfiable',
  guards_disjoint: 'guards disjoint',
  outcome_conflict: 'conflict',
  dead_rule: 'dead rule',
};

const caseOf = (key: string): string => key.split('/').pop() ?? key;

function RuleCard({ rule, discharged }: { rule: ReportRule; discharged: boolean }) {
  const bad = (rule.dead_rule ? 1 : 0) + (rule.conflicts?.length ?? 0);
  return (
    <div className={`vr-rule${bad > 0 ? ' bad' : ''}`}>
      <div className="vr-head">
        <span className="vr-case">{rule.case}</span>
        <span className="vr-path">{rule.entity} · {rule.operation}</span>
        <span className="vr-tests">{rule.tests.length} test{rule.tests.length !== 1 ? 's' : ''}</span>
      </div>
      <div className="vr-obls">
        {rule.obligations.map((o) => (
          <div key={o.theorem + (o.pair ?? '')} className={`vr-obl${o.kind === 'outcome_conflict' || o.kind === 'dead_rule' ? ' red' : ''}`}>
            <span className={`vr-qed${o.status === 'discharged' ? '' : o.status === 'failed' ? ' failed' : ' pending'}`}>
              {o.status === 'discharged' ? '∎' : o.status === 'failed' ? '✗' : '…'}
            </span>
            <span className="vr-kind">{KIND_LABEL[o.kind] ?? o.kind}</span>
            <code className="vr-stmt">{o.statement}</code>
            {o.pair && <span className="vr-pair">vs {caseOf(o.pair)}</span>}
            {o.error && <span className="vr-err">{o.error}</span>}
          </div>
        ))}
        {rule.obligations.length === 0 && (
          <span className="vr-none">no decidable obligations (conditions not modelled)</span>
        )}
      </div>
      {rule.dead_rule && <div className="vr-flag">Dead rule — {rule.dead_rule.reason}</div>}
      {(rule.conflicts ?? []).map((c, i) => (
        <div className="vr-flag" key={i}>
          Conflicts with {caseOf(c.key_a === rule.key ? c.key_b : c.key_a)} — {c.reason}
        </div>
      ))}
      {rule.conformance.some((c) => c.status !== 'pass') &&
        rule.conformance
          .filter((c) => c.status !== 'pass')
          .map((c, i) => (
            <div className={`vr-flag${c.status === 'fail' ? '' : ' soft'}`} key={`cf${i}`}>
              Example #{c.index + 1} {c.status}: {c.message}
            </div>
          ))}
      {!discharged && <span />}
    </div>
  );
}

function LeanSource({ files }: { files: LeanFile[] }) {
  const [selected, setSelected] = useState<LeanFile | null>(files[0] ?? null);
  return (
    <div className="thm-layout" style={{ marginTop: 10 }}>
      <nav className="thm-nav" aria-label="Workspace files">
        {files.map((f) => (
          <button key={f.path} className={selected?.path === f.path ? 'active' : ''} onClick={() => setSelected(f)}>
            {f.path.replace('TautoContracts/contracts/', '').replace('TautoContracts/', '')}
          </button>
        ))}
      </nav>
      <div className="thm-listing">
        {selected && (
          <>
            <div className="cap"><code>{selected.path}</code></div>
            <pre>{selected.content}</pre>
          </>
        )}
      </div>
    </div>
  );
}

export default function ProofsPanel() {
  const [state, setState] = useState<State>({ kind: 'idle' });

  const run = () => {
    setState({ kind: 'loading' });
    fetchReport()
      .then((data) => setState({ kind: 'done', data }))
      .catch((e: unknown) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  };

  if (state.kind === 'idle')
    return (
      <div>
        <button className="outline-btn" onClick={run}>Verify the rule set</button>
        <p className="section-desc" style={{ marginTop: 12 }}>
          Compiles every rule to Lean&nbsp;4 theorems and machine-checks them with <code>lake</code>;
          the report ties each rule to its proved obligations and generated tests. The same JSON is
          available to agents via the MCP <code>get_verification_report</code> tool.
        </p>
      </div>
    );
  if (state.kind === 'loading') return <p className="empty-note">Building the Lean workspace…</p>;
  if (state.kind === 'error') return <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {state.message}</p>;

  const { data } = state;
  const discharged = data.build_available && data.build_success;

  return (
    <div>
      <div className="thm-status">
        {!data.build_available ? (
          <span className="badge bad">lake not available</span>
        ) : data.build_success ? (
          <span className="badge ok">build ✓</span>
        ) : (
          <span className="badge bad">build failed</span>
        )}
        <span className="thm-stats">
          {data.rules.length} rule{data.rules.length !== 1 ? 's' : ''} · {data.obligations_total} obligation
          {data.obligations_total !== 1 ? 's' : ''}
          {discharged ? ' · all discharged (decide/omega)' : ''}
        </span>
        <button className="link-btn" onClick={run}>re-verify ↺</button>
      </div>

      {data.build_stderr && !data.build_success && (
        <div className="thm-output err">{data.build_stderr}</div>
      )}

      {data.rules.map((r) => (
        <RuleCard key={r.key} rule={r} discharged={discharged} />
      ))}

      <details className="vr-src">
        <summary>Lean source ({data.files.length} files)</summary>
        <LeanSource files={data.files} />
      </details>
    </div>
  );
}
