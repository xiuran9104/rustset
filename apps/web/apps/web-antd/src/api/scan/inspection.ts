import { requestClient } from '#/api/request';

import type { ScanTaskApi } from './task';

export namespace InspectionApi {
  export type InspectionTask = ScanTaskApi.ScanTask;

  export interface InspectionDifference {
    description: string;
    kind: string;
    port?: number;
    severity: string;
  }

  export interface InspectionRiskRef {
    assetIp: string;
    description: string;
    id: string;
    port: number;
    severity: string;
    solution?: string;
    status: string;
  }

  export interface InspectionResult {
    baselinePorts: null | number[];
    createTime: string;
    differences: InspectionDifference[];
    id: string;
    ip: string;
    openPorts: number[];
    registered: boolean;
    risks: InspectionRiskRef[];
    taskId: string;
    uncertainPorts: number[];
  }

  export interface InspectionBaseline {
    allowedPorts: number[];
    ip: string;
    reason: string;
    updateTime: string;
    updatedBy: string;
  }
}

export function getInspectionList() {
  return requestClient.get<InspectionApi.InspectionTask[]>('/infra/inspection/list');
}

export function getInspectionResults(taskId: string) {
  return requestClient.get<InspectionApi.InspectionResult[]>(
    '/infra/inspection/results',
    { params: { taskId } },
  );
}

export function getInspectionBaseline(ip: string) {
  return requestClient.get<InspectionApi.InspectionBaseline | null>(
    '/infra/inspection/baseline',
    { params: { ip } },
  );
}

export function runInspection(data: {
  name?: string;
  ports: number[];
  targetIps: string[];
}) {
  return requestClient.post('/infra/inspection/run', data);
}

export function saveInspectionBaseline(data: {
  allowedPorts: number[];
  ip: string;
  reason: string;
}) {
  return requestClient.put('/infra/inspection/baseline', data);
}
