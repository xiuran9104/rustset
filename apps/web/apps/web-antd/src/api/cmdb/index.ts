import { requestClient } from '#/api/request';

export interface CmdbAttribute {
  id?: number;
  modelId?: number;
  name: string;
  code: string;
  attrType: string;
  required: boolean;
  choices?: null | { label: string; value: string }[];
  defaultValue?: any;
  expression?: string;
  isComputed?: boolean;
  color?: string;
  showInList: boolean;
  sort: number;
}

export interface CmdbModel {
  id?: number;
  name: string;
  code: string;
  description?: string;
  icon?: string;
  uniqueKey?: string;
  sort: number;
  status?: number;
  instanceCount?: number;
  attributeCount?: number;
}

export interface CmdbInstance {
  id: number;
  modelId: number;
  attributes: Record<string, any>;
  createTime?: string;
  updateTime?: string;
}

export interface CmdbTopologyNode {
  depth: number;
  id: number;
  label: string;
  modelCode: string;
  modelId: number;
  modelName: string;
  root: boolean;
}

export interface CmdbTopologyEdge {
  id: number;
  relation: string;
  sourceId: number;
  targetId: number;
}

export interface CmdbTopology {
  depth: number;
  edges: CmdbTopologyEdge[];
  limit: number;
  nodes: CmdbTopologyNode[];
  rootId: number;
  truncated: boolean;
}

export const ATTR_TYPES = [
  { value: 'text', label: '文本' },
  { value: 'textarea', label: '长文本' },
  { value: 'number', label: '整数' },
  { value: 'float', label: '浮点数' },
  { value: 'bool', label: '布尔' },
  { value: 'date', label: '日期' },
  { value: 'datetime', label: '日期时间' },
  { value: 'select', label: '单选' },
  { value: 'multi_select', label: '多选' },
  { value: 'link', label: '链接' },
  { value: 'json', label: 'JSON' },
  { value: 'password', label: '密码' },
] as const;

// ---------- model ----------
export function getModelList() {
  return requestClient.get<CmdbModel[]>('/cmdb/model/list');
}
export function getModelPage(params: { keyword?: string; pageNo?: number; pageSize?: number; }) {
  return requestClient.get<{ list: CmdbModel[]; total: number }>('/cmdb/model/page', { params });
}
export function getModel(id: number) {
  return requestClient.get<CmdbModel>('/cmdb/model/get', { params: { id } });
}
export function createModel(data: CmdbModel) {
  return requestClient.post('/cmdb/model/create', data);
}
export function updateModel(data: CmdbModel & { id: number }) {
  return requestClient.put('/cmdb/model/update', data);
}
export function deleteModel(id: number) {
  return requestClient.delete('/cmdb/model/delete', { params: { id } });
}

// ---------- attribute ----------
export function getAttributesByModel(modelId: number) {
  return requestClient.get<CmdbAttribute[]>('/cmdb/attribute/list-by-model', {
    params: { modelId },
  });
}
export function createAttribute(data: CmdbAttribute) {
  return requestClient.post('/cmdb/attribute/create', data);
}
export function updateAttribute(data: CmdbAttribute & { id: number }) {
  return requestClient.put('/cmdb/attribute/update', data);
}
export function deleteAttribute(id: number) {
  return requestClient.delete('/cmdb/attribute/delete', { params: { id } });
}

export interface CmdbAttributeTrigger {
  id?: number; modelId: number; name: string; conditionCode: string;
  conditionValue: any; actionCode: string; actionValue: any; enabled: boolean;
}
export function getAttributeTriggers(modelId: number) {
  return requestClient.get<CmdbAttributeTrigger[]>(`/cmdb/trigger/list/${modelId}`);
}
export function createAttributeTrigger(data: CmdbAttributeTrigger) {
  return requestClient.post('/cmdb/trigger/create', data);
}
export function updateAttributeTrigger(data: CmdbAttributeTrigger & { id: number }) {
  return requestClient.put('/cmdb/trigger/update', data);
}
export function deleteAttributeTrigger(id: number) {
  return requestClient.delete(`/cmdb/trigger/delete/${id}`);
}

// ---------- instance ----------
export function getInstancePage(params: {
  keyword?: string;
  modelId: number;
  pageNo?: number;
  pageSize?: number;
}) {
  return requestClient.get<{ list: CmdbInstance[]; total: number }>('/cmdb/instance/page', {
    params,
  });
}
export function createInstance(modelId: number, attributes: Record<string, any>) {
  return requestClient.post('/cmdb/instance/create', { modelId, attributes });
}
export function updateInstance(id: number, attributes: Record<string, any>) {
  return requestClient.put('/cmdb/instance/update', { id, attributes });
}
export function updateInstances(ids: number[], attributes: Record<string, any>) {
  return requestClient.put<{ unchanged: number; updated: number }>(
    '/cmdb/instance/update-list',
    { ids, attributes },
  );
}
export function deleteInstance(id: number) {
  return requestClient.delete('/cmdb/instance/delete', { params: { id } });
}
export function deleteInstances(ids: number[]) {
  return requestClient.delete('/cmdb/instance/delete-list', { params: { ids: ids.join(',') } });
}

// ---------- relation ----------
export function getRelationsByInstance(instanceId: number) {
  return requestClient.get<
    { id: number; relation: string; sourceId: number; targetId: number; }[]
  >('/cmdb/relation/list-by-instance', { params: { instanceId } });
}
export function getRelationTopology(instanceId: number, depth = 2, limit = 80) {
  return requestClient.get<CmdbTopology>('/cmdb/relation/topology', {
    params: { depth, instanceId, limit },
  });
}
export function bindRelation(sourceId: number, targetId: number, relation?: string) {
  return requestClient.post('/cmdb/relation/bind', { sourceId, targetId, relation });
}
export function unbindRelation(id: number) {
  return requestClient.delete('/cmdb/relation/unbind', { params: { id } });
}
