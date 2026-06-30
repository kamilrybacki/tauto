import type { ContractItem } from '../api/types';

interface Props {
  contracts: ContractItem[];
  selected: ContractItem | null;
  onSelect: (item: ContractItem) => void;
}

export default function ContractList({ contracts, selected, onSelect }: Props) {
  const groups = contracts.reduce<Record<string, ContractItem[]>>((acc, c) => {
    const key = c.entity;
    if (!acc[key]) acc[key] = [];
    acc[key].push(c);
    return acc;
  }, {});

  return (
    <div style={{ padding: '16px', overflowY: 'auto', height: '100%', maxWidth: 640 }}>
      {Object.entries(groups)
        .sort(([a], [b]) => a.localeCompare(b))
        .map(([entity, items]) => (
          <div key={entity} style={{ marginBottom: 28 }}>
            <h3
              style={{
                color: '#6366f1',
                fontSize: 11,
                textTransform: 'uppercase',
                letterSpacing: '0.1em',
                marginBottom: 10,
                fontWeight: 600,
              }}
            >
              {entity}
            </h3>
            {items.map(c => (
              <div
                key={c.key}
                onClick={() => onSelect(c)}
                style={{
                  padding: '10px 14px',
                  marginBottom: 6,
                  borderRadius: 8,
                  cursor: 'pointer',
                  background: selected?.key === c.key ? '#1e1b4b' : '#1f2937',
                  border: `1px solid ${selected?.key === c.key ? '#6366f1' : '#374151'}`,
                  transition: 'background 0.1s, border-color 0.1s',
                }}
              >
                <div style={{ fontSize: 13, fontWeight: 500, color: '#e5e7eb' }}>{c.operation}</div>
                <div style={{ fontSize: 11, color: '#9ca3af', marginTop: 1 }}>{c.case}</div>
                <div style={{ display: 'flex', gap: 10, marginTop: 4, fontSize: 10 }}>
                  {c.requires_count > 0 && (
                    <span style={{ color: '#fbbf24' }}>{c.requires_count} requires</span>
                  )}
                  {c.ensures_count > 0 && (
                    <span style={{ color: '#34d399' }}>{c.ensures_count} ensures</span>
                  )}
                  {c.source && <span style={{ color: '#4b5563', marginLeft: 'auto', fontFamily: 'monospace' }}>{c.source}</span>}
                </div>
              </div>
            ))}
          </div>
        ))}
    </div>
  );
}
