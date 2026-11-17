import { requestClient } from '#/api/request';

import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanCloudProviderConfigApi {
  export interface CloudProviderConfig {
    id?: number; platform_id: number; provider: string;
    region_id: string; region_name: string; available_zones: string[];
    account_name: string; access_key_id: string; access_key_secret: string;
    auth_type: string; auth_username?: string; auth_password?: string;
    auth_domain?: string; project_id?: string;
    status: string; remarks?: string; last_test_time?: string; last_test_result?: string;
    created_at: string; updated_at?: string;
  }
}
export function getCloudProviderConfigList() { return infraList<ScanCloudProviderConfigApi.CloudProviderConfig>('cloud-provider-config'); }
export function getCloudProviderConfig(id: number) { return infraGet<ScanCloudProviderConfigApi.CloudProviderConfig>('cloud-provider-config', id); }
export function createCloudProviderConfig(data: ScanCloudProviderConfigApi.CloudProviderConfig) { return infraCreate('cloud-provider-config', data); }
export function updateCloudProviderConfig(id: number, data: ScanCloudProviderConfigApi.CloudProviderConfig) { return infraUpdate('cloud-provider-config', id, data); }
export function deleteCloudProviderConfig(id: number) { return infraDelete('cloud-provider-config', id); }
export function testCloudConnection(configId: number) { return requestClient.post('/infra/cloud-provider-config/test-connection', undefined, { params: { id: configId }, timeout: 600_000 }); }
