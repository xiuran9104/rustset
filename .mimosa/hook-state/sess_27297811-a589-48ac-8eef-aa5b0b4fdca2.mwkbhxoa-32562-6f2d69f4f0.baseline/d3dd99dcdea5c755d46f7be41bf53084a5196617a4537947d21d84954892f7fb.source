import { requestClient } from '#/api/request';

export interface ApprovalRule {
  id?: number;
  name: string;
  resourceType: string;
  maxCpuCores?: null | number;
  maxMemoryGb?: null | number;
  maxResourceCount?: null | number;
  autoProvision: boolean;
  status: number;
  remarks?: string;
}

export function getApprovalRules() {
  return requestClient.get<{ list: ApprovalRule[] }>('/infra/approval-rule/page', {
    params: { pageNo: 1, pageSize: 100 },
  });
}
export function createApprovalRule(data: ApprovalRule) {
  return requestClient.post('/infra/approval-rule/create', data);
}
export function updateApprovalRule(id: number, data: ApprovalRule) {
  return requestClient.put('/infra/approval-rule/update', { ...data, id });
}
export function deleteApprovalRule(id: number) {
  return requestClient.delete('/infra/approval-rule/delete', { params: { id } });
}
