import { requestClient } from '#/api/request';

export namespace ScanTaskApi {
  export interface ScanTask {
    attemptCount: number;
    cancelRequested: boolean;
    completedTargets: number;
    createTime: string;
    createdBy?: string;
    domainBrute: boolean;
    endTime?: string;
    errorMessage?: string;
    foundAssets: number;
    foundRisks: number;
    id: string;
    maxAttempts: number;
    name: string;
    nextAttemptAt: string;
    osDetection: boolean;
    portPolicy: string;
    scanPorts: number[];
    serviceDetection: boolean;
    siteIdentify: boolean;
    startTime?: string;
    status: string;
    target: string;
    taskKind: string;
    timeoutSeconds: number;
    totalTargets: number;
    updateTime: string;
  }

  export interface CreateScanTask {
    domainBrute?: boolean;
    idempotencyKey?: string;
    maxAttempts?: number;
    name: string;
    osDetection?: boolean;
    portPolicy: string;
    serviceDetection?: boolean;
    siteIdentify?: boolean;
    target: string;
    timeoutSeconds?: number;
  }

  export interface TriggerScan {
    idempotencyKey?: string;
    maxAttempts?: number;
    ports: number[];
    targetIp: string;
    timeoutSeconds?: number;
  }
}

export function getTaskList() {
  return requestClient.get<ScanTaskApi.ScanTask[]>('/infra/task/list');
}

export function getTask(id: string) {
  return requestClient.get<ScanTaskApi.ScanTask>('/infra/task/get', {
    params: { id },
  });
}

export function createTask(data: ScanTaskApi.CreateScanTask) {
  return requestClient.post<string>('/infra/task/create', data);
}

export function deleteTask(id: string) {
  return requestClient.delete('/infra/task/delete', { params: { id } });
}

export function executeScan(data: ScanTaskApi.TriggerScan) {
  return requestClient.post('/infra/task/trigger-scan', data);
}

export function cancelTask(id: string) {
  return requestClient.put('/infra/task/cancel', { id });
}

export function retryTask(id: string) {
  return requestClient.post('/infra/task/retry', { id });
}
