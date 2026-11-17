import { requestClient } from '#/api/request';

import { infraGet, infraList } from './compat';

export namespace ScanRiskApi {
  export interface Risk { id: string; rule_id?: string; asset_ip: string; port: number; severity: string; description: string; solution?: string; status: string; created_at?: string; updated_at?: string; assigned_to?: string; evidence?: Record<string, unknown>; source?: string; first_seen?: string; last_seen?: string; }
}
export function getRiskList() { return infraList<ScanRiskApi.Risk>('risk'); }
export function getRisk(id: string) { return infraGet<ScanRiskApi.Risk>('risk', id); }
export function updateRisk(id: string, data: { status?: string; assigned_to?: string; solution?: string }) {
  if (data.status === 'resolved') return requestClient.put(`/infra/risk/${id}/resolve`);
  return requestClient.put(`/infra/risk/${id}/status/${data.status ?? 'open'}`);
}
