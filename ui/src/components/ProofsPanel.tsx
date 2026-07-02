import { useState } from 'react';
import type { ProofsResponse, LeanFile } from '../api/types';
import { fetchProofs } from '../api/client';

type State =
  | { kind: 'idle' }
  | { kind: 'loading' }
  | { kind: 'done'; data: ProofsResponse }
  | { kind: 'error'; message: string };

function BuildBadge({ data }: { data: ProofsResponse }) {
  if (!data.build_available) {
    return <span className="proof-badge proof-badge--warn">lake not available</span>;
  }
  return data.build_success
    ? <span className="proof-badge proof-badge--ok">build ok</span>
    : <span className="proof-badge proof-badge--fail">build failed</span>;
}

function FileViewer({ file }: { file: LeanFile }) {
  return (
    <div className="proof-file-viewer">
      <div className="proof-file-path">{file.path}</div>
      <pre className="proof-file-content">{file.content}</pre>
    </div>
  );
}

export default function ProofsPanel() {
  const [state, setState] = useState<State>({ kind: 'idle' });
  const [selected, setSelected] = useState<LeanFile | null>(null);

  const run = () => {
    setState({ kind: 'loading' });
    fetchProofs()
      .then(data => {
        setState({ kind: 'done', data });
        const first = data.files.find(f => f.path.endsWith('.lean') && !f.path.includes('lakefile'));
        setSelected(first ?? data.files[0] ?? null);
      })
      .catch((e: unknown) => {
        const msg = e instanceof Error ? e.message : String(e);
        setState({ kind: 'error', message: msg });
      });
  };

  if (state.kind === 'idle') {
    return (
      <div className="proof-idle">
        <p className="proof-idle-desc">
          Generates Lean 4 proof obligation stubs for all loaded contracts and runs <code>lake build</code>.
          All theorems use <code>sorry</code> — this confirms the formal structure compiles, not that the logic is proven.
        </p>
        <button className="proof-run-btn" onClick={run}>
          Build proof obligations
        </button>
      </div>
    );
  }

  if (state.kind === 'loading') {
    return <div className="proof-loading">Running lake build… this may take a moment.</div>;
  }

  if (state.kind === 'error') {
    return <div className="proof-error">Error: {state.message}</div>;
  }

  const { data } = state;

  const CONTRACTS_DIR = 'TautoContracts/contracts/';
  const contractFiles = data.files.filter(f => f.path.startsWith(CONTRACTS_DIR));
  const infraFiles = data.files.filter(f => !f.path.startsWith(CONTRACTS_DIR));

  return (
    <div className="proof-panel">
      <div className="proof-header">
        <BuildBadge data={data} />
        <span className="proof-stats">
          {data.contracts} contract{data.contracts !== 1 ? 's' : ''} ·{' '}
          {data.sorry_count} proof obligation{data.sorry_count !== 1 ? 's' : ''} (sorry-stubbed)
        </span>
        <button className="proof-rerun-btn" onClick={run}>↺ rebuild</button>
      </div>

      {!data.build_available && (
        <div className="proof-warn">
          <code>lake</code> not found in PATH — install Lean 4 via{' '}
          <code>elan</code> to run compilation checks.
        </div>
      )}

      {data.build_stderr && (
        <details className="proof-output">
          <summary>Build output</summary>
          <pre className="proof-output-content">{data.build_stderr}{data.build_stdout}</pre>
        </details>
      )}

      <div className="proof-layout">
        <div className="proof-tree">
          {contractFiles.length > 0 && (
            <>
              <div className="proof-tree-section">Contracts</div>
              {contractFiles.map(f => (
                <button
                  key={f.path}
                  className={`proof-tree-item ${selected?.path === f.path ? 'active' : ''}`}
                  onClick={() => setSelected(f)}
                >
                  {f.path.replace(CONTRACTS_DIR, '')}
                </button>
              ))}
            </>
          )}
          {infraFiles.length > 0 && (
            <>
              <div className="proof-tree-section">Workspace</div>
              {infraFiles.map(f => (
                <button
                  key={f.path}
                  className={`proof-tree-item ${selected?.path === f.path ? 'active' : ''}`}
                  onClick={() => setSelected(f)}
                >
                  {f.path}
                </button>
              ))}
            </>
          )}
        </div>
        <div className="proof-content">
          {selected ? <FileViewer file={selected} /> : <div className="proof-no-select">Select a file</div>}
        </div>
      </div>
    </div>
  );
}
