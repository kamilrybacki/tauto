import type {
  CheckResponse,
  ContractsResponse,
  GraphResponse,
  HistoryResponse,
  ProjectsResponse,
  ProofsResponse,
  ReportResponse,
  StateCoverage,
} from './types';

// The selected project is threaded onto every request as `?project=<slug>`.
// Empty means "let the server pick the default" (and back-compat single-project).
let currentProject = '';

export function setProject(slug: string): void {
  currentProject = slug;
}

export function getProject(): string {
  return currentProject;
}

function withProject(path: string): string {
  if (!currentProject) {
    return path;
  }
  const sep = path.includes('?') ? '&' : '?';
  return `${path}${sep}project=${encodeURIComponent(currentProject)}`;
}

async function get<T>(path: string): Promise<T> {
  const res = await fetch(withProject(path));
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`${res.status} ${res.statusText}${body ? ': ' + body : ''}`);
  }
  return res.json() as Promise<T>;
}

export const fetchProjects = (): Promise<ProjectsResponse> =>
  // Not project-scoped — lists them.
  fetch('/api/v1/projects').then((res) => {
    if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
    return res.json() as Promise<ProjectsResponse>;
  });

export const fetchContracts = (): Promise<ContractsResponse> =>
  get<ContractsResponse>('/api/v1/contracts');

export const fetchGraph = (): Promise<GraphResponse> =>
  get<GraphResponse>('/api/v1/graph');

export const fetchHistory = (): Promise<HistoryResponse> =>
  get<HistoryResponse>('/api/v1/history');

export const fetchProofs = (): Promise<ProofsResponse> =>
  get<ProofsResponse>('/api/v1/proofs');

export const fetchReport = (): Promise<ReportResponse> =>
  get<ReportResponse>('/api/v1/report');

export const fetchLifecycle = (): Promise<StateCoverage[]> =>
  get<StateCoverage[]>('/api/v1/lifecycle');

export async function checkRule(content: string): Promise<CheckResponse> {
  const res = await fetch(withProject('/api/v1/check'), {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain' },
    body: content,
  });
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`${res.status} ${res.statusText}${body ? ': ' + body : ''}`);
  }
  return res.json() as Promise<CheckResponse>;
}
