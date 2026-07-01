import type { HistoryEntry } from '../api/types';

interface HistoryPanelProps {
  entries: HistoryEntry[];
}

function formatTime(unix: number): string {
  return new Date(unix * 1000).toLocaleString();
}

function OutcomeBadge({ outcome }: { outcome: HistoryEntry['outcome'] }) {
  const accepted = outcome === 'accepted';
  return (
    <span className={`history-badge ${accepted ? 'history-badge--accepted' : 'history-badge--rejected'}`}>
      {accepted ? 'accepted' : 'rejected'}
    </span>
  );
}

export default function HistoryPanel({ entries }: HistoryPanelProps) {
  if (entries.length === 0) {
    return (
      <div className="history-empty">
        No uploads yet. Use the upload API to add contract files.
      </div>
    );
  }

  return (
    <div className="history-panel">
      <div className="history-list">
        {entries.map(entry => (
          <div key={entry.id} className={`history-entry history-entry--${entry.outcome}`}>
            <div className="history-entry-header">
              <OutcomeBadge outcome={entry.outcome} />
              <span className="history-filename">{entry.filename}</span>
              <span className="history-time">{formatTime(entry.timestamp_unix)}</span>
            </div>
            <div className="history-meta">
              {entry.contracts_count} contract{entry.contracts_count !== 1 ? 's' : ''}
              {entry.parse_errors > 0 && (
                <span className="history-parse-errors"> · {entry.parse_errors} parse error{entry.parse_errors !== 1 ? 's' : ''}</span>
              )}
            </div>
            {entry.conflicts.length > 0 && (
              <div className="history-conflicts">
                {entry.conflicts.map((c, i) => (
                  <div key={i} className="history-conflict-item">
                    <span className="history-conflict-keys">{c.key_a} ↔ {c.key_b}</span>
                    <span className="history-conflict-reason">{c.reason}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
