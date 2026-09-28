<script lang="ts" setup>
import type { CmdbAttribute, CmdbInstance, CmdbModel, CmdbTopology, CmdbTopologyNode } from '#/api/cmdb';

import { computed, onMounted, ref, watch } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import {
  createInstance,
  deleteInstance,
  deleteInstances,
  getAttributesByModel,
  getInstancePage,
  getModelList,
  getRelationTopology,
  updateInstance,
  updateInstances,
} from '#/api/cmdb';
import { requestClient } from '#/api/request';

const models = ref<CmdbModel[]>([]);
const attributes = ref<CmdbAttribute[]>([]);
const currentModelId = ref<number>();
const rows = ref<CmdbInstance[]>([]);
const total = ref(0);
const pageNo = ref(1);
const pageSize = 20;
const keyword = ref('');
const loading = ref(false);
const selectedIds = ref<number[]>([]);

const modalVisible = ref(false);
const editing = ref<CmdbInstance | null>(null);
const form = ref<Record<string, any>>({});
const batchModalVisible = ref(false);
const batchSubmitting = ref(false);
const batchAttributeCodes = ref<string[]>([]);
const batchForm = ref<Record<string, any>>({});
const atomicImport = ref(false);
const topologyVisible = ref(false);
const topologyLoading = ref(false);
const topologyDepth = ref(2);
const topologyRoot = ref<CmdbInstance | null>(null);
const topology = ref<CmdbTopology | null>(null);
const topologySelectedId = ref<number>();

const currentModel = computed(() => models.value.find((m) => m.id === currentModelId.value));
const batchAttributes = computed(() =>
  attributes.value.filter((attr) => batchAttributeCodes.value.includes(attr.code)),
);
const topologySelectedNode = computed(() =>
  topology.value?.nodes.find((node) => node.id === topologySelectedId.value),
);
const topologyModels = computed(() => {
  const values = new Map<string, { code: string; name: string }>();
  for (const node of topology.value?.nodes ?? []) {
    values.set(node.modelCode, { code: node.modelCode, name: node.modelName });
  }
  return [...values.values()];
});
const topologyLayout = computed(() => {
  const width = 920;
  const height = 560;
  const centerX = width / 2;
  const centerY = height / 2;
  const grouped = new Map<number, CmdbTopologyNode[]>();
  for (const node of topology.value?.nodes ?? []) {
    const group = grouped.get(node.depth) ?? [];
    group.push(node);
    grouped.set(node.depth, group);
  }
  const nodes = [...grouped.entries()].flatMap(([depth, group]) =>
    group.map((node, index) => {
      if (depth === 0) return { ...node, x: centerX, y: centerY };
      const radius = Math.min(95 + depth * 105, 245);
      const angle = -Math.PI / 2 + (index * Math.PI * 2) / group.length + depth * 0.24;
      return {
        ...node,
        x: centerX + Math.cos(angle) * radius,
        y: centerY + Math.sin(angle) * radius,
      };
    }),
  );
  const byId = new Map(nodes.map((node) => [node.id, node]));
  const edges = (topology.value?.edges ?? []).flatMap((edge) => {
    const source = byId.get(edge.sourceId);
    const target = byId.get(edge.targetId);
    if (!source || !target) return [];
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const distanceScale = Math.max(Math.abs(dx) / 74, Math.abs(dy) / 31, 1);
    const offsetX = dx / distanceScale;
    const offsetY = dy / distanceScale;
    return [{
      ...edge,
      labelX: (source.x + target.x) / 2,
      labelY: (source.y + target.y) / 2 - 5,
      x1: source.x + offsetX,
      x2: target.x - offsetX,
      y1: source.y + offsetY,
      y2: target.y - offsetY,
    }];
  });
  return { edges, height, nodes, width };
});

onMounted(async () => {
  models.value = (await getModelList()) as any;
  if (models.value.length) {
    currentModelId.value = models.value[0]!.id;
  }
});

watch(currentModelId, async (id) => {
  rows.value = [];
  selectedIds.value = [];
  if (!id) return;
  attributes.value = (await getAttributesByModel(id)) as any;
  pageNo.value = 1;
  await fetchRows();
});

async function fetchRows() {
  if (!currentModelId.value) return;
  loading.value = true;
  try {
    const result = (await getInstancePage({
      modelId: currentModelId.value,
      pageNo: pageNo.value,
      pageSize,
      keyword: keyword.value || undefined,
    })) as any;
    rows.value = result.list ?? [];
    total.value = result.total ?? 0;
  } catch {
    message.error('加载实例失败');
  } finally {
    loading.value = false;
  }
}

const tableColumns = computed(() => {
  const visible = attributes.value.filter((attr) => attr.showInList);
  const list = visible.slice(0, 10).map((attr) => ({
    title: attr.name,
    key: attr.code,
    dataIndex: ['attributes', attr.code],
    width: attr.attrType === 'textarea' || attr.attrType === 'json' ? 200 : 140,
    ellipsis: true,
  }));
  const hidden = visible.length - Math.min(visible.length, 10);
  return [
    ...list,
    { title: hidden > 0 ? `…等 ${visible.length} 列` : '', key: '__placeholder', width: 0 },
    { title: '更新时间', dataIndex: 'updateTime', key: 'updateTime', width: 170 },
    { title: '操作', key: 'actions', width: 200, fixed: 'right' as const },
  ].filter((column) => column.key !== '__placeholder' || (column.title as string));
});

function displayValue(record: CmdbInstance, attr: CmdbAttribute) {
  const value = record.attributes?.[attr.code];
  if (value === undefined || value === null) return '-';
  if (attr.attrType === 'bool') return value ? '是' : '否';
  if (Array.isArray(value)) return value.join('、');
  if (typeof value === 'object') return JSON.stringify(value);
  return String(value);
}

function defaultForm(): Record<string, any> {
  const data: Record<string, any> = {};
  for (const attr of attributes.value) {
    data[attr.code] = attr.defaultValue ?? defaultByType(attr.attrType);
  }
  return data;
}
function defaultByType(type: string) {
  switch (type) {
    case 'bool': return false;
    case 'float': return undefined;
    case 'json': return undefined;
    case 'multi_select': return [];
    case 'number': return undefined;
    default: return '';
  }
}

function openCreate() {
  editing.value = null;
  form.value = defaultForm();
  modalVisible.value = true;
}
function openEdit(record: CmdbInstance) {
  editing.value = record;
  form.value = { ...defaultForm(), ...record.attributes };
  modalVisible.value = true;
}

function serializeForm(): Record<string, any> {
  const data: Record<string, any> = {};
  for (const attr of attributes.value) {
    let value = form.value[attr.code];
    if (attr.attrType === 'json' && typeof value === 'string' && value.trim()) {
      try {
        value = JSON.parse(value);
      } catch {
        message.warning(`属性 ${attr.name} 的 JSON 格式不正确`);
        throw new Error('invalid json');
      }
    }
    if (value === '' || value === undefined || value === null) continue;
    data[attr.code] = value;
  }
  return data;
}

async function submit() {
  if (!currentModelId.value) return;
  let payload: Record<string, any>;
  try {
    payload = serializeForm();
  } catch {
    return;
  }
  try {
    if (editing.value) {
      await updateInstance(editing.value.id, payload);
      message.success('实例已更新');
    } else {
      await createInstance(currentModelId.value, payload);
      message.success('实例已创建');
    }
    modalVisible.value = false;
    fetchRows();
  } catch (error: any) {
    message.error(error?.message || '保存失败');
  }
}

function openBatchUpdate() {
  batchAttributeCodes.value = [];
  batchForm.value = {};
  batchModalVisible.value = true;
}

function changeBatchAttributes(codes: string[]) {
  const next: Record<string, any> = {};
  for (const code of codes) {
    if (Object.hasOwn(batchForm.value, code)) {
      next[code] = batchForm.value[code];
      continue;
    }
    next[code] = undefined;
  }
  batchForm.value = next;
}

function serializeBatchPatch() {
  const patch: Record<string, any> = {};
  for (const attr of batchAttributes.value) {
    let value = batchForm.value[attr.code];
    if (attr.attrType === 'json' && typeof value === 'string' && value.trim()) {
      try {
        value = JSON.parse(value);
      } catch {
        message.warning(`属性 ${attr.name} 的 JSON 格式不正确`);
        throw new Error('invalid json');
      }
    }
    patch[attr.code] = value === '' || value === undefined ? null : value;
  }
  return patch;
}

async function submitBatchUpdate() {
  if (!batchAttributeCodes.value.length || !selectedIds.value.length) return;
  let patch: Record<string, any>;
  try {
    patch = serializeBatchPatch();
  } catch {
    return;
  }
  batchSubmitting.value = true;
  try {
    const result = await updateInstances(selectedIds.value, patch);
    message.success(`批量修改完成：更新 ${result.updated} 条，未变化 ${result.unchanged} 条`);
    batchModalVisible.value = false;
    selectedIds.value = [];
    await fetchRows();
  } catch (error: any) {
    message.error(error?.message || '批量修改失败');
  } finally {
    batchSubmitting.value = false;
  }
}

async function fetchTopology() {
  if (!topologyRoot.value) return;
  topologyLoading.value = true;
  try {
    topology.value = await getRelationTopology(topologyRoot.value.id, topologyDepth.value, 80);
    topologySelectedId.value = topologyRoot.value.id;
  } catch (error: any) {
    message.error(error?.message || '加载关系拓扑失败');
  } finally {
    topologyLoading.value = false;
  }
}

async function openTopology(record: CmdbInstance) {
  topologyRoot.value = record;
  topologyDepth.value = 2;
  topology.value = null;
  topologyVisible.value = true;
  await fetchTopology();
}

function topologyColor(code: string) {
  const palette = ['#1677ff', '#13a8a8', '#722ed1', '#d46b08', '#389e0d', '#c41d7f', '#0958d9'];
  let hash = 0;
  for (const character of code) hash = (hash * 31 + character.charCodeAt(0)) >>> 0;
  return palette[hash % palette.length]!;
}

function topologyLabel(label: string, max = 18) {
  return label.length > max ? `${label.slice(0, max - 1)}…` : label;
}

async function exportExcel() {
  if (!currentModelId.value) return;
  try {
    const blob = (await requestClient.download('/cmdb/instance/export', {
      params: { modelId: currentModelId.value },
    })) as any;
    const url = URL.createObjectURL(
      blob instanceof Blob ? blob : new Blob([blob]),
    );
    const link = document.createElement('a');
    link.href = url;
    link.download = `cmdb_instances_model_${currentModelId.value}.xlsx`;
    link.click();
    URL.revokeObjectURL(url);
  } catch {
    message.error('导出失败');
  }
}

async function importExcel(file: File) {
  if (!currentModelId.value) return false;
  const form_data = new FormData();
  form_data.append('modelId', String(currentModelId.value));
  form_data.append('mode', atomicImport.value ? 'atomic' : 'row');
  form_data.append('file', file);
  try {
    const result = (await requestClient.post('/cmdb/instance/import', form_data, {
      headers: { 'Content-Type': 'multipart/form-data' },
    })) as any;
    message.success(`${atomicImport.value ? '原子导入' : '导入'}完成：新增 ${result?.created ?? 0} 条，失败 ${result?.failed ?? 0} 条`);
    if (result?.failed) {
      console.warn('import errors:', result.errors);
      message.warning('失败明细已输出到浏览器控制台（前 50 条）');
    }
    fetchRows();
  } catch (error: any) {
    message.error(error?.message || '导入失败');
  }
  return false;
}

async function removeOne(record: CmdbInstance) {
  try {
    await deleteInstance(record.id);
    message.success('已删除');
    fetchRows();
  } catch (error: any) {
    message.error(error?.message || '删除失败');
  }
}
async function removeSelected() {
  try {
    await deleteInstances(selectedIds.value);
    message.success(`已删除 ${selectedIds.value.length} 条`);
    selectedIds.value = [];
    fetchRows();
  } catch (error: any) {
    message.error(error?.message || '批量删除失败');
  }
}

function controlFor(attr: CmdbAttribute) {
  return attr.attrType;
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-page-header
        title="实例管理"
        sub-title="配置项（CI）台账 —— 表格与表单由所选模型的属性定义动态生成"
        style="margin-bottom: 16px; padding: 0"
      />
      <a-alert
        v-if="!models.length"
        type="warning"
        show-icon
        message="还没有模型。请先到「模型管理」创建模型并定义属性。"
        style="margin-bottom: 16px"
      />
      <a-space :size="16" style="margin-bottom: 16px" wrap>
        <a-select
          v-model:value="currentModelId"
          :options="models.map((m) => ({ value: m.id, label: m.name }))"
          placeholder="选择模型"
          style="width: 220px"
          show-search
          option-filter-prop="label"
        />
        <a-input-search
          v-model:value="keyword"
          placeholder="搜索实例属性内容..."
          style="width: 280px"
          allow-clear
          @search="() => { pageNo = 1; fetchRows(); }"
        />
        <a-button @click="fetchRows">刷新</a-button>
        <a-button :disabled="!currentModelId || !rows.length" @click="exportExcel">导出 Excel</a-button>
        <a-upload
          :show-upload-list="false"
          :before-upload="importExcel"
          accept=".xlsx"
          :disabled="!currentModelId || !attributes.length"
        >
          <a-button v-access:code="['cmdb:instance:create']" :disabled="!currentModelId || !attributes.length">导入 Excel</a-button>
        </a-upload>
        <a-tooltip title="开启后整份文件在一个事务中提交，任一行失败都会全部回滚">
          <a-space size="small">
            <a-switch v-model:checked="atomicImport" :disabled="!currentModelId || !attributes.length" />
            <span>原子导入</span>
          </a-space>
        </a-tooltip>
        <a-button
          v-access:code="['cmdb:instance:create']"
          type="primary"
          :disabled="!currentModelId || !attributes.length"
          @click="openCreate"
        >
          新建{{ currentModel?.name || '实例' }}
        </a-button>
        <a-button
          v-if="selectedIds.length"
          v-access:code="['cmdb:instance:update']"
          @click="openBatchUpdate"
        >
          批量修改（{{ selectedIds.length }}）
        </a-button>
        <a-popconfirm
          v-if="selectedIds.length"
          :title="`确认删除选中的 ${selectedIds.length} 条实例？`"
          @confirm="removeSelected"
        >
          <a-button v-access:code="['cmdb:instance:delete']" danger>批量删除</a-button>
        </a-popconfirm>
      </a-space>

      <a-table
        :columns="tableColumns"
        :data-source="rows"
        :loading="loading"
        row-key="id"
        size="middle"
        :row-selection="{ selectedRowKeys: selectedIds, onChange: (keys: any) => (selectedIds = keys) }"
        :pagination="{
          current: pageNo,
          pageSize,
          total,
          showTotal: (t: number) => `共 ${t} 条`,
          onChange: (p: number) => { pageNo = p; fetchRows(); },
        }"
        :scroll="{ x: 1200 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key !== 'actions' && column.key !== 'updateTime'">
            <template v-if="column.key && attributes.find((a) => a.code === column.key)">
              <span :style="{ color: attributes.find((a) => a.code === column.key)?.color || undefined }">
                {{ displayValue(record, attributes.find((a) => a.code === column.key)!) }}
              </span>
            </template>
          </template>
          <template v-if="column.key === 'updateTime'">{{ record.updateTime }}</template>
          <template v-if="column.key === 'actions'">
            <a-space>
              <a-button
                v-access:code="['cmdb:instance:query']"
                type="link"
                size="small"
                @click="openTopology(record)"
              >
                拓扑
              </a-button>
              <a-button
                v-access:code="['cmdb:instance:update']"
                type="link"
                size="small"
                @click="openEdit(record)"
              >
                编辑
              </a-button>
              <a-popconfirm title="确认删除该实例？" ok-text="删除" ok-type="danger" cancel-text="取消" @confirm="removeOne(record)">
                <a-button v-access:code="['cmdb:instance:delete']" type="link" size="small" danger>删除</a-button>
              </a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>

      <!-- 动态表单 -->
      <a-modal
        v-model:open="modalVisible"
        :title="editing ? `编辑${currentModel?.name ?? '实例'}` : `新建${currentModel?.name ?? '实例'}`"
        @ok="submit"
        width="720px"
      >
        <a-form v-if="form" layout="vertical">
          <a-row :gutter="16">
            <a-col v-for="attr in attributes" :key="attr.code" :span="attr.attrType === 'textarea' || attr.attrType === 'json' ? 24 : 8">
              <a-form-item :label="attr.name + (attr.required ? ' *' : '')">
                <a-input v-if="controlFor(attr) === 'text' || controlFor(attr) === 'link'" v-model:value="form[attr.code]" />
                <a-input-password v-else-if="controlFor(attr) === 'password'" v-model:value="form[attr.code]" />
                <a-textarea v-else-if="controlFor(attr) === 'textarea'" v-model:value="form[attr.code]" :rows="2" />
                <a-input-number v-else-if="controlFor(attr) === 'number' || controlFor(attr) === 'float'" v-model:value="form[attr.code]" style="width: 100%" />
                <a-switch v-else-if="controlFor(attr) === 'bool'" v-model:checked="form[attr.code]" />
                <a-date-picker
                  v-else-if="controlFor(attr) === 'date'"
                  v-model:value="form[attr.code]"
                  value-format="YYYY-MM-DD"
                  style="width: 100%"
                />
                <a-date-picker
                  v-else-if="controlFor(attr) === 'datetime'"
                  v-model:value="form[attr.code]"
                  value-format="YYYY-MM-DD HH:mm:ss"
                  show-time
                  style="width: 100%"
                />
                <a-select
                  v-else-if="controlFor(attr) === 'select'"
                  v-model:value="form[attr.code]"
                  :options="(attr.choices ?? []).map((c) => ({ value: c.value, label: c.label }))"
                  allow-clear
                />
                <a-select
                  v-else-if="controlFor(attr) === 'multi_select'"
                  v-model:value="form[attr.code]"
                  :options="(attr.choices ?? []).map((c) => ({ value: c.value, label: c.label }))"
                  mode="multiple"
                  allow-clear
                />
                <a-textarea
                  v-else-if="controlFor(attr) === 'json'"
                  v-model:value="form[attr.code]"
                  :rows="3"
                  placeholder="JSON 对象或数组，例如 {&quot;key&quot;: &quot;value&quot;}"
                />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>

      <a-modal
        v-model:open="batchModalVisible"
        :title="`批量修改 ${selectedIds.length} 条${currentModel?.name ?? '实例'}`"
        :confirm-loading="batchSubmitting"
        :ok-button-props="{ disabled: !batchAttributeCodes.length }"
        ok-text="应用修改"
        width="720px"
        @ok="submitBatchUpdate"
      >
        <a-alert
          type="info"
          show-icon
          message="只修改已选择的字段；留空会清除可选字段，必填字段留空将拒绝整批操作。"
          style="margin-bottom: 16px"
        />
        <a-form layout="vertical">
          <a-form-item label="选择要修改的字段" required>
            <a-select
              v-model:value="batchAttributeCodes"
              mode="multiple"
              :options="attributes.map((attr) => ({ value: attr.code, label: attr.name }))"
              placeholder="可选择一个或多个字段"
              @change="changeBatchAttributes"
            />
          </a-form-item>
          <a-row :gutter="16">
            <a-col v-for="attr in batchAttributes" :key="attr.code" :span="attr.attrType === 'textarea' || attr.attrType === 'json' ? 24 : 12">
              <a-form-item :label="attr.name + (attr.required ? ' *' : '')">
                <a-input v-if="controlFor(attr) === 'text' || controlFor(attr) === 'link'" v-model:value="batchForm[attr.code]" allow-clear />
                <a-input-password v-else-if="controlFor(attr) === 'password'" v-model:value="batchForm[attr.code]" />
                <a-textarea v-else-if="controlFor(attr) === 'textarea'" v-model:value="batchForm[attr.code]" :rows="2" allow-clear />
                <a-input-number v-else-if="controlFor(attr) === 'number' || controlFor(attr) === 'float'" v-model:value="batchForm[attr.code]" style="width: 100%" />
                <a-select
                  v-else-if="controlFor(attr) === 'bool'"
                  v-model:value="batchForm[attr.code]"
                  :options="[{ value: true, label: '是' }, { value: false, label: '否' }]"
                  allow-clear
                  placeholder="选择是或否"
                />
                <a-date-picker
                  v-else-if="controlFor(attr) === 'date'"
                  v-model:value="batchForm[attr.code]"
                  value-format="YYYY-MM-DD"
                  allow-clear
                  style="width: 100%"
                />
                <a-date-picker
                  v-else-if="controlFor(attr) === 'datetime'"
                  v-model:value="batchForm[attr.code]"
                  value-format="YYYY-MM-DD HH:mm:ss"
                  show-time
                  allow-clear
                  style="width: 100%"
                />
                <a-select
                  v-else-if="controlFor(attr) === 'select'"
                  v-model:value="batchForm[attr.code]"
                  :options="(attr.choices ?? []).map((choice) => ({ value: choice.value, label: choice.label }))"
                  allow-clear
                />
                <a-select
                  v-else-if="controlFor(attr) === 'multi_select'"
                  v-model:value="batchForm[attr.code]"
                  :options="(attr.choices ?? []).map((choice) => ({ value: choice.value, label: choice.label }))"
                  mode="multiple"
                  allow-clear
                />
                <a-textarea
                  v-else-if="controlFor(attr) === 'json'"
                  v-model:value="batchForm[attr.code]"
                  :rows="3"
                  placeholder="JSON 对象或数组；留空表示清除"
                  allow-clear
                />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>

      <a-drawer
        v-model:open="topologyVisible"
        title="配置项关系拓扑"
        width="1040"
      >
        <a-space wrap style="margin-bottom: 12px">
          <a-tag color="blue">根实例 #{{ topologyRoot?.id }}</a-tag>
          <span>展开深度</span>
          <a-select
            v-model:value="topologyDepth"
            :options="[1, 2, 3, 4].map((value) => ({ value, label: `${value} 层` }))"
            style="width: 90px"
            @change="fetchTopology"
          />
          <a-button :loading="topologyLoading" @click="fetchTopology">刷新</a-button>
          <span v-if="topology" style="color: var(--ant-color-text-secondary)">
            {{ topology.nodes.length }} 个节点 · {{ topology.edges.length }} 条关系
          </span>
        </a-space>

        <a-alert
          v-if="topology?.truncated"
          type="warning"
          show-icon
          message="拓扑已达到 80 个节点或关系数量上限，请降低深度或从其他节点继续查看。"
          style="margin-bottom: 12px"
        />

        <a-space v-if="topologyModels.length" wrap style="margin-bottom: 12px">
          <span style="color: var(--ant-color-text-secondary)">模型图例：</span>
          <a-tag
            v-for="model in topologyModels"
            :key="model.code"
            :color="topologyColor(model.code)"
          >
            {{ model.name }}
          </a-tag>
        </a-space>

        <a-spin :spinning="topologyLoading">
          <div class="topology-canvas">
            <a-empty
              v-if="topology && topology.nodes.length === 1 && !topology.edges.length"
              description="该实例尚未建立关系"
              class="topology-empty"
            />
            <svg
              v-if="topology"
              :viewBox="`0 0 ${topologyLayout.width} ${topologyLayout.height}`"
              role="img"
              aria-label="配置项关系拓扑"
            >
              <defs>
                <marker id="topology-arrow" marker-width="8" marker-height="8" ref-x="8" ref-y="4" orient="auto">
                  <path d="M0,0 L8,4 L0,8 Z" fill="#94a3b8" />
                </marker>
              </defs>
              <g v-for="edge in topologyLayout.edges" :key="edge.id">
                <line
                  :x1="edge.x1"
                  :y1="edge.y1"
                  :x2="edge.x2"
                  :y2="edge.y2"
                  stroke="#94a3b8"
                  stroke-width="1.6"
                  marker-end="url(#topology-arrow)"
                />
                <text
                  v-if="topologyLayout.edges.length <= 40"
                  :x="edge.labelX"
                  :y="edge.labelY"
                  class="topology-edge-label"
                  text-anchor="middle"
                >
                  {{ topologyLabel(edge.relation, 16) }}
                </text>
              </g>
              <g
                v-for="node in topologyLayout.nodes"
                :key="node.id"
                class="topology-node"
                :transform="`translate(${node.x - 70}, ${node.y - 27})`"
                @click="topologySelectedId = node.id"
              >
                <rect
                  width="140"
                  height="54"
                  rx="9"
                  :fill="topologyColor(node.modelCode)"
                  :stroke="node.id === topologySelectedId ? '#faad14' : node.root ? '#10239e' : '#fff'"
                  :stroke-width="node.id === topologySelectedId || node.root ? 4 : 2"
                />
                <text x="70" y="22" text-anchor="middle" class="topology-node-title">
                  {{ topologyLabel(node.label) }}
                </text>
                <text x="70" y="41" text-anchor="middle" class="topology-node-model">
                  {{ topologyLabel(node.modelName, 20) }} · #{{ node.id }}
                </text>
              </g>
            </svg>
          </div>
        </a-spin>

        <a-descriptions
          v-if="topologySelectedNode"
          title="节点详情"
          :column="4"
          bordered
          size="small"
          style="margin-top: 16px"
        >
          <a-descriptions-item label="实例">#{{ topologySelectedNode.id }}</a-descriptions-item>
          <a-descriptions-item label="标识">{{ topologySelectedNode.label }}</a-descriptions-item>
          <a-descriptions-item label="模型">{{ topologySelectedNode.modelName }}</a-descriptions-item>
          <a-descriptions-item label="距离">{{ topologySelectedNode.depth }} 层</a-descriptions-item>
        </a-descriptions>
      </a-drawer>
    </div>
  </Page>
</template>

<style scoped>
.topology-canvas {
  position: relative;
  min-height: 560px;
  overflow: auto;
  border: 1px solid var(--ant-color-border-secondary);
  border-radius: 10px;
  background:
    radial-gradient(circle at center, rgb(22 119 255 / 7%), transparent 45%),
    var(--ant-color-bg-layout);
}

.topology-canvas svg {
  display: block;
  min-width: 920px;
  min-height: 560px;
}

.topology-empty {
  position: absolute;
  z-index: 2;
  top: 32px;
  right: 0;
  left: 0;
}

.topology-edge-label {
  font-size: 11px;
  fill: var(--ant-color-text-secondary);
  paint-order: stroke;
  stroke: var(--ant-color-bg-container);
  stroke-width: 4px;
}

.topology-node {
  cursor: pointer;
  filter: drop-shadow(0 3px 5px rgb(15 23 42 / 16%));
  transition: filter 0.2s ease;
}

.topology-node:hover {
  filter: drop-shadow(0 5px 8px rgb(15 23 42 / 28%));
}

.topology-node-title,
.topology-node-model {
  pointer-events: none;
  fill: #fff;
}

.topology-node-title {
  font-size: 13px;
  font-weight: 600;
}

.topology-node-model {
  font-size: 10px;
  opacity: 0.84;
}
</style>
