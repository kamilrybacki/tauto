import type { CheckResponse, ContractsResponse, GraphResponse, HistoryResponse, ProofsResponse, StateCoverage } from './types';

async function get<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(`${res.status} ${res.statusText}${body ? ': ' + body : ''}`);
  }
  return res.json() as Promise<T>;
}

export const fetchContracts = (): Promise<ContractsResponse> =>
  get<ContractsResponse>('/api/v1/contracts');

export const fetchGraph = (): Promise<GraphResponse> =>
  get<GraphResponse>('/api/v1/graph');

export const fetchHistory = (): Promise<HistoryResponse> =>
  get<HistoryResponse>('/api/v1/history');

export const fetchProofs = (): Promise<ProofsResponse> =>
  get<ProofsResponse>('/api/v1/proofs');

export const fetchLifecycle = (): Promise<StateCoverage[]> =>
  get<StateCoverage[]>('/api/v1/lifecycle');

export async function checkRule(content: string): Promise<CheckResponse> {
  const res = await fetch('/api/v1/check', {
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
