import { requestClient } from '#/api/request';

import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanAssetApi {
  export interface Asset {
    id?: number; name: string; ip: string; zone: string; ports: any[];
    last_scanned?: string; contact_person?: string; contact_phone?: string;
    owner?: string; weight: number; labels: string[]; os?: string; device_type?: string;
    url?: string; language?: string; finding_count?: number; endpoint_count?: number;
    city?: string; district?: string; organization_name?: string;
    business_department?: string; department_contact?: string;
    application_name?: string; server_name?: string;
    hardware_configuration?: string; operating_system?: string;
    database_type?: string; launch_date?: string; decommission_date?: string;
    application_type?: string; network_environment?: string;
    internet_ipv4?: string; internet_ipv6?: string; domain_address?: string;
    internal_network_ip?: string; government_extranet_ip?: string;
    open_ports?: string; publishing_endpoint?: string;
    publishes_other_endpoint?: boolean; other_endpoint_name?: string;
    security_product_installation?: string; development_vendor?: string;
    development_vendor_contact?: string; security_vendor?: string;
    security_vendor_contact?: string; operations_vendor?: string;
    operations_vendor_contact?: string; classified_protection_level?: string;
    classified_protection_assessed?: boolean;
    classified_protection_assessor?: string;
    classified_protection_assessment_date?: string;
    classified_protection_score?: number;
    classified_protection_filed?: boolean;
    classified_protection_filing_date?: string;
    classified_protection_filing_number?: string;
    classified_protection_filing_authority?: string;
    cryptography_assessed?: boolean;
    cryptography_assessment_level?: string;
    cryptography_assessment_date?: string;
    cryptography_assessment_number?: string;
  }
}

const dateFields: (keyof ScanAssetApi.Asset)[] = [
  'launch_date',
  'decommission_date',
  'classified_protection_assessment_date',
  'classified_protection_filing_date',
  'cryptography_assessment_date',
];

function parseArray(value: unknown) {
  if (Array.isArray(value)) return value;
  if (typeof value !== 'string' || !value) return [];
  try {
    const parsed = JSON.parse(value);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function normalizeAsset(row: ScanAssetApi.Asset): ScanAssetApi.Asset {
  return {
    ...row,
    ports: parseArray(row.ports),
    labels: parseArray(row.labels),
  };
}

export async function getAssetList() {
  const rows = await infraList<ScanAssetApi.Asset>('asset');
  return rows.map((row) => ({
    ...normalizeAsset(row),
    editable: true,
    record_id: row.id,
    source_label: '本地资产',
    source_type: 'scan',
    status: 'active',
  }));
}
// infra_asset stores ports/labels as text (JSON strings); the page model
// works with arrays, so serialize collections on write.
function serializeCollections(data: ScanAssetApi.Asset) {
  const payload: Record<string, unknown> = {
    ...data,
    ports: Array.isArray(data.ports) ? JSON.stringify(data.ports) : data.ports,
    labels: Array.isArray(data.labels) ? JSON.stringify(data.labels) : data.labels,
  };
  for (const field of dateFields) {
    if (payload[field] === '') payload[field] = null;
  }
  return payload;
}
export async function getAsset(id: number) {
  return normalizeAsset(await infraGet<ScanAssetApi.Asset>('asset', id));
}
export function createAsset(data: ScanAssetApi.Asset) { return infraCreate('asset', serializeCollections(data)); }
export function updateAsset(id: number, data: ScanAssetApi.Asset) { return infraUpdate('asset', id, serializeCollections(data)); }
export function deleteAsset(id: number) { return infraDelete('asset', id); }
export function exportAssetCsv() { return requestClient.download('/infra/asset/export-csv'); }
export function downloadAssetImportTemplate() { return requestClient.download('/infra/asset/import-template'); }
export function importAssetCsv(file: File) { return requestClient.upload('/infra/asset/import-csv', { file }); }
