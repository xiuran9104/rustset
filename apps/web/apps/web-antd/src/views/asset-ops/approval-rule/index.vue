<script lang="ts" setup>
import { onMounted, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { createApprovalRule, deleteApprovalRule, getApprovalRules, updateApprovalRule, type ApprovalRule } from '#/api/scan/approval-rule';

const data = ref<ApprovalRule[]>([]);
const loading = ref(false);
const modalVisible = ref(false);
const editingId = ref<number>();
const form = ref<ApprovalRule>(defaultForm());

function defaultForm(): ApprovalRule {
  return {
    name: '', resourceType: '', maxCpuCores: null, maxMemoryGb: null,
    maxResourceCount: null, autoProvision: false, status: 0, remarks: '',
  };
}

async function fetchData() {
  loading.value = true;
  try {
    const page = await getApprovalRules();
    data.value = page.list ?? [];
  } catch {
    message.error('加载审批规则失败');
  } finally {
    loading.value = false;
  }
}
onMounted(fetchData);

function openCreate() {
  editingId.value = undefined;
  form.value = defaultForm();
  modalVisible.value = true;
}
function openEdit(row: ApprovalRule) {
  editingId.value = row.id;
  form.value = { ...row };
  modalVisible.value = true;
}
async function handleSubmit() {
  try {
    if (editingId.value) {
      await updateApprovalRule(editingId.value, form.value);
      message.success('规则已更新');
    } else {
      await createApprovalRule(form.value);
      message.success('规则已创建');
    }
    modalVisible.value = false;
    fetchData();
  } catch (error: any) {
    message.error(error?.message || '保存失败');
  }
}
async function handleDelete(id: number) {
  try {
    await deleteApprovalRule(id);
    message.success('已删除');
    fetchData();
  } catch (error: any) {
    message.error(error?.message || '删除失败');
  }
}

const columns = [
  { title: '规则名称', dataIndex: 'name', key: 'name', width: 170 },
  { title: '资源类型', key: 'resourceType', width: 110 },
  { title: 'CPU 上限', key: 'maxCpuCores', width: 95 },
  { title: '内存上限(GB)', key: 'maxMemoryGb', width: 110 },
  { title: '数量上限', key: 'maxResourceCount', width: 95 },
  { title: '自动开通', key: 'autoProvision', width: 90 },
  { title: '状态', key: 'status', width: 80 },
  { title: '备注', dataIndex: 'remarks', key: 'remarks', ellipsis: true },
  { title: '操作', key: 'actions', width: 130, fixed: 'right' },
];
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header
        title="审批规则"
        sub-title="资源工单自动审批阈值：命中的工单自动通过，开启自动开通后直接执行 OpenTofu 配置"
        style="margin-bottom:16px;padding:0"
      />
      <a-space :size="16" style="margin-bottom:16px">
        <a-button v-access:code="['infra:approval-rule:create']" type="primary" @click="openCreate">新建规则</a-button>
        <a-button @click="fetchData">刷新</a-button>
      </a-space>
      <a-table :columns="columns" :data-source="data" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'resourceType'">
            <a-tag v-if="record.resourceType" color="blue">{{ record.resourceType }}</a-tag>
            <a-tag v-else>全部</a-tag>
          </template>
          <template v-if="['maxCpuCores','maxMemoryGb','maxResourceCount'].includes(column.key as string)">
            {{ (record as any)[column.key as string] ?? '不限' }}
          </template>
          <template v-if="column.key === 'autoProvision'">
            <a-tag :color="record.autoProvision ? 'green' : 'default'">{{ record.autoProvision ? '开通' : '仅审批' }}</a-tag>
          </template>
          <template v-if="column.key === 'status'">
            <a-tag :color="record.status === 0 ? 'green' : 'red'">{{ record.status === 0 ? '启用' : '停用' }}</a-tag>
          </template>
          <template v-if="column.key === 'actions'">
            <a-space>
              <a-button v-access:code="['infra:approval-rule:update']" type="link" size="small" @click="openEdit(record)">编辑</a-button>
              <a-popconfirm title="确认删除该规则？" ok-text="删除" ok-type="danger" cancel-text="取消" @confirm="handleDelete(record.id!)">
                <a-button v-access:code="['infra:approval-rule:delete']" type="link" size="small" danger>删除</a-button>
              </a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>

      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑审批规则' : '新建审批规则'" @ok="handleSubmit" width="620px">
        <a-form v-if="form" layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12"><a-form-item label="规则名称" required><a-input v-model:value="form.name" placeholder="例如：小规格自动开通" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="资源类型" extra="留空表示匹配全部类型"><a-input v-model:value="form.resourceType" placeholder="例如：ecs" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="CPU 核数上限"><a-input-number v-model:value="form.maxCpuCores" :min="1" style="width:100%" placeholder="不限" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="内存 GB 上限"><a-input-number v-model:value="form.maxMemoryGb" :min="1" style="width:100%" placeholder="不限" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="数量上限"><a-input-number v-model:value="form.maxResourceCount" :min="1" style="width:100%" placeholder="不限" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="自动开通"><a-switch v-model:checked="form.autoProvision" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="状态"><a-select v-model:value="form.status"><a-select-option :value="0">启用</a-select-option><a-select-option :value="1">停用</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="备注"><a-textarea v-model:value="form.remarks" :rows="2" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
