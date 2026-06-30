import { useState, useEffect } from 'react';
import { fetchContracts, fetchGraph } from './api/client';
import type { ContractsResponse, ContractItem, GraphResponse } from './api/types';
import ContractGraph from './components/ContractGraph';
import ContractList from './components/ContractList';
import ContractDetail from './components/ContractDetail';

type View = 'graph' | 'list';

export default function App() {
  const [contracts, setContracts] = useState<ContractsResponse | null>(null);
  const [graph, setGraph] = useState<GraphResponse | null>(null);
  const [selected, setSelected] = useState<ContractItem | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<View>('graph');

  useEffect(() => {
    Promise.all([fetchContracts(), fetchGraph()])
      .then(([c, g]) => { setContracts(c); setGraph(g); })
      .catch((e: Error) => setError(e.message));
  }, []);

  const handleSelectById = (id: string) => {
    const item = contracts?.items.find(c => c.key === id) ?? null;
    setSelected(item);
  };

  if (error) {
    return (
      <div className="error">
        <div>
          <div style={{ marginBottom: 8, fontSize: 16 }}>Failed to load contracts</div>
          <div style={{ fontSize: 12, color: '#6b7280' }}>{error}</div>
          <div style={{ fontSize: 11, color: '#4b5563', marginTop: 12 }}>
            Make sure <code style={{ fontFamily: 'monospace', color: '#9ca3af' }}>tauto serve</code> is running.
          </div>
        </div>
      </div>
    );
  }

  if (!contracts || !graph) {
    return <div className="loading">Loading contracts…</div>;
  }

  return (
    <div className="app">
      <header className="header">
        <h1>
          tauto <span className="subtitle">— business logic explorer</span>
        </h1>
        <div className="stats">
          {contracts.contracts} contract{contracts.contracts !== 1 ? 's' : ''} ·{' '}
          {contracts.files} file{contracts.files !== 1 ? 's' : ''}
        </div>
        <nav className="nav">
          <button className={view === 'graph' ? 'active' : ''} onClick={() => setView('graph')}>
            Graph
          </button>
          <button className={view === 'list' ? 'active' : ''} onClick={() => setView('list')}>
            List
          </button>
        </nav>
      </header>

      <main className="main">
        {view === 'graph' ? (
          <div className="graph-layout">
            <ContractGraph
              graph={graph}
              selected={selected?.key ?? null}
              onSelect={handleSelectById}
            />
            {selected && (
              <ContractDetail contract={selected} onClose={() => setSelected(null)} />
            )}
          </div>
        ) : (
          <ContractList
            contracts={contracts.items}
            selected={selected}
            onSelect={item => setSelected(item)}
          />
        )}
      </main>
    </div>
  );
}
