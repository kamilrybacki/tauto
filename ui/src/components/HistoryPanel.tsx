import type { HistoryEntry } from '../api/types';

interface HistoryPanelProps {
  entries: HistoryEntry[];
}

const caseOf = (key: string): string => key.split('/').pop() ?? key;
const formatTime = (unix: number): string => new Date(unix * 1000).toLocaleString();

export default function HistoryPanel({ entries }: HistoryPanelProps) {
  if (entries.length === 0) {
    return (
      <p className="empty-note">
        No revisions yet. Submitted rules (via the referee report, or the upload API) are recorded here,
        newest first.
      </p>
    );
  }

  const ordered = [...entries].sort((a, b) => b.id - a.id);

  return (
    <div className="revlist">
      {ordered.map((entry, i) => (
        <div key={entry.id} className="rev">
          <span className="revn">Rev. {ordered.length - i}</span>
          <span className="file">{entry.filename}</span>
          <span className={`outcome ${entry.outcome}`}>{entry.outcome}</span>
          <span className="meta">
            {entry.contracts_count} contract{entry.contracts_count !== 1 ? 's' : ''}
            {entry.parse_errors > 0 && ` · ${entry.parse_errors} parse error${entry.parse_errors !== 1 ? 's' : ''}`}
          </span>
          <span className="time">{formatTime(entry.timestamp_unix)}</span>
          {entry.conflicts.map((c, j) => (
            <div className="rev-bot" key={j}>
              ⊥ barred by {caseOf(c.key_a)} ↔ {caseOf(c.key_b)} — {c.reason}
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
