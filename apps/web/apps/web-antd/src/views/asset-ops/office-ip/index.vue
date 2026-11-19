<script lang="ts" setup>
/**
 * 办公网IP申报独立页（普通员工自服务入口，UI 与资源工单页同构）：
 * - 操作区：发起申报 + 关键字搜索 + 刷新
 * - 页签：全部申报 / 待我审批 / 待配置 / 待交付 / 已完成（带计数徽章）
 * - 富表格：申报人/部门/设备 MAC/申请 IP/接入区域等 payload 字段直出
 * - 详情弹窗：a-steps 生命周期步骤条 + a-descriptions 申报字段 + 步骤
 *   时间线（处理人/时间/备注留痕，同资源工单审批记录）
 * 待我审批 ← 我的待办（approval 步骤）；待配置/待交付 ← 实例当前步骤
 * （配置-分配IP / 交付-开通确认，步骤名与 0066 种子定义固定对应）。
 */
import type { AssetOpsFlowApi } from '#/api/asset-ops/flow';

import { computed, onActivated, onMounted, reactive, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import {
  approveFlowStep,
  completeFlowStep,
  getFlowInstance,
  getFlowInstancePage,
  getFlowTodo,
  rejectFlowStep,
} from '#/api/asset-ops/flow';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';
import OfficeIpForm from './office-ip-form.vue';

const FLOW_NAME = '办公网IP申报流程';
const STEP_ASSIGN = '配置-分配IP';
const STEP_DELIVER = '交付-开通确认';

const STATUS_MAP: Record<string, { color: string; label: string }> = {
  cancelled: { color: 'default', label: '已取消' },
  completed: { color: 'green', label: '已完成' },
  failed: { color: 'red', label: '已失败' },
  running: { color: 'blue', label: '进行中' },
};

const instances = ref<any[]>([]);
const todos = ref<AssetOpsFlowApi.FlowTodo[]>([]);
const loading = ref(false);
const activeTab = ref('all');

const applyVisible = ref(false);

/** 行归一：待办行与实例行统一字段（payload 直出申报明细）。 */
function todoRow(t: any) {
  const payload = t.payload ?? {};
  return {
    applicant: payload.applicantName ?? '',
    currentStepName: t.stepName,
    department: payload.department ?? '',
    deviceType: payload.deviceType ?? '',
    hostname: payload.hostname ?? '',
    id: t.instanceId,
    macAddress: payload.macAddress ?? '',
    networkZone: payload.networkZone ?? '',
    payload,
    requestedIp: payload.requestedIp || '自动分配',
    starter: '',
    status: 'running',
    stepKey: t.stepKey,
    title: t.title,
    todoType: t.type,
  };
}

async function load() {
  loading.value = true;
  try {
    const [page, todoList] = await Promise.all([
      getFlowInstancePage({ pageNo: 1, pageSize: 100 }),
      getFlowTodo().catch(() => [] as any),
    ]);
    instances.value = (page?.list ?? []).map((i: any) => ({
      ...i,
      applicant: i.payload?.applicantName ?? i.starter,
      department: i.payload?.department ?? '',
      macAddress: i.payload?.macAddress ?? '',
      requestedIp: i.payload?.requestedIp || '自动分配',
      networkZone: i.payload?.networkZone ?? '',
      deviceType: i.payload?.deviceType ?? '',
      hostname: i.payload?.hostname ?? '',
    })).filter((i: any) => i.definitionName === FLOW_NAME);
    todos.value = (todoList ?? []).filter(
      (t: any) => t.type === 'approval' && String(t.title ?? '').includes('办公网IP申报'),
    ) as any;
  } finally {
    loading.value = false;
  }
}

const tabs = computed(() => [
  { count: instances.value.length, key: 'all', label: '全部申报' },
  { count: todos.value.length, key: 'approval', label: '待我审批' },
  {
    count: instances.value.filter((i) => i.currentStepName === STEP_ASSIGN)
      .length,
    key: 'assign',
    label: '待配置',
  },
  {
    count: instances.value.filter((i) => i.currentStepName === STEP_DELIVER)
      .length,
    key: 'deliver',
    label: '待交付',
  },
  {
    count: instances.value.filter((i) => i.status !== 'running').length,
    key: 'done',
    label: '已完成',
  },
]);

const tabRows = computed(() => {
  switch (activeTab.value) {
    case 'approval': {
      return todos.value.map(todoRow);
    }
    case 'assign': {
      return instances.value.filter(
        (i) => i.currentStepName === STEP_ASSIGN && i.status === 'running',
      );
    }
    case 'deliver': {
      return instances.value.filter(
        (i) => i.currentStepName === STEP_DELIVER && i.status === 'running',
      );
    }
    case 'done': {
      return instances.value.filter((i) => i.status !== 'running');
    }
    default: {
      return instances.value;
    }
  }
});

const { filteredRows: searchedRows, searchText } = useLocalTableSearch(
  tabRows,
  ['title', 'applicant', 'department', 'macAddress', 'status'],
);

const columns = [
  { title: '申报', dataIndex: 'title', key: 'title' },
  { title: '申报人', key: 'applicant', width: 90 },
  { title: '部门', key: 'department', width: 110 },
  { title: '设备 MAC', dataIndex: 'macAddress', key: 'macAddress', width: 160 },
  { title: '申请 IP', dataIndex: 'requestedIp', key: 'requestedIp', width: 110 },
  { title: '接入区域', dataIndex: 'networkZone', key: 'networkZone', width: 90 },
  { title: '当前步骤', key: 'step', width: 130 },
  { title: '状态', key: 'status', width: 85 },
  { title: '发起时间', dataIndex: 'start_time', key: 'start_time', width: 165 },
  { title: '操作', key: 'actions', width: 200, fixed: 'right' },
];

/* ---------- 详情弹窗（步骤条 + 字段 + 时间线，同资源工单审批详情） ---------- */
const detailVisible = ref(false);
const detail = reactive<{
  instance: null | any;
  steps: any[];
}>({ instance: null, steps: [] });

async function openDetail(row: any) {
  try {
    const data = await getFlowInstance(row.id);
    detail.instance = data.instance;
    detail.steps = data.steps ?? [];
    detailVisible.value = true;
  } catch {
    message.error('详情加载失败');
  }
}

/** 生命周期步骤条：提交→审批→配置→交付→完成（failed 终态红）。 */
const stepIndex = computed(() => {
  const i = detail.instance;
  if (!i) return 0;
  if (i.status === 'completed') return 4;
  if (i.status === 'failed') return 1;
  switch (i.currentStepName) {
    case '部门主管审批': return 1;
    case STEP_ASSIGN: return 2;
    case STEP_DELIVER: return 3;
    default: return 1;
  }
});

/* ---------- 操作：审批 / 配置完成 / 交付确认 ---------- */
const actionState = reactive({
  comment: '',
  instanceId: 0,
  mode: '' as '' | 'approve' | 'assign' | 'deliver' | 'reject',
  stepKey: '',
  title: '',
  visible: false,
});

function openAction(
  mode: 'approve' | 'assign' | 'deliver' | 'reject',
  row: any,
) {
  Object.assign(actionState, {
    comment: '',
    instanceId: row.id,
    mode,
    stepKey: row.stepKey,
    title: row.title,
    visible: true,
  });
}

async function submitAction() {
  const { comment, instanceId, mode, stepKey } = actionState;
  if (mode === 'reject' && !comment.trim()) {
    message.error('拒绝必须填写原因');
    return;
  }
  try {
    if (mode === 'reject') {
      await rejectFlowStep(instanceId, stepKey, { comment });
    } else if (mode === 'approve') {
      await approveFlowStep(instanceId, stepKey, { comment });
    } else {
      // 配置/交付：manual complete，备注记录分配 IP 或开通结论
      await completeFlowStep(instanceId, stepKey, {
        comment,
        result: { note: comment },
      });
    }
    message.success('已处理');
    actionState.visible = false;
    load();
  } catch (error: any) {
    message.error(error?.message ?? '操作失败');
  }
}

const ACTION_PLACEHOLDER: Record<string, string> = {
  approve: '审批备注（可选）',
  assign: '分配的 IP 与配置说明（如 192.168.1.101 / 交换机端口 G1/0/12）',
  deliver: '开通确认说明（可选）',
  reject: '拒绝原因（必填）',
};
const ACTION_TITLE: Record<string, string> = {
  approve: '通过',
  assign: '完成配置',
  deliver: '确认开通',
  reject: '拒绝',
};
const actionTitle = computed(
  () => `${ACTION_TITLE[actionState.mode] ?? ''}：${actionState.title}`,
);

onMounted(load);
onActivated(load);
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-page-header
        title="办公网 IP 申报"
        sub-title="终端入网登记 · 审批 → 配置（分配IP）→ 交付（开通确认）"
        style="margin-bottom: 12px; padding: 0"
      />
      <a-space :size="24" style="margin-bottom: 16px">
        <a-button
          v-access:code="['infra:flow-instance:start']"
          type="primary"
          @click="applyVisible = true"
        >
          发起申报
        </a-button>
        <a-input-search
          v-model:value="searchText"
          allow-clear
          placeholder="搜索申报、申请人、部门或 MAC"
          style="width: 280px"
        />
        <a-button @click="load">刷新</a-button>
      </a-space>

      <a-tabs v-model:active-key="activeTab">
        <a-tab-pane v-for="tab in tabs" :key="tab.key">
          <template #tab>
            {{ tab.label }}
            <a-badge
              :count="tab.count"
              :number-style="{ backgroundColor: '#8c8c8c' }"
              style="margin-left: 4px"
            />
          </template>
        </a-tab-pane>
      </a-tabs>

      <a-table
        :columns="columns"
        :data-source="searchedRows"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 15 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'applicant'">
            {{ record.applicant || record.starter || '-' }}
          </template>
          <template v-else-if="column.key === 'department'">
            {{ record.department || '-' }}
          </template>
          <template v-else-if="column.key === 'macAddress'">
            <a-typography-text :copyable="!!record.macAddress" style="font-family: monospace">
              {{ record.macAddress || '-' }}
            </a-typography-text>
          </template>
          <template v-else-if="column.key === 'step'">
            <a-tag v-if="record.currentStepName" color="processing">
              {{ record.currentStepName }}
            </a-tag>
            <span v-else>-</span>
          </template>
          <template v-else-if="column.key === 'status'">
            <a-tag :color="STATUS_MAP[record.status]?.color || 'default'">
              {{ STATUS_MAP[record.status]?.label || record.status }}
            </a-tag>
          </template>
          <template v-else-if="column.key === 'actions'">
            <a-space :size="12" wrap>
              <a-button size="small" @click="openDetail(record)">详情</a-button>
              <template v-if="activeTab === 'approval'">
                <a-button
                  danger
                  size="small"
                  @click="openAction('reject', record)"
                >
                  拒绝
                </a-button>
                <a-button
                  size="small"
                  type="primary"
                  @click="openAction('approve', record)"
                >
                  通过
                </a-button>
              </template>
              <a-button
                v-else-if="activeTab === 'assign'"
                size="small"
                type="primary"
                @click="openAction('assign', record)"
              >
                完成配置
              </a-button>
              <a-button
                v-else-if="activeTab === 'deliver'"
                size="small"
                type="primary"
                @click="openAction('deliver', record)"
              >
                确认开通
              </a-button>
            </a-space>
          </template>
        </template>
      </a-table>

      <!-- 发起申报 -->
      <a-modal
        v-model:open="applyVisible"
        title="办公网 IP 申报"
        :footer="null"
        width="760px"
      >
        <OfficeIpForm @submitted="applyVisible = false; load()" />
      </a-modal>

      <!-- 操作弹窗 -->
      <a-modal
        v-model:open="actionState.visible"
        :title="actionTitle"
        @ok="submitAction"
      >
        <a-textarea
          v-model:value="actionState.comment"
          :rows="3"
          :placeholder="ACTION_PLACEHOLDER[actionState.mode]"
        />
      </a-modal>

      <!-- 详情：步骤条 + 申报字段 + 处理时间线 -->
      <a-modal
        v-model:open="detailVisible"
        :title="detail.instance ? `F${detail.instance.id} ${detail.instance.title}` : '申报详情'"
        width="760px"
        :footer="null"
      >
        <template v-if="detail.instance">
          <a-steps :current="stepIndex" size="small" style="margin-bottom: 24px">
            <a-step title="提交申请" />
            <a-step title="部门主管审批" />
            <a-step title="配置（分配IP）" />
            <a-step title="交付（开通确认）" />
            <a-step title="完成" />
          </a-steps>
          <a-descriptions bordered size="small" :column="2">
            <a-descriptions-item label="申报人">
              {{ detail.instance.payload?.applicantName || detail.instance.starter }}
            </a-descriptions-item>
            <a-descriptions-item label="部门">
              {{ detail.instance.payload?.department || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="设备 MAC" span="2">
              <span style="font-family: monospace">
                {{ detail.instance.payload?.macAddress || '-' }}
              </span>
            </a-descriptions-item>
            <a-descriptions-item label="设备类型">
              {{ detail.instance.payload?.deviceType || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="主机名">
              {{ detail.instance.payload?.hostname || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="申请 IP">
              {{ detail.instance.payload?.requestedIp || '自动分配' }}
            </a-descriptions-item>
            <a-descriptions-item label="接入区域">
              {{ detail.instance.payload?.networkZone || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="使用期限">
              {{ detail.instance.payload?.accessPeriod || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="联系电话">
              {{ detail.instance.payload?.phone || '-' }}
            </a-descriptions-item>
            <a-descriptions-item label="用途说明" span="2">
              {{ detail.instance.payload?.reason || '-' }}
            </a-descriptions-item>
          </a-descriptions>
          <a-divider orientation="left">处理记录</a-divider>
          <a-timeline>
            <a-timeline-item
              v-for="step in detail.steps"
              :key="step.id"
              :color="
                step.status === 'done'
                  ? 'green'
                  : step.status === 'failed'
                    ? 'red'
                    : step.status === 'active'
                      ? 'blue'
                      : 'gray'
              "
            >
              <div class="flex items-center justify-between">
                <div>
                  <span class="font-medium">{{ step.name }}</span>
                  <span class="ml-2 text-xs text-gray-400">{{ step.status }}</span>
                  <div v-if="step.operator" class="text-xs text-gray-500">
                    {{ step.operator }}
                    <template v-if="step.finishedAt">
                      ｜ {{ String(step.finishedAt).slice(0, 19) }}
                    </template>
                    <template v-if="step.comment">｜ {{ step.comment }}</template>
                  </div>
                  <div v-if="step.result?.lastError" class="text-xs text-red-500">
                    {{ step.result.lastError }}
                  </div>
                </div>
              </div>
            </a-timeline-item>
          </a-timeline>
        </template>
      </a-modal>
    </div>
  </Page>
</template>
