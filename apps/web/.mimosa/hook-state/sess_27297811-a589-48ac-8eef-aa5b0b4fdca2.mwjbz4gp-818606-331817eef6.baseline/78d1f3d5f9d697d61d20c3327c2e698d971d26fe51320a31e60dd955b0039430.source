import type { PageParam, PageResult } from '@vben/request';

import { requestClient } from '#/api/request';

export namespace AiMcpServerApi {
  /** 外部 MCP 服务器（令牌只写不读，列表仅回 hasToken） */
  export interface McpServer {
    id: number;
    name: string;
    url: string;
    authHeader: string;
    hasToken?: boolean;
    /** 仅创建/编辑时提交；编辑留空 = 保持不变 */
    authToken?: string;
    status: number;
    serverInfo?: Record<string, any>;
    toolCount?: number;
    lastSyncTime?: number | null;
    lastError?: string;
    remark?: string;
    createTime?: number;
  }

  export interface TestConnectionResult {
    serverInfo: Record<string, any>;
    toolCount: number;
    tools: string[];
  }

  export interface SyncResult {
    added: number;
    disabled: number;
    toolCount: number;
    updated: number;
  }
}

/** 查询 MCP 服务器分页 */
export function getMcpServerPage(params: PageParam) {
  return requestClient.get<PageResult<AiMcpServerApi.McpServer>>(
    '/ai/mcp-server/page',
    { params },
  );
}

/** 查询 MCP 服务器详情 */
export function getMcpServer(id: number) {
  return requestClient.get<AiMcpServerApi.McpServer>(
    `/ai/mcp-server/get?id=${id}`,
  );
}

/** 创建 MCP 服务器 */
export function createMcpServer(data: AiMcpServerApi.McpServer) {
  return requestClient.post<number>('/ai/mcp-server/create', data);
}

/** 修改 MCP 服务器 */
export function updateMcpServer(data: AiMcpServerApi.McpServer) {
  return requestClient.put('/ai/mcp-server/update', data);
}

/** 删除 MCP 服务器（同步工具一并清理；被对话绑定时拒绝） */
export function deleteMcpServer(id: number) {
  return requestClient.delete(`/ai/mcp-server/delete?id=${id}`);
}

/** 测试连接（不落库）：initialize + tools/list */
export function testMcpConnection(data: {
  authToken?: string;
  authHeader?: string;
  url: string;
}) {
  return requestClient.post<AiMcpServerApi.TestConnectionResult>(
    '/ai/mcp-server/test-connection',
    data,
  );
}

/** 同步远端工具进 ai.tools */
export function syncMcpServer(id: number) {
  return requestClient.post<AiMcpServerApi.SyncResult>(
    `/ai/mcp-server/sync?id=${id}`,
  );
}
