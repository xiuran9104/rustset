<script lang="ts" setup>
import { computed, onMounted, ref, watch } from 'vue';
import { Page } from '@vben/common-ui';
import { downloadFileFromBlobPart } from '@vben/utils';
import { message } from 'ant-design-vue';
import { requestClient } from '#/api/request';
import {
  createInstance,
  deleteInstance,
  deleteInstances,
  getAttributesByModel,
  getInstancePage,
  getModelList,
  updateInstance,
} from '#/api/cmdb';
import type { CmdbAttribute, CmdbInstance, CmdbModel } from '#/api/cmdb';

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

const currentModel = computed(() => models.value.find((m) => m.id === currentModelId.value));

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
    { title: '操作', key: 'actions', width: 140, fixed: 'right' as const },
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
    case 'number': return undefined;
    case 'float': return undefined;
    case 'multi_select': return [];
    case 'json': return undefined;
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

async function exportCsv() {
  if (!currentModelId.value) return;
  try {
    const blob = (await requestClient.download('/cmdb/instance/export', {
      params: { modelId: currentModelId.value },
    })) as any;
    downloadFileFromBlobPart({
      fileName: `cmdb_instances_model_${currentModelId.value}.csv`,
      source: blob,
    });
  } catch {
    message.error('导出失败');
  }
}

async function downloadTemplate() {
  if (!currentModelId.value) return;
  const blob = await requestClient.download('/cmdb/instance/import-template', {
    params: { modelId: currentModelId.value },
  });
  downloadFileFromBlobPart({
    fileName: `cmdb_instances_model_${currentModelId.value}_template.csv`,
    source: blob,
  });
}

async function importCsv(file: File) {
  if (!currentModelId.value) return false;
  const form_data = new FormData();
  form_data.append('modelId', String(currentModelId.value));
  form_data.append('file', file);
  try {
    const result = (await requestClient.post('/cmdb/instance/import', form_data, {
      headers: { 'Content-Type': 'multipart/form-data' },
    })) as any;
    message.success(`导入完成：新增 ${result?.created ?? 0} 条，失败 ${result?.failed ?? 0} 条`);
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
        <a-button :disabled="!currentModelId || !rows.length" @click="exportCsv">导出 CSV</a-button>
        <a-button v-access:code="['cmdb:instance:create']" :disabled="!currentModelId || !attributes.length" @click="downloadTemplate">下载模板</a-button>
        <a-upload
          :show-upload-list="false"
          :before-upload="importCsv"
          accept=".csv,text/csv"
          :disabled="!currentModelId || !attributes.length"
        >
          <a-button v-access:code="['cmdb:instance:create']" :disabled="!currentModelId || !attributes.length">导入 CSV</a-button>
        </a-upload>
        <a-button
          v-access:code="['cmdb:instance:create']"
          type="primary"
          :disabled="!currentModelId || !attributes.length"
          @click="openCreate"
        >
          新建{{ currentModel?.name || '实例' }}
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
              {{ displayValue(record, attributes.find((a) => a.code === column.key)!) }}
            </template>
          </template>
          <template v-if="column.key === 'updateTime'">{{ record.updateTime }}</template>
          <template v-if="column.key === 'actions'">
            <a-space>
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
                  placeholder='JSON 对象或数组，例如 {"key": "value"}'
                />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
