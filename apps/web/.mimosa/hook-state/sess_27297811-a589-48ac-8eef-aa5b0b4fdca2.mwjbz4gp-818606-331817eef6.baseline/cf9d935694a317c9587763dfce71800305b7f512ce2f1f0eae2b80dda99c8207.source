import type { PageParam, PageResult } from '@vben/request';

import { requestClient } from '#/api/request';

export namespace AiModelModelApi {
  export interface Model {
    id: number; // 编号
    key: string; // 配置唯一键
    name: string; // 模型名字
    model: string; // 模型标识
    platform: string; // 模型平台
    type: string; // 模型类型
    apiKey: string; // API 密钥
    url: string; // API 地址
    config?: Record<string, unknown>; // 平台扩展配置
    status: number; // 状态
  }

  export interface PlatformCapability {
    label: string;
    platform: string;
    presets: Record<string, Array<{ label: string; model: string }>>;
    types: string[];
    url: string;
  }

  export interface PlatformCapabilities {
    cachedModels: Array<{
      model: string;
      platform: string;
      syncedAt: number;
      type: string;
    }>;
    platforms: PlatformCapability[];
    types: string[];
  }

  export interface DiscoveredModel {
    id: string;
    type: string;
  }
}

/** 查询模型分页 */
export function getModelPage(params: PageParam) {
  return requestClient.get<PageResult<AiModelModelApi.Model>>(
    '/ai/model/page',
    { params },
  );
}

/** 获得模型列表 */
export function getModelSimpleList(type?: number) {
  return requestClient.get<AiModelModelApi.Model[]>('/ai/model/simple-list', {
    params: {
      type,
    },
  });
}

/** 查询模型详情 */
export function getModel(id: number) {
  return requestClient.get<AiModelModelApi.Model>(`/ai/model/get?id=${id}`);
}

/** 新增模型 */
export function createModel(data: AiModelModelApi.Model) {
  return requestClient.post('/ai/model/create', data);
}

/** 修改模型 */
export function updateModel(data: AiModelModelApi.Model) {
  return requestClient.put('/ai/model/update', data);
}

/** 删除模型 */
export function deleteModel(id: number) {
  return requestClient.delete(`/ai/model/delete?id=${id}`);
}

/** 按模型类型执行真实连接测试 */
export function testModel(id: number, prompt?: string) {
  return requestClient.post<{
    latencyMs: number;
    result: unknown;
    success: boolean;
  }>('/ai/model/test', { id, prompt });
}

/** 获取网关实际支持的平台、模型类型与默认地址 */
export function getModelPlatformCapabilities() {
  return requestClient.get<AiModelModelApi.PlatformCapabilities>(
    '/ai/model/platforms',
  );
}

/** 使用供应商 API Key 同步账号可用模型 */
export function discoverModels(data: {
  apiKey: string;
  platform: string;
  url: string;
}) {
  return requestClient.post<{
    models: AiModelModelApi.DiscoveredModel[];
    platform: string;
    persisted: boolean;
    source: string;
    syncedAt: number;
  }>('/ai/model/discover', data);
}
