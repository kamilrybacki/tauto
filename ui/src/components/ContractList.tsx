import { useMemo, useState, useEffect } from 'react';
import type { ContractItem, Condition, GraphEdge } from '../api/types';

interface Props {
  contracts: ContractItem[];
  conflicts: GraphEdge[];
  selected: string | null;
  onSelect: (item: ContractItem) => void;
}

interface OpGroup {
  entity: string;
  operation: string;
  rules: ContractItem[];
}

const OP_GLYPH: Record<string, string> = {
  '>=': '≥',
  '<=': '≤',
  '!=': '≠',
  '==': '=',
};

const condText = (c: Condition): string =>
  `${OP_GLYPH[c.operator] ?? c.operator} ${String(c.right.value)}`;

const leftPath = (c: Condition): string => String(c.left.value);

/** Instance prefix of a path (`order.status` → `order`), '' when bare. */
const prefixOf = (path: string): string => (path.includes('.') ? path.split('.')[0] : '');
const shortField = (path: string): string => (path.includes('.') ? path.slice(path.indexOf('.') + 1) : path);

export default function ContractList({ contracts, conflicts, selected, onSelect }: Props) {
  // Group rules: entity → operation (first-seen order within sorted entities).
  const groups = useMemo((): OpGroup[] => {
    const out: OpGroup[] = [];
    const idx = new Map<string, number>();
    for (const c of contracts) {
      const key = `${c.entity}/${c.operation}`;
      const i = idx.get(key);
      if (i === undefined) {
        idx.set(key, out.length);
        out.push({ entity: c.entity, operation: c.operation, rules: [c] });
      } else {
        out[i].rules.push(c);
      }
    }
    return out.sort((a, b) => a.entity.localeCompare(b.entity));
  }, [contracts]);

  const entities = useMemo(() => [...new Set(groups.map((g) => g.entity))], [groups]);

  // Conflict pairs, keyed per rule.
  const conflictKeys = useMemo(() => {
    const s = new Set<string>();
    conflicts.filter((e) => e.kind === 'conflict').forEach((e) => { s.add(e.source); s.add(e.target); });
    return s;
  }, [conflicts]);
  const conflictReason = useMemo(() => {
    const m = new Map<string, string>();
    conflicts
      .filter((e) => e.kind === 'conflict')
      .forEach((e) => {
        if (e.label) {
          m.set(e.source, e.label);
          m.set(e.target, e.label);
        }
      });
    return m;
  }, [conflicts]);

  // Selection: an entity (all its tables) or one operation.
  const [activeEnt, setActiveEnt] = useState<string>('');
  const [activeOp, setActiveOp] = useState<string>(''); // `${entity}/${operation}` or ''
  const [openRule, setOpenRule] = useState<string | null>(null);

  useEffect(() => {
    if (!activeEnt && entities.length) setActiveEnt(entities[0]);
  }, [entities, activeEnt]);

  // Follow external selection (e.g. a click in the graph).
  useEffect(() => {
    if (!selected) return;
    const c = contracts.find((x) => x.key === selected);
    if (c) {
      setActiveEnt(c.entity);
      setActiveOp(`${c.entity}/${c.operation}`);
      setOpenRule(c.key);
    }
  }, [selected, contracts]);

  const visible = groups.filter((g) =>
    activeOp ? `${g.entity}/${g.operation}` === activeOp : g.entity === activeEnt,
  );

  return (
    <div className="explorer">
      <nav className="tree" aria-label="Rules by entity and operation">
        {entities.map((ent) => {
          const ops = groups.filter((g) => g.entity === ent);
          const count = ops.reduce((n, g) => n + g.rules.length, 0);
          return (
            <div key={ent}>
              <button
                className="ent"
                onClick={() => {
                  setActiveEnt(ent);
                  setActiveOp('');
                }}
              >
                {ent} <span className="n">· {count}</span>
              </button>
              {ops.map((g) => {
                const key = `${g.entity}/${g.operation}`;
                const nConf = g.rules.filter((r) => conflictKeys.has(r.key)).length;
                return (
                  <button
                    key={key}
                    className={`op${activeOp === key || (!activeOp && activeEnt === ent) ? ' active' : ''}`}
                    onClick={() => {
                      setActiveEnt(ent);
                      setActiveOp(key);
                    }}
                  >
                    {g.operation}
                    {nConf > 0 && <span className="flag conflict">⊥</span>}
                  </button>
                );
              })}
            </div>
          );
        })}
      </nav>

      <div className="dtables">
        {visible.map((g) => (
          <DecisionTable
            key={`${g.entity}/${g.operation}`}
            group={g}
            conflictKeys={conflictKeys}
            conflictReason={conflictReason}
            openRule={openRule}
            onToggle={(c) => {
              setOpenRule(openRule === c.key ? null : c.key);
              onSelect(c);
            }}
          />
        ))}
      </div>
    </div>
  );
}

function DecisionTable({
  group,
  conflictKeys,
  conflictReason,
  openRule,
  onToggle,
}: {
  group: OpGroup;
  conflictKeys: Set<string>;
  conflictReason: Map<string, string>;
  openRule: string | null;
  onToggle: (c: ContractItem) => void;
}) {
  const { entity, operation, rules } = group;

  // Dominant instance prefix for this entity (e.g. `order`); guard columns from
  // other prefixes are cross-entity references and get flagged.
  const dominant = useMemo(() => {
    const counts = new Map<string, number>();
    rules.forEach((r) =>
      r.requires.forEach((c) => {
        const p = prefixOf(leftPath(c));
        counts.set(p, (counts.get(p) ?? 0) + 1);
      }),
    );
    let best = '';
    let n = -1;
    counts.forEach((v, k) => {
      if (v > n) {
        n = v;
        best = k;
      }
    });
    return best;
  }, [rules]);

  // Guard columns: union of require paths, first-seen order.
  const columns = useMemo(() => {
    const cols: { path: string; label: string; warn: boolean }[] = [];
    const seen = new Set<string>();
    rules.forEach((r) =>
      r.requires.forEach((c) => {
        const path = leftPath(c);
        if (seen.has(path)) return;
        seen.add(path);
        const warn = prefixOf(path) !== dominant && prefixOf(path) !== '';
        cols.push({ path, label: warn ? path : shortField(path), warn });
      }),
    );
    return cols;
  }, [rules, dominant]);

  const nConf = rules.filter((r) => conflictKeys.has(r.key)).length;
  const reason = rules.map((r) => conflictReason.get(r.key)).find(Boolean);
  const warnCols = columns.filter((c) => c.warn);

  return (
    <div className="dt">
      <div className="dt-head">
        <span className="name">
          {entity} · {operation}
        </span>
        <span className="meta">
          {rules.length} rule{rules.length !== 1 ? 's' : ''}
          {nConf > 0 && ` · ${nConf} in conflict`}
        </span>
      </div>
      <div className="dt-scroll">
        <table className="decision">
          <thead>
            <tr className="grouprow">
              <th rowSpan={2}>Rule</th>
              <th colSpan={Math.max(columns.length, 1)} className="group">
                When — before <code>{operation}</code> may run
              </th>
              <th rowSpan={2} className="out">
                Then — the result of <code>{operation}</code>
              </th>
            </tr>
            <tr>
              {columns.length ? (
                columns.map((c) => (
                  <th key={c.path} className={c.warn ? 'warncol' : ''}>
                    {c.label}
                  </th>
                ))
              ) : (
                <th className="anycol">always</th>
              )}
            </tr>
          </thead>
          <tbody>
            {rules.map((r) => {
              const isConf = conflictKeys.has(r.key);
              const open = openRule === r.key;
              return (
                <RuleRows
                  key={r.key}
                  rule={r}
                  columns={columns}
                  conflict={isConf}
                  open={open}
                  onToggle={() => onToggle(r)}
                />
              );
            })}
          </tbody>
        </table>
      </div>
      {nConf > 0 && (
        <p className="dt-note red">⊥ {reason ?? 'Rules in this table have contradictory outcomes for the same guards.'}</p>
      )}
      {warnCols.length > 0 && (
        <p className="dt-note red">
          ⚠ {warnCols.map((c) => c.path).join(', ')} reference{warnCols.length === 1 ? 's' : ''} another
          entity&rsquo;s state — likely should use a snapshot field on {entity}.
        </p>
      )}
    </div>
  );
}

function RuleRows({
  rule: r,
  columns,
  conflict,
  open,
  onToggle,
}: {
  rule: ContractItem;
  columns: { path: string; label: string; warn: boolean }[];
  conflict: boolean;
  open: boolean;
  onToggle: () => void;
}) {
  const span = columns.length + 2;
  return (
    <>
      <tr className={conflict ? 'conflict-row' : ''} onClick={onToggle}>
        <td className="rule">{r.case}</td>
        {columns.map((col) => {
          const conds = r.requires.filter((c) => leftPath(c) === col.path);
          return (
            <td key={col.path}>
              {conds.length ? conds.map(condText).join(' ∧ ') : <span className="any">—</span>}
            </td>
          );
        })}
        <td className="out">
          {r.ensures.length ? (
            r.ensures.map((e, i) => (
              <div key={i}>
                {shortField(leftPath(e))} {condText(e)}
              </div>
            ))
          ) : (
            <span className="any">—</span>
          )}
        </td>
      </tr>
      {open && (
        <tr className="detail-row">
          <td colSpan={span}>
            {r.intent && (
              <div>
                <span className="d-label">Intent</span> {r.intent}
              </div>
            )}
            {r.forbidden.length > 0 && (
              <div className="d-red">
                <span className="d-label">Forbidden</span>
                <code>
                  {r.forbidden
                    .map((f) => `${f.operation}(${(f.args ?? []).map((a) => String(a.value)).join(', ')})`)
                    .join(', ')}
                </code>
              </div>
            )}
            {r.preserves.length > 0 && (
              <div>
                <span className="d-label">Preserves</span> <code>{r.preserves.join(', ')}</code>
              </div>
            )}
            {r.assumes.length > 0 && (
              <div>
                <span className="d-label">Assumes</span> {r.assumes.join('; ')}
              </div>
            )}
            {r.source && (
              <div>
                <span className="d-label">Source</span> <code>{r.source}</code>
              </div>
            )}
          </td>
        </tr>
      )}
    </>
  );
}
