import { infraCreate, infraDelete, infraGet, infraList, infraUpdate } from './compat';

export namespace ScanServiceProviderApi {
  export interface ServiceProvider {
    id?: number; provider_name: string; provider_code: string;
    short_name: string; logo_url?: string; contact_person: string;
    contact_phone: string; contact_email: string; headquarters: string;
    service_area: string; business_license: string; remarks?: string;
    status: string; created_at: string; updated_at?: string;
  }
}
export function getServiceProviderList() { return infraList<ScanServiceProviderApi.ServiceProvider>('service-provider'); }
export function getServiceProvider(id: number) { return infraGet<ScanServiceProviderApi.ServiceProvider>('service-provider', id); }
export function createServiceProvider(data: ScanServiceProviderApi.ServiceProvider) { return infraCreate('service-provider', data); }
export function updateServiceProvider(id: number, data: ScanServiceProviderApi.ServiceProvider) { return infraUpdate('service-provider', id, data); }
export function deleteServiceProvider(id: number) { return infraDelete('service-provider', id); }
