import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanPhysicalResourceApi {
  export interface PhysicalResource {
    id?: number;
    ecs_name: string;
    ecs_status: string;
    cloud_region: string;
    cloud_category?: string;
    customer_name: string;
    machine_room_id?: number;
    cloud_platform_id?: number;
    cabinet_no?: string;
    rack_start_u?: number;
    rack_end_u?: number;
    deployment_type?: 'cloud_managed' | 'datacenter_managed' | 'standalone';
    management_ip?: string;
    business_ip?: string;
    network_cidr?: string;
    gateway?: string;
    vlan_id?: string;
    dns_servers?: string;
    mac_address?: string;
    serial_number?: string;
    hardware_model?: string;
    warranty_expiry?: string;
    ipmi_address?: string;
    project_name?: string;
    contract_name?: string;
    supplier_name?: string;
    cpu_cores: number;
    memory_gb: number;
    has_security_product: boolean;
    remarks?: string;
    application_status?: string;
    delivery_status?: string;
  }
}

export function getPhysicalResourceList() {
  return infraList<ScanPhysicalResourceApi.PhysicalResource>('physical-resource');
}
export function getPhysicalResource(id: number) {
  return infraGet<ScanPhysicalResourceApi.PhysicalResource>('physical-resource', id);
}
export function createPhysicalResource(
  data: ScanPhysicalResourceApi.PhysicalResource,
) {
  return infraCreate('physical-resource', data);
}
export function updatePhysicalResource(
  id: number,
  data: ScanPhysicalResourceApi.PhysicalResource,
) {
  return infraUpdate('physical-resource', id, data);
}
export function deletePhysicalResource(id: number) {
  return infraDelete('physical-resource', id);
}
