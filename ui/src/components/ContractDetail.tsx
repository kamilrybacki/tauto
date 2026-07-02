import type { ContractItem, Condition } from '../api/types';

function renderCond(c: Condition): string {
  return `${String(c.left.value)} ${c.operator} ${String(c.right.value)}`;
}

function Section({ label, color, items }: { label: string; color: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <section style={{ marginBottom: 14 }}>
      <h4 style={{ color, fontSize: 10, textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: 6 }}>
        {label}
      </h4>
      {items.map((item, i) => (
        <div key={i} style={{ fontFamily: 'monospace', fontSize: 12, color: '#d1d5db', padding: '2px 0', lineHeight: 1.5 }}>
          {item}
        </div>
      ))}
    </section>
  );
}

interface Props {
  contract: ContractItem;
  onClose: () => void;
}

export default function ContractDetail({ contract: c, onClose }: Props) {
  return (
    <div
      className="contract-detail"
      style={{
        width: 300,
        minWidth: 300,
        height: '100%',
        background: '#0f172a',
        borderLeft: '1px solid #1f2937',
        overflowY: 'auto',
        padding: 16,
        display: 'flex',
        flexDirection: 'column',
        gap: 0,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 16 }}>
        <div>
          <div style={{ fontSize: 10, color: '#6366f1', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
            {c.entity}
          </div>
          <h2 style={{ color: '#f9fafb', fontSize: 15, fontWeight: 600, marginTop: 3 }}>
            {c.operation}
          </h2>
          <div style={{ fontSize: 11, color: '#6b7280', marginTop: 2 }}>case: {c.case}</div>
        </div>
        <button
          onClick={onClose}
          style={{
            background: 'none',
            border: 'none',
            color: '#6b7280',
            cursor: 'pointer',
            fontSize: 16,
            lineHeight: 1,
            padding: 2,
          }}
          aria-label="Close"
        >
          ✕
        </button>
      </div>

      {c.source && (
        <div style={{ fontSize: 10, color: '#4b5563', marginBottom: 16, fontFamily: 'monospace' }}>
          {c.source}
        </div>
      )}

      <Section label="requires" color="#fbbf24" items={c.requires.map(renderCond)} />
      <Section label="ensures" color="#34d399" items={c.ensures.map(renderCond)} />
      <Section label="forbidden" color="#f87171" items={c.forbidden.map(f => f.operation)} />
      <Section label="preserves" color="#a78bfa" items={c.preserves} />
      <Section label="assumes" color="#7dd3fc" items={c.assumes} />

      {c.requires.length === 0 && c.ensures.length === 0 && c.forbidden.length === 0 && (
        <div style={{ color: '#4b5563', fontSize: 12, fontStyle: 'italic' }}>No conditions defined.</div>
      )}
    </div>
  );
}
