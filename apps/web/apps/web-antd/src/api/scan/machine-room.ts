import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanMachineRoomApi {
  export interface MachineRoom {
    id?: number; room_name: string; room_code: string;
    facility_type: string; address: string; zone_id?: number;
    room_type: string; contact_person: string; contact_phone: string;
    floor?: string; cabinet_count?: number; area_size?: string;
    remarks?: string; status: string; created_at?: string; updated_at?: string;
  }
}
export function getMachineRoomList() { return infraList<ScanMachineRoomApi.MachineRoom>('machine-room'); }
export function getMachineRoom(id: number) { return infraGet<ScanMachineRoomApi.MachineRoom>('machine-room', id); }
export function createMachineRoom(data: ScanMachineRoomApi.MachineRoom) { return infraCreate('machine-room', data); }
export function updateMachineRoom(id: number, data: ScanMachineRoomApi.MachineRoom) { return infraUpdate('machine-room', id, data); }
export function deleteMachineRoom(id: number) { return infraDelete('machine-room', id); }
