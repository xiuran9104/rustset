import {
  infraCreate,
  infraDelete,
  infraGet,
  infraList,
  infraUpdate,
} from './compat';
import { requestClient } from '#/api/request';

export interface NetworkPolicy {
  id?: number;
  firewall_name: string;
  destination_organization: string;
  destination_project: string;
  source_organization: string;
  source_project: string;
  source_security_zone: string;
  source_ip: string;
  destination_security_zone: string;
  destination_ip: string;
  service_port: string;
  applicant: string;
  application_date: string;
  traffic_direction: string;
  action: string;
  implementer?: string;
  implementation_date?: string;
  delivery_date?: string;
  create_time?: string;
  update_time?: string;
  high_risk_count?: number;
}

export interface NetworkPolicyRiskMatchSummary {
  policiesChecked: number;
  warningsMatched: number;
}

const resource = 'network-policy';

function normalizeDates(data: NetworkPolicy) {
  const payload: Record<string, unknown> = { ...data };
  for (const field of [
    'application_date',
    'implementation_date',
    'delivery_date',
  ]) {
    if (payload[field] === '') payload[field] = null;
  }
  return payload;
}

export function getNetworkPolicyList() {
  return infraList<NetworkPolicy>(resource);
}

export function getNetworkPolicy(id: number) {
  return infraGet<NetworkPolicy>(resource, id);
}

export function createNetworkPolicy(data: NetworkPolicy) {
  return infraCreate(resource, normalizeDates(data));
}

export function updateNetworkPolicy(id: number, data: NetworkPolicy) {
  return infraUpdate(resource, id, normalizeDates(data));
}

export function deleteNetworkPolicy(id: number) {
  return infraDelete(resource, id);
}

export function recheckNetworkPolicyRisks() {
  return requestClient.post<NetworkPolicyRiskMatchSummary>(
    '/infra/network-policy/recheck-risks',
  );
}

export function exportNetworkPolicyCsv() {
  return requestClient.download('/infra/network-policy/export-csv');
}

export function downloadNetworkPolicyImportTemplate() {
  return requestClient.download('/infra/network-policy/import-template');
}

export function importNetworkPolicyCsv(file: File) {
  return requestClient.upload('/infra/network-policy/import-csv', { file });
}
