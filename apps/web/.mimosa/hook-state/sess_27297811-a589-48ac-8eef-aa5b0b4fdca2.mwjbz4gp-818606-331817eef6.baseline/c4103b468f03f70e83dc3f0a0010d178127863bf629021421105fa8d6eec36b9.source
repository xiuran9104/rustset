import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanCloudZoneApi {
  export interface CloudZone { id?: number; zone_name: string; zone_code: string; provider_id?: number; description?: string; created_at: string; }
}
export namespace ScanCloudPlatformApi {
  export interface CloudPlatform { id?: number; zone_id: number; platform_name: string; platform_code: string; description?: string; created_at: string; }
}
export function getCloudZoneList() { return infraList<ScanCloudZoneApi.CloudZone>('cloud-zone'); }
export function getCloudZone(id: number) { return infraGet<ScanCloudZoneApi.CloudZone>('cloud-zone', id); }
export function createCloudZone(data: ScanCloudZoneApi.CloudZone) { return infraCreate('cloud-zone', data); }
export function updateCloudZone(id: number, data: ScanCloudZoneApi.CloudZone) { return infraUpdate('cloud-zone', id, data); }
export function deleteCloudZone(id: number) { return infraDelete('cloud-zone', id); }
export function getCloudPlatformList() { return infraList<ScanCloudPlatformApi.CloudPlatform>('cloud-platform'); }
export function getCloudPlatform(id: number) { return infraGet<ScanCloudPlatformApi.CloudPlatform>('cloud-platform', id); }
export function createCloudPlatform(data: ScanCloudPlatformApi.CloudPlatform) { return infraCreate('cloud-platform', data); }
export function updateCloudPlatform(id: number, data: ScanCloudPlatformApi.CloudPlatform) { return infraUpdate('cloud-platform', id, data); }
export function deleteCloudPlatform(id: number) { return infraDelete('cloud-platform', id); }
