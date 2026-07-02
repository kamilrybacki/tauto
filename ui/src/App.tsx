import { useState, useEffect } from 'react';
import { fetchContracts, fetchGraph, fetchHistory, fetchProjects, setProject } from './api/client';
import type { ContractsResponse, ContractItem, GraphResponse, HistoryEntry, ProjectInfo } from './api/types';
import ContractGraph from './components/ContractGraph';
import ContractList from './components/ContractList';
import HistoryPanel from './components/HistoryPanel';
import ProofsPanel from './components/ProofsPanel';
import CheckPanel from './components/CheckPanel';
import StateMachinePanel from './components/StateMachinePanel';

type View = 'graph' | 'list' | 'states' | 'proofs' | 'check' | 'history';

const SECTIONS: { view: View; num: number; title: string; intro: React.ReactNode }[] = [
  {
    view: 'graph',
    num: 1,
    title: 'Dependency graph',
    intro: (
      <>
        The corpus drawn as a graph. Nodes are contracts grouped by entity and operation; rules
        sharing an operation are joined, and candidate contradictions are dashed in{' '}
        <span className="bot">red</span>.
      </>
    ),
  },
  {
    view: 'list',
    num: 2,
    title: 'Propositions',
    intro: (
      <>
        Each contract in the corpus is stated below as a proposition over its entity. Conflicting
        pairs are marked <span className="bot">⊥</span> in the margin.
      </>
    ),
  },
  {
    view: 'states',
    num: 3,
    title: 'Lifecycles',
    intro: (
      <>
        Where a glossary marks an enum field as a state, the rules form a state machine. Each figure
        traces one entity&rsquo;s lifecycle; states no rule reaches are flagged.
      </>
    ),
  },
  {
    view: 'proofs',
    num: 4,
    title: 'Theorems',
    intro: (
      <>
        Every contract induces proof obligations in a Lean&nbsp;4 workspace. Building with{' '}
        <code>lake</code> machine-checks each theorem; a clean build closes them with ∎.
      </>
    ),
  },
  {
    view: 'check',
    num: 5,
    title: 'Referee report',
    intro: (
      <>
        Submit a proposed rule for review. It is checked for contradiction with the corpus, for dead
        preconditions, and against its own stated intent — nothing is saved.
      </>
    ),
  },
  {
    view: 'history',
    num: 6,
    title: 'Revision history',
    intro: <>Every submission, newest first. Rejected revisions record the contradiction that barred them.</>,
  },
];

export default function App() {
  const [contracts, setContracts] = useState<ContractsResponse | null>(null);
  const [graph, setGraph] = useState<GraphResponse | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [selected, setSelected] = useState<ContractItem | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<View>('list');
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [project, setProjectSel] = useState<string>('');

  useEffect(() => {
    fetchProjects()
      .then((r) => {
        setProjects(r.projects);
        setProjectSel(r.default_project);
      })
      .catch((e: Error) => setError(e.message));
  }, []);

  useEffect(() => {
    if (!project) return;
    setProject(project);
    setContracts(null);
    setSelected(null);
    Promise.all([fetchContracts(), fetchGraph(), fetchHistory()])
      .then(([c, g, h]) => {
        setContracts(c);
        setGraph(g);
        setHistory(h.entries);
      })
      .catch((e: Error) => setError(e.message));
  }, [project]);

  const selectByKey = (key: string) => {
    const item = contracts?.items.find((c) => c.key === key) ?? null;
    setSelected(item);
    setView('list');
  };

  if (error) {
    return (
      <div className="error">
        <div style={{ textAlign: 'center' }}>
          <div>Failed to load the corpus.</div>
          <div className="detail">{error}</div>
        </div>
      </div>
    );
  }
  if (!contracts || !graph) return <div className="loading">Typesetting the corpus…</div>;

  const active = SECTIONS.find((s) => s.view === view)!;
  const volumeLabel = project ? `Vol. ${projects.findIndex((p) => p.slug === project) + 1} — ${project}` : '';

  return (
    <div className="app">
      <header className="running-header">
        <span className="running-title">tauto · business-rule contracts</span>
        <span className="running-volume">
          <span>running volume</span>
          {projects.length > 1 ? (
            <select
              className="project-select"
              value={project}
              onChange={(e) => setProjectSel(e.target.value)}
              aria-label="Project (volume)"
            >
              {projects.map((p, i) => (
                <option key={p.slug} value={p.slug}>
                  Vol. {i + 1} — {p.slug} ({p.contracts})
                </option>
              ))}
            </select>
          ) : (
            <span style={{ fontStyle: 'normal', color: 'var(--ink)' }}>{volumeLabel}</span>
          )}
        </span>
      </header>

      <div className="page">
        <div className="masthead">
          <h1>tauto</h1>
          <p className="subtitle">A working notebook of business-rule contracts, mechanically checked</p>
          <p className="stats">
            {contracts.contracts} contract{contracts.contracts !== 1 ? 's' : ''} · {contracts.files} source
            file{contracts.files !== 1 ? 's' : ''}
          </p>
        </div>

        <nav className="contents" aria-label="Contents">
          {SECTIONS.map((s) => (
            <button
              key={s.view}
              aria-current={view === s.view}
              onClick={() => setView(s.view)}
            >
              <span className="secnum">§{s.num}</span>
              {s.title}
              {s.view === 'history' && history.length > 0 && <span className="nav-count">{history.length}</span>}
            </button>
          ))}
        </nav>

        <section className="section" aria-labelledby="sec-heading" data-screen-label={active.title}>
          <h2 id="sec-heading">
            <span className="secnum">§{active.num}</span>
            {active.title}
          </h2>
          <p className="prose">{active.intro}</p>

          {view === 'graph' && (
            <ContractGraph
              graph={graph}
              slug={project}
              selected={selected?.key ?? null}
              onSelect={selectByKey}
            />
          )}
          {view === 'list' && (
            <ContractList
              contracts={contracts.items}
              conflicts={graph.edges}
              selected={selected?.key ?? null}
              onSelect={(item) => setSelected(item)}
              onJump={selectByKey}
            />
          )}
          {view === 'states' && <StateMachinePanel key={project} />}
          {view === 'proofs' && <ProofsPanel key={project} />}
          {view === 'check' && <CheckPanel />}
          {view === 'history' && <HistoryPanel entries={history} />}
        </section>
      </div>

      <footer className="folio">
        <span>tauto · Rust core · Lean 4 proof workspace</span>
        <span>Vol. {Math.max(1, projects.findIndex((p) => p.slug === project) + 1)} · §{active.num}</span>
      </footer>
    </div>
  );
}
