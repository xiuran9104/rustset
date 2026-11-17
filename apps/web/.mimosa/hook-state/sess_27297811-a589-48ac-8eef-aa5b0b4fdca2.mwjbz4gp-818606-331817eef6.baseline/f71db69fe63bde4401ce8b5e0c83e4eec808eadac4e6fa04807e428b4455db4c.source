import { requestClient } from '#/api/request';

export namespace InfraMonitorApi {
  export interface PostgreSqlMonitor {
    activeQueries: Array<{ durationMs: number; pid: number; query: string; state: string; user: string }>;
    cacheHitRatio: number;
    connectionStates: Array<{ count: number; state: string }>;
    connections: number;
    databaseName: string;
    databaseSize: number;
    deadlocks: number;
    maxConnections: number;
    startedAt: string;
    tables: Array<{ deadRows: number; indexScans: number; liveRows: number; schema: string; sequentialScans: number; table: string; totalSize: number }>;
    temporaryBytes: number;
    transactions: { committed: number; rolledBack: number };
    version: string;
  }

  export interface RustMonitor {
    cpuCores: number;
    environment: string;
    fileDescriptors: number;
    healthy: boolean;
    memory: { peakResidentBytes: number; residentBytes: number; virtualBytes: number };
    processId: number;
    rust: string;
    service: string;
    threads: number;
    uptimeSeconds: number;
    version: string;
  }

  export interface TraceMonitor {
    averageDurationMs: number;
    failed: number;
    maximumDurationMs: number;
    total: number;
    traces: Array<{ durationMs: number; endedAt: string; message: string; method: string; service: string; startedAt: string; status: number; traceId: string; url: string }>;
    window: string;
  }
}

export const getPostgreSqlMonitor = () =>
  requestClient.get<InfraMonitorApi.PostgreSqlMonitor>('/infra/monitor/postgresql');
export const getRustMonitor = () =>
  requestClient.get<InfraMonitorApi.RustMonitor>('/infra/monitor/rust');
export const getTraceMonitor = () =>
  requestClient.get<InfraMonitorApi.TraceMonitor>('/infra/monitor/traces');
