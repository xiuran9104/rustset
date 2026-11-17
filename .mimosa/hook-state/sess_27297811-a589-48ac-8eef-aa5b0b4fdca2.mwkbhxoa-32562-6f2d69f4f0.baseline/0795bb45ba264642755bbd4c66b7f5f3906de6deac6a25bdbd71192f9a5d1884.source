import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanSecurityProductApi {
  export interface SecurityProduct {
    id?: number; name: string; category: string; vendor: string;
    model: string; version: string; serial_number?: string;
    license_type: string; license_expiry?: string; management_ip?: string;
    deployment_mode: string; cloud_platform_id?: number; machine_room_id?: number;
    provider_id?: number; status: string; features?: string;
    throughput?: string; contact_person: string; contact_phone: string;
    remarks?: string; created_at: string;
  }
}
export function getSecurityProductList() { return infraList<ScanSecurityProductApi.SecurityProduct>('security-product'); }
export function getSecurityProduct(id: number) { return infraGet<ScanSecurityProductApi.SecurityProduct>('security-product', id); }
export function createSecurityProduct(data: ScanSecurityProductApi.SecurityProduct) { return infraCreate('security-product', data); }
export function updateSecurityProduct(id: number, data: ScanSecurityProductApi.SecurityProduct) { return infraUpdate('security-product', id, data); }
export function deleteSecurityProduct(id: number) { return infraDelete('security-product', id); }
