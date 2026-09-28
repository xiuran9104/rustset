import { requestClient } from '#/api/request';
import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanCloudResourceApi {
  export interface CloudResource {
    id?: number;
    ecs_name: string;
    ecs_status: string;
    resource_id?: string;
    cloud_region: string;
    cloud_category: string;
    cloud_provider_config_id?: number;
    zone_name?: string;
    platform_name?: string;
    county_city?: string;
    vdc_name?: string;
    customer_name: string;
    application_name?: string;
    contract_name?: string;
    instance_id?: string;
    ecs_type?: string;
    ecs_os?: string;
    cpu_cores: number;
    memory_gb: number;
    system_disk?: string;
    system_disk_size_gb?: number;
    data_disk?: string;
    has_security_product: boolean;
    ip_address: string;
    remarks?: string;
    application_status?: string;
    delivery_status?: string;
  }
}

export function getCloudResourceList() {
  return infraList<ScanCloudResourceApi.CloudResource>('cloud-resource');
}
export function getCloudResource(id: number) {
  return infraGet<ScanCloudResourceApi.CloudResource>('cloud-resource', id);
}
export function createCloudResource(data: ScanCloudResourceApi.CloudResource) {
  return infraCreate('cloud-resource', data);
}
export function updateCloudResource(
  id: number,
  data: ScanCloudResourceApi.CloudResource,
) {
  return infraUpdate('cloud-resource', id, data);
}
export function deleteCloudResource(id: number) {
  return infraDelete('cloud-resource', id);
}
export function discoverCloudAssets() {
  return requestClient.post('/infra/cloud-asset/discover');
}
