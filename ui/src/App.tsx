import { useState, useEffect } from 'react';
import { fetchContracts, fetchGraph, fetchHistory, fetchProjects, setProject } from './api/client';
import type { ContractsResponse, ContractItem, GraphResponse, HistoryEntry, ProjectInfo } from './api/types';
import ContractGraph from './components/ContractGraph';
import ContractList from './components/ContractList';
import HistoryPanel from './components/HistoryPanel';
import ProofsPanel from './components/ProofsPanel';
import CheckPanel from './components/CheckPanel';
import StateMachinePanel from './components/StateMachinePanel';

type View = 'list' | 'graph' | 'states' | 'proofs' | 'check' | 'history';

const SECTIONS: { view: View; title: string; desc: React.ReactNode }[] = [
  {
    view: 'list',
    title: 'Rules',
    desc: (
      <>
        Rules that share an operation form one decision table — guards line up as columns, the
        outcome sits at the end. Contradictory rows are flagged <span className="bot">⊥</span>; click
        a row for its intent and clauses.
      </>
    ),
  },
  {
    view: 'graph',
    title: 'Flow',
    desc: (
      <>
        The workflow as ordered stages. Each operation lists what must hold elsewhere before it can
        fire (gates derived from snapshot guards); <span className="bot">⊥</span> marks operations
        with contradictory rules. Tap one to open its decision table.
      </>
    ),
  },
  {
    view: 'states',
    title: 'Lifecycles',
    desc: (
      <>
        Each entity&rsquo;s states, one row per state, with the transitions its rules allow —{' '}
        <span className="bot">isolated</span> states have no rule at all. Every transition links to
        the rule behind it.
      </>
    ),
  },
  {
    view: 'proofs',
    title: 'Proofs',
    desc: (
      <>
        Each contract compiled to a Lean&nbsp;4 theorem; satisfiability and conflict obligations are
        discharged by <code>decide</code> / <code>omega</code>.
      </>
    ),
  },
  {
    view: 'check',
    title: 'Check',
    desc: <>Submit a proposed rule. It is checked against the set for conflicts, dead preconditions, and its own stated intent — nothing is saved.</>,
  },
  { view: 'history', title: 'History', desc: <>Every submission, newest first.</> },
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

  if (error)
    return (
      <div className="error">
        <div style={{ textAlign: 'center' }}>
          <div>Failed to load the rule set.</div>
          <div className="detail">{error}</div>
        </div>
      </div>
    );
  if (!contracts || !graph) return <div className="loading">Loading…</div>;

  const active = SECTIONS.find((s) => s.view === view)!;

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">
          tauto<span className="dot">.</span>
        </span>
        <span className="tagline">business-rule contracts, mechanically checked</span>
        <span className="spacer" />
        <span className="stat">
          {contracts.contracts} rule{contracts.contracts !== 1 ? 's' : ''}
        </span>
        {projects.length > 1 && (
          <select
            className="project-select"
            value={project}
            onChange={(e) => setProjectSel(e.target.value)}
            aria-label="Project"
          >
            {projects.map((p) => (
              <option key={p.slug} value={p.slug}>
                {p.slug} ({p.contracts})
              </option>
            ))}
          </select>
        )}
      </header>

      <nav className="tabs" aria-label="Sections">
        {SECTIONS.map((s) => (
          <button key={s.view} aria-current={view === s.view} onClick={() => setView(s.view)}>
            {s.title}
            {s.view === 'history' && history.length > 0 && <span className="nav-count">{history.length}</span>}
          </button>
        ))}
      </nav>

      <main className="content">
        <div className="section-head">
          <h2>{active.title}</h2>
        </div>
        <p className="section-desc">{active.desc}</p>

        {view === 'graph' && (
          <ContractGraph
            graph={graph}
            contracts={contracts.items}
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
          />
        )}
        {view === 'states' && <StateMachinePanel key={project} onOpenRule={selectByKey} />}
        {view === 'proofs' && <ProofsPanel key={project} contracts={contracts.items} />}
        {view === 'check' && <CheckPanel />}
        {view === 'history' && <HistoryPanel entries={history} />}
      </main>
    </div>
  );
}
