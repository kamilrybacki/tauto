import { useState } from 'react';
import type { ProofsResponse, LeanFile, ContractItem } from '../api/types';
import { fetchProofs } from '../api/client';

type State =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'done'; data: ProofsResponse }
  | { kind: 'error'; message: string };

const CONTRACTS_DIR = 'TautoContracts/contracts/';

/** Lean module names are the case name stripped to alphanumerics. */
const norm = (s: string): string => s.replace(/[^a-zA-Z0-9]/g, '');

export default function ProofsPanel({ contracts = [] }: { contracts?: ContractItem[] }) {
  const [state, setState] = useState<State>({ kind: 'idle' });
  const [selected, setSelected] = useState<LeanFile | null>(null);

  const run = () => {
    setState({ kind: 'loading' });
    fetchProofs()
      .then((data) => {
        setState({ kind: 'done', data });
        const first = data.files.find((f) => f.path.endsWith('.lean') && !f.path.includes('lakefile'));
        setSelected(first ?? data.files[0] ?? null);
      })
      .catch((e: unknown) => setState({ kind: 'error', message: e instanceof Error ? e.message : String(e) }));
  };

  if (state.kind === 'idle')
    return (
      <div>
        <button className="outline-btn" onClick={run}>Build proof obligations</button>
        <p className="section-desc" style={{ fontSize: 15, marginTop: 12, fontStyle: 'italic' }}>
          Each contract is compiled to a Lean&nbsp;4 theorem; satisfiability and conflict obligations are
          discharged by <code>decide</code> / <code>omega</code>.
        </p>
      </div>
    );
  if (state.kind === 'loading') return <p className="empty-note">Running <code>lake build</code>…</p>;
  if (state.kind === 'error') return <p className="empty-note" style={{ color: 'var(--red)' }}>Error: {state.message}</p>;

  const { data } = state;
  const contractFiles = data.files.filter((f) => f.path.startsWith(CONTRACTS_DIR));
  const infraFiles = data.files.filter((f) => !f.path.startsWith(CONTRACTS_DIR));
  const obligations = data.files.reduce((n, f) => n + (f.content.match(/theorem /g)?.length ?? 0), 0);
  const proven = data.build_available && data.build_success;
  const isContractThm = selected?.path.startsWith(CONTRACTS_DIR) ?? false;

  // Group contract files by entity via the rule set (module name = case name).
  const entityOf = new Map(contracts.map((c) => [norm(c.case), c.entity]));
  const groups = new Map<string, LeanFile[]>();
  for (const f of contractFiles) {
    const stem = f.path.replace(CONTRACTS_DIR, '').replace(/\.lean$/, '');
    const ent = entityOf.get(norm(stem)) ?? 'Contracts';
    if (!groups.has(ent)) groups.set(ent, []);
    groups.get(ent)!.push(f);
  }
  const hasTheorems = (f: LeanFile) => f.content.includes('theorem ');
  const fileBtn = (f: LeanFile, label: string) => (
    <button key={f.path} className={selected?.path === f.path ? 'active' : ''} onClick={() => setSelected(f)}>
      {label}
      {proven && hasTheorems(f) && <span className="thm-qed">∎</span>}
    </button>
  );

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
          {data.contracts} contract{data.contracts !== 1 ? 's' : ''} · {obligations} obligation
          {obligations !== 1 ? 's' : ''}
          {proven ? ' · all discharged (decide/omega)' : data.sorry_count > 0 ? ` · ${data.sorry_count} admitted (sorry)` : ''}
        </span>
        <button className="link-btn" onClick={run}>rebuild ↺</button>
      </div>

      {data.build_stderr && (
        <div className={`thm-output${!data.build_success ? ' err' : ''}`}>{data.build_stderr}{data.build_stdout}</div>
      )}

      <div className="thm-layout">
        <nav className="thm-nav" aria-label="Workspace files">
          {[...groups.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([ent, files]) => (
            <div key={ent}>
              <div className="grp">{ent}</div>
              {files.map((f) => fileBtn(f, f.path.replace(CONTRACTS_DIR, '').replace(/\.lean$/, '')))}
            </div>
          ))}
          {infraFiles.length > 0 && <div className="grp">Workspace</div>}
          {infraFiles.map((f) => fileBtn(f, f.path.replace('TautoContracts/', '')))}
        </nav>
        <div className="thm-listing">
          {selected && (
            <>
              <div className="cap">
                <code>{selected.path}</code>
                {proven && isContractThm && <span className="thm-qed">∎</span>}
              </div>
              <pre>{selected.content}</pre>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
