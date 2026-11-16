import type { PageParam, PageResult } from '@vben/request';

import { requestClient } from '#/api/request';

export namespace AssetOpsFlowApi {
  /** 流程定义 */
  export interface FlowDefinition {
    id: number;
    name: string;
    description: string;
    /** 0=启用 1=停用 */
    status: number;
    definition: { steps: FlowStep[] };
    version?: number;
    stepCount?: number;
    createTime?: string;
    updateTime?: string;
  }

  /** 步骤定义 */
  export interface FlowStep {
    key: string;
    name: string;
    type: 'approval' | 'auto' | 'manual' | 'notification' | 'ticket_wait';
    config: {
      action?: string;
      params?: Record<string, any>;
      assignee?: { role?: string; userIds?: number[]; usernames?: string[] };
      timeoutHours?: number;
      onTimeout?: 'auto_reject' | 'escalate_notify';
      templateCode?: string;
      ticketId?: number;
      [key: string]: any;
    };
    condition?: null | { field: string; op: string; value: any };
  }

  /** 流程实例 */
  export interface FlowInstance {
    id: number;
    definitionId: number;
    definitionName?: string;
    title: string;
    bizType?: string;
    bizId?: string;
    payload: Record<string, any>;
    status: 'cancelled' | 'completed' | 'failed' | 'running';
    currentIndex: number;
    currentStepName?: string;
    starter: string;
    startTime: string;
    finishTime?: string | null;
  }

  /** 步骤实例 */
  export interface FlowStepInstance {
    id: number;
    instanceId: number;
    stepKey: string;
    stepIndex: number;
    name: string;
    type: string;
    status: 'active' | 'cancelled' | 'done' | 'failed' | 'pending' | 'skipped';
    config: Record<string, any>;
    result: Record<string, any>;
    operator: string;
    comment: string;
    timeoutAt?: string | null;
    startedAt?: string | null;
    finishedAt?: string | null;
  }

  /** 我的待办步骤 */
  export interface FlowTodo {
    instanceId: number;
    stepId: number;
    stepKey: string;
    stepIndex: number;
    stepName: string;
    title: string;
    type: string;
    timeoutAt?: string | null;
    payload: Record<string, any>;
  }
}

/** 流程定义 CRUD */
export function getFlowDefinitionPage(params: PageParam) {
  return requestClient.get<PageResult<AssetOpsFlowApi.FlowDefinition>>(
    '/infra/flow-definition/page',
    { params },
  );
}
export function getFlowDefinition(id: number) {
  return requestClient.get<AssetOpsFlowApi.FlowDefinition>(
    `/infra/flow-definition/get?id=${id}`,
  );
}
export function createFlowDefinition(data: AssetOpsFlowApi.FlowDefinition) {
  return requestClient.post('/infra/flow-definition/create', data);
}
export function updateFlowDefinition(data: AssetOpsFlowApi.FlowDefinition) {
  return requestClient.put('/infra/flow-definition/update', data);
}
export function deleteFlowDefinition(id: number) {
  return requestClient.get(`/infra/flow-definition/delete?id=${id}`);
}

/** 流程实例 */
export function getFlowInstancePage(params: PageParam) {
  return requestClient.get<PageResult<AssetOpsFlowApi.FlowInstance>>(
    '/infra/flow-instance/page',
    { params },
  );
}
export function getFlowInstance(id: number) {
  return requestClient.get<{
    instance: AssetOpsFlowApi.FlowInstance;
    steps: AssetOpsFlowApi.FlowStepInstance[];
  }>(`/infra/flow-instance/get?id=${id}`);
}
export function startFlowInstance(data: {
  definitionId: number;
  payload?: Record<string, any>;
  title?: string;
}) {
  return requestClient.post<number>('/infra/flow-instance/create', data);
}
export function cancelFlowInstance(id: number) {
  return requestClient.post(`/infra/flow-instance/cancel?id=${id}`);
}
export function getFlowTodo() {
  return requestClient.get<AssetOpsFlowApi.FlowTodo[]>(
    '/infra/flow-instance/todo',
  );
}
export function approveFlowStep(
  instanceId: number,
  stepKey: string,
  data?: { comment?: string },
) {
  return requestClient.post(
    `/infra/flow-instance/${instanceId}/step/${stepKey}/approve`,
    data ?? {},
  );
}
export function rejectFlowStep(
  instanceId: number,
  stepKey: string,
  data: { comment: string },
) {
  return requestClient.post(
    `/infra/flow-instance/${instanceId}/step/${stepKey}/reject`,
    data,
  );
}
export function completeFlowStep(
  instanceId: number,
  stepKey: string,
  data?: { comment?: string; result?: any },
) {
  return requestClient.post(
    `/infra/flow-instance/${instanceId}/step/${stepKey}/complete`,
    data ?? {},
  );
}
export function retryFlowStep(instanceId: number, stepKey: string) {
  return requestClient.post(
    `/infra/flow-instance/${instanceId}/step/${stepKey}/retry`,
    {},
  );
}
