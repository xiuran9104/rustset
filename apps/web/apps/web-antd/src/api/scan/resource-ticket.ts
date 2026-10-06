import { requestClient } from '#/api/request';

import { fromInfraResponse, infraCreate, infraDelete, infraGet, infraPageList, infraUpdate, toInfraPayload } from './compat';

export namespace ScanResourceTicketApi {
  export interface CreateResourceTicketRequest {
    resource_type: string; ecs_name: string; idempotency_key?: string;
    ticket_type?: string; risk_level?: string; target_resource_id?: number;
    target_resource_type?: string; target_config?: string;
    maintenance_window?: string; allow_interruption?: boolean;
    backup_confirmed?: boolean; rollback_plan?: string; retention_until?: string;
    provider_id?: number; provider_name?: string; cloud_platform_id?: number;
    cloud_platform_name?: string; machine_room_id?: number; machine_room_name?: string;
    cloud_region?: string; cloud_category?: string; zone_name?: string;
    zone_cabinet?: string; rack_units?: number; customer_name?: string;
    application_name?: string; application_endpoint_id?: number;
    application_domain?: string; contract_name?: string; ecs_type?: string;
    ecs_os?: string; resource_count?: number; cpu_cores?: number; memory_gb?: number;
    system_disk?: string; system_disk_size_gb?: number; data_disk?: string;
    expire_at?: string; has_security_product?: boolean; security_products?: string;
    ip_address?: string; remarks?: string;
  }
  export interface ResourceTicket {
    id?: number; resource_type: string; ecs_name: string; ticket_status: string;
    provider_id?: number; provider_name?: string; cloud_platform_id?: number;
    cloud_platform_name?: string; machine_room_id?: number; machine_room_name?: string;
    cloud_region?: string; cloud_category?: string; zone_name?: string;
    zone_cabinet?: string; rack_units: number; customer_name?: string;
    application_name?: string; contract_name?: string; ecs_type?: string;
    ecs_os?: string; resource_count: number; cpu_cores: number; memory_gb: number;
    system_disk?: string; system_disk_size_gb: number; data_disk?: string;
    expire_at?: string; has_security_product: boolean; security_products?: string;
    ip_address?: string; delivery_status?: string; remarks?: string;
    created_at: string; updated_at?: string; created_by: string;
    approver?: string; approve_time?: string; approve_comment?: string;
    provisioner?: string; provision_time?: string; provision_details?: string;
    deliverer?: string; deliver_time?: string; deliver_comment?: string;
  }
  export type UpdateResourceTicketRequest = Partial<Pick<CreateResourceTicketRequest,
    'allow_interruption' | 'backup_confirmed' | 'cpu_cores' | 'data_disk' |
    'ecs_name' | 'expire_at' | 'has_security_product' | 'ip_address' |
    'maintenance_window' | 'memory_gb' | 'remarks' | 'resource_count' |
    'retention_until' | 'rollback_plan' | 'security_products' | 'system_disk' |
    'system_disk_size_gb' | 'target_config'>>;
}
export function getResourceTicketList() { return infraPageList<ScanResourceTicketApi.ResourceTicket>('resource-ticket'); }
export function getResourceTicket(id: number) { return infraGet<ScanResourceTicketApi.ResourceTicket>('resource-ticket', id); }
export function createResourceTicket(data: ScanResourceTicketApi.CreateResourceTicketRequest) { return infraCreate('resource-ticket', data); }
export function updateResourceTicket(id: number, data: ScanResourceTicketApi.UpdateResourceTicketRequest) { return infraUpdate('resource-ticket', id, data); }
export function deleteResourceTicket(id: number) { return infraDelete('resource-ticket', id); }
export function approveTicket(id: number, data: { approved: boolean; comment?: string }) { return requestClient.post(`/infra/resource-ticket/${id}/approve`, toInfraPayload(data)); }
export async function getTicketApprovalHistory(id: number) {
  const ticket = await getResourceTicket(id);
  const events = [
    ticket.approve_time && { id: 'approve', stage_no: 1, role_name: '资源审批', approver: ticket.approver, decision: ticket.ticket_status === 'rejected' ? 'rejected' : 'approved', comment: ticket.approve_comment },
    ticket.provision_time && { id: 'provision', stage_no: 2, role_name: '资源配置', approver: ticket.provisioner, decision: 'approved', comment: ticket.provision_details },
    ticket.deliver_time && { id: 'deliver', stage_no: 3, role_name: '资源交付', approver: ticket.deliverer, decision: 'approved', comment: ticket.deliver_comment },
  ].filter(Boolean);
  return fromInfraResponse(events);
}
export function provisionTicket(id: number, data: { details?: string; config_id?: number; image_id?: string; flavor?: string; availability_zone?: string; subnet_id?: string; vpc_id?: string; security_groups?: string[] }) { return requestClient.post(`/infra/resource-ticket/${id}/provision`, toInfraPayload(data), { timeout: 600_000 }); }
export function deliverTicket(id: number, data: { comment?: string }) { return requestClient.post(`/infra/resource-ticket/${id}/deliver`, toInfraPayload(data)); }
