import { requestClient } from '#/api/request';

import { fromInfraResponse, infraCreate, infraDelete, infraGet, infraList, infraUpdate, toInfraPayload } from './compat';

export namespace ScanBusinessApplicationApi {
  export interface BusinessApplication { id?: number; name: string; description?: string; owner_user_id?: number; created_at: string; updated_at?: string; }
  export interface ApplicationEndpoint { id?: number; business_application_id: number; protocol: string; dest_ip: string; nat_ip?: string; dest_port: string; domain?: string; created_at: string; updated_at?: string; }
}
export function getBusinessApplicationList() { return infraList<ScanBusinessApplicationApi.BusinessApplication>('business-application'); }
export function getBusinessApplication(id: number) { return infraGet<ScanBusinessApplicationApi.BusinessApplication>('business-application', id); }
export function createBusinessApplication(data: ScanBusinessApplicationApi.BusinessApplication) { return infraCreate('business-application', data); }
export function updateBusinessApplication(id: number, data: ScanBusinessApplicationApi.BusinessApplication) { return infraUpdate('business-application', id, data); }
export function deleteBusinessApplication(id: number) { return infraDelete('business-application', id); }
export async function getApplicationEndpoints(appId: number) {
  const rows = await requestClient.get<ScanBusinessApplicationApi.ApplicationEndpoint[]>('/infra/application-endpoint/list-by-app', { params: { businessApplicationId: appId } });
  return fromInfraResponse(rows);
}
export function addApplicationEndpoint(appId: number, data: ScanBusinessApplicationApi.ApplicationEndpoint) {
  return requestClient.post('/infra/application-endpoint/create', toInfraPayload({ ...data, business_application_id: appId }));
}
