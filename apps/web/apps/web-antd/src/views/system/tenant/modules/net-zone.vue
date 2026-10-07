<script lang="ts" setup>
import type { SystemTenantApi } from '#/api/system/tenant';

import { computed, ref } from 'vue';

import { useVbenModal } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import { requestClient } from '#/api/request';

interface NetZone {
  id?: number;
  name: string;
  parentId: number;
  zoneType: string;
  cidr?: string;
  sort: number;
  description?: string;
  children?: NetZone[];
}

const tenant = ref<SystemTenantApi.Tenant>();
const tree = ref<NetZone[]>([]);
const loading = ref(false);
const editorOpen = ref(false);
const editingId = ref<number>();
const resolveIp = ref('');
const resolveResult = ref<any>();
const form = ref<NetZone>(defaultForm());

const zoneTypeOptions = [
  { value: 'company', label: '单位' },
  { value: 'subsidiary', label: '下级单位' },
  { value: 'department', label: '部门' },
  { value: 'segment', label: '网段' },
];
const zoneTypeLabel: Record<string, string> = {
  company: '单位',
  department: '部门',
  segment: '网段',
  subsidiary: '下级单位',
};

function defaultForm(): NetZone {
  return {
    cidr: '',
    description: '',
    name: '',
    parentId: 0,
    sort: 0,
    zoneType: 'segment',
  };
}

async function fetchTree() {
  if (!tenant.value?.id) return;
  loading.value = true;
  try {
    const result = (await requestClient.get('/cmdb/net-zone/tree')) as any;
    tree.value = result?.tree ?? [];
  } catch {
    message.error('加载租户网段失败');
  } finally {
    loading.value = false;
  }
}

const [Modal, modalApi] = useVbenModal({
  async onOpenChange(open) {
    if (!open) {
      tenant.value = undefined;
      tree.value = [];
      resolveIp.value = '';
      resolveResult.value = undefined;
      return;
    }
    tenant.value = modalApi.getData<SystemTenantApi.Tenant>();
    await fetchTree();
  },
});

function collectParents(
  nodes: NetZone[],
  depth = 0,
): { label: string; value: number }[] {
  return (nodes ?? []).flatMap((node) => [
    { label: `${'　'.repeat(depth)}${node.name}`, value: node.id! },
    ...collectParents(node.children ?? [], depth + 1),
  ]);
}
const parentOptions = computed(() => [
  { label: '（根节点）', value: 0 },
  ...collectParents(tree.value),
]);

function openCreate(parentId = 0) {
  editingId.value = undefined;
  form.value = { ...defaultForm(), parentId };
  editorOpen.value = true;
}

function openEdit(row: NetZone) {
  editingId.value = row.id;
  form.value = { ...row, children: undefined };
  editorOpen.value = true;
}

async function handleSubmit() {
  if (!tenant.value?.id || !form.value.name.trim()) {
    message.warning('请填写名称');
    return;
  }
  const payload = {
    ...form.value,
    id: editingId.value,
  };
  if (editingId.value) {
    await requestClient.put('/cmdb/net-zone/update', payload);
  } else {
    await requestClient.post('/cmdb/net-zone/create', payload);
  }
  message.success(editingId.value ? '网段已更新' : '网段已创建');
  editorOpen.value = false;
  await fetchTree();
}

async function handleDelete(row: NetZone) {
  await requestClient.delete('/cmdb/net-zone/delete', {
    params: { id: row.id },
  });
  message.success('网段已删除');
  await fetchTree();
}

async function resolve() {
  if (!tenant.value?.id || !resolveIp.value.trim()) return;
  resolveResult.value = await requestClient.post('/cmdb/net-zone/resolve', {
    ip: resolveIp.value,
  });
}
</script>

<template>
  <Modal :title="`${tenant?.name ?? ''} · 网段管理`" class="w-[1000px]">
    <a-space :size="12" style="margin-bottom: 12px" wrap>
      <a-button
        v-access:code="['cmdb:net-zone:create']"
        type="primary"
        @click="openCreate()"
      >
        新建网段
      </a-button>
      <a-input
        v-model:value="resolveIp"
        placeholder="输入 IP 试算归属，例如 10.1.2.3"
        style="width: 280px"
        @press-enter="resolve"
      />
      <a-button @click="resolve">识别归属</a-button>
      <a-tag v-if="resolveResult?.matched" color="green">
        {{ resolveResult.netZoneName }} · {{ resolveResult.organization || '-' }}
      </a-tag>
      <a-tag v-else-if="resolveResult" color="orange">未命中网段</a-tag>
    </a-space>

    <a-table
      :columns="[
        { title: '名称', key: 'name' },
        { title: '类型', key: 'zoneType', width: 100 },
        { title: '网段 (CIDR)', key: 'cidr', width: 180 },
        { title: '说明', key: 'description', ellipsis: true },
        { title: '操作', key: 'actions', width: 210, fixed: 'right' },
      ]"
      :data-source="tree"
      :loading="loading"
      :pagination="false"
      row-key="id"
      size="middle"
    >
      <template #bodyCell="{ column, record }">
        <template v-if="column.key === 'name'">{{ record.name }}</template>
        <template v-else-if="column.key === 'zoneType'">
          <a-tag :color="record.zoneType === 'segment' ? 'blue' : 'geekblue'">
            {{ zoneTypeLabel[record.zoneType] }}
          </a-tag>
        </template>
        <template v-else-if="column.key === 'cidr'">
          <code v-if="record.cidr">{{ record.cidr }}</code>
          <span v-else>-</span>
        </template>
        <template v-else-if="column.key === 'description'">
          {{ record.description || '-' }}
        </template>
        <template v-else-if="column.key === 'actions'">
          <a-space>
            <a-button
              v-access:code="['cmdb:net-zone:create']"
              size="small"
              type="link"
              @click="openCreate(record.id)"
            >
              添加子级
            </a-button>
            <a-button
              v-access:code="['cmdb:net-zone:update']"
              size="small"
              type="link"
              @click="openEdit(record)"
            >
              编辑
            </a-button>
            <a-popconfirm
              cancel-text="取消"
              ok-text="删除"
              title="确认删除？（需先删除子级）"
              @confirm="handleDelete(record)"
            >
              <a-button
                v-access:code="['cmdb:net-zone:delete']"
                danger
                size="small"
                type="link"
              >
                删除
              </a-button>
            </a-popconfirm>
          </a-space>
        </template>
      </template>
    </a-table>

    <a-modal
      v-model:open="editorOpen"
      :title="editingId ? '编辑网段' : '新建网段'"
      width="560px"
      @ok="handleSubmit"
    >
      <a-form layout="vertical">
        <a-row :gutter="16">
          <a-col :span="12">
            <a-form-item label="名称" required>
              <a-input v-model:value="form.name" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="类型">
              <a-select v-model:value="form.zoneType" :options="zoneTypeOptions" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="上级节点">
              <a-select v-model:value="form.parentId" :options="parentOptions" />
            </a-form-item>
          </a-col>
          <a-col :span="12">
            <a-form-item label="网段 (CIDR)" extra="例如 10.1.0.0/16；仅 IPv4">
              <a-input v-model:value="form.cidr" />
            </a-form-item>
          </a-col>
          <a-col :span="6">
            <a-form-item label="排序">
              <a-input-number v-model:value="form.sort" :min="0" class="w-full" />
            </a-form-item>
          </a-col>
          <a-col :span="24">
            <a-form-item label="说明">
              <a-textarea v-model:value="form.description" :rows="2" />
            </a-form-item>
          </a-col>
        </a-row>
      </a-form>
    </a-modal>
  </Modal>
</template>
