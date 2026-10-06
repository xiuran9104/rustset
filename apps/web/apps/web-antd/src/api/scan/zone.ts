import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanZoneApi {
  export interface ZoneConfig { id: string; name: string; cidr: string; priority: number; cloud_platform_id?: number; cloud_platform_name?: string; machine_room_id?: number; machine_room_name?: string; }
  export type CreateZoneRequest = Omit<ZoneConfig, 'id'>;
}
export function getZoneList() { return infraList<ScanZoneApi.ZoneConfig>('network-zone'); }
export function getZone(id: string) { return infraGet<ScanZoneApi.ZoneConfig>('network-zone', id); }
export function createZone(data: ScanZoneApi.CreateZoneRequest) { return infraCreate('network-zone', data); }
export function updateZone(id: string, data: ScanZoneApi.CreateZoneRequest) { return infraUpdate('network-zone', id, data); }
export function deleteZone(id: string) { return infraDelete('network-zone', id); }
