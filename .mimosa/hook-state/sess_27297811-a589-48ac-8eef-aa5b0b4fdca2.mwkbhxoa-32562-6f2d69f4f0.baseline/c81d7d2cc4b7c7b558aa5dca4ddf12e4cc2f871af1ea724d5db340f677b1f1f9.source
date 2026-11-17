import type { PageParam, PageResult } from '@vben/request';

import { requestClient } from '#/api/request';

export namespace SystemMcpKeyApi {
  /** MCP API Key（列表项：明文永不返回，只有前缀） */
  export interface McpApiKey {
    id: number;
    name: string;
    keyPrefix: string;
    userId: number;
    username?: string;
    nickname?: string;
    enabled: number;
    expiresTime?: string | null;
    lastUsedTime?: string | null;
    createTime?: string;
  }

  /** 创建响应：明文仅此一次返回 */
  export interface McpApiKeyCreated {
    id: number;
    apiKey: string;
  }
}

/** 查询 MCP 密钥分页 */
export function getMcpKeyPage(params: PageParam) {
  return requestClient.get<PageResult<SystemMcpKeyApi.McpApiKey>>(
    '/system/mcp-key/page',
    { params },
  );
}

/** 查询 MCP 密钥详情 */
export function getMcpKey(id: number) {
  return requestClient.get<SystemMcpKeyApi.McpApiKey>(
    `/system/mcp-key/get?id=${id}`,
  );
}

/** 创建 MCP 密钥（响应含一次性明文 apiKey） */
export function createMcpKey(data: {
  expiresTime?: string;
  name: string;
  userId: number;
}) {
  return requestClient.post<SystemMcpKeyApi.McpApiKeyCreated>(
    '/system/mcp-key/create',
    data,
  );
}

/** 修改 MCP 密钥（名称/启停） */
export function updateMcpKey(data: {
  enabled?: number;
  id: number;
  name?: string;
}) {
  return requestClient.put('/system/mcp-key/update', data);
}

/** 删除 MCP 密钥 */
export function deleteMcpKey(id: number) {
  return requestClient.delete(`/system/mcp-key/delete?id=${id}`);
}
