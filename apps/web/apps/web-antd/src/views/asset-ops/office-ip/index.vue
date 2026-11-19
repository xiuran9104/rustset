<script lang="ts" setup>
/**
 * 办公网IP申报独立页（普通员工自服务入口）：
 * - 申报表单（与申请中心共用 office-ip-form 组件，同一流程定义）
 * - 全生命周期页签（与资源工单页同构）：
 *   全部申报 / 待我审批 / 待配置 / 待交付 / 已完成
 *   待我审批 ← 我的待办（approval 步骤）；待配置/待交付 ← 实例当前步骤
 *   （配置-分配IP / 交付-开通确认，步骤名与 0066 种子定义固定对应）。
 */
import type { AssetOpsFlowApi } from '#/api/asset-ops/flow';

import { computed, onActivated, onMounted, reactive, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import {
  approveFlowStep,
  completeFlowStep,
  getFlowInstancePage,
  getFlowTodo,
  rejectFlowStep,
} from '#/api/asset-ops/flow';
import OfficeIpForm from './office-ip-form.vue';

const FLOW_NAME = '办公网IP申报流程';
const STEP_ASSIGN = '配置-分配IP';
const STEP_DELIVER = '交付-开通确认';

const STATUS_LABEL: Record<string, string> = {
  running: '进行中',
  completed: '已完成',
  failed: '已失败',
  cancelled: '已取消',
};

const instances = ref<any[]>([]);
const todos = ref<AssetOpsFlowApi.FlowTodo[]>([]);
const loading = ref(false);
const activeTab = ref('all');

const applyVisible = ref(false);

async function load() {
  loading.value = true;
  try {
    const [page, todoList] = await Promise.all([
      getFlowInstancePage({ pageNo: 1, pageSize: 100 }),
      getFlowTodo().catch(() => []),
    ]);
    instances.value = (page?.list ?? []).filter(
      (item: any) => item.definitionName === FLOW_NAME,
    );
    // 我的待办中命中本流程的审批步骤（实例标题模板「xx 办公网IP申报」）
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
    count: instances.value.filter((i) => i.currentStepName === STEP_ASSIGN).length,
    key: 'assign',
    label: '待配置',
  },
  {
    count: instances.value.filter((i) => i.currentStepName === STEP_DELIVER).length,
    key: 'deliver',
    label: '待交付',
  },
  {
    count: instances.value.filter((i) => i.status !== 'running').length,
    key: 'done',
    label: '已完成',
  },
]);

const filtered = computed(() => {
  switch (activeTab.value) {
    case 'approval': {
      return todos.value.map((t) => ({
        id: t.instanceId,
        title: t.title,
        stepName: t.stepName,
        stepKey: t.stepKey,
        status: 'running',
        todoType: t.type,
        applicant: (t.payload as any)?.applicantName ?? t.title,
        starter: '',
        start_time: '',
      }));
    }
    case 'assign': {
      return instances.value.filter((i) => i.currentStepName === STEP_ASSIGN);
    }
    case 'deliver': {
      return instances.value.filter((i) => i.currentStepName === STEP_DELIVER);
    }
    case 'done': {
      return instances.value.filter((i) => i.status !== 'running');
    }
    default: {
      return instances.value;
    }
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
    <div class="mx-auto max-w-5xl space-y-3 p-4">
      <div class="flex items-center justify-between">
        <a-page-header
          :title="'办公网 IP 申报'"
          sub-title="终端入网登记 · 审批 → 配置（分配IP）→ 交付（开通确认）"
          style="padding: 0"
        />
        <a-button
          v-access:code="['infra:flow-instance:start']"
          type="primary"
          @click="applyVisible = true"
        >
          发起申报
        </a-button>
      </div>

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
        :columns="[
          { title: '申报', dataIndex: 'title', key: 'title' },
          { title: '申报人', key: 'applicant', width: 100 },
          { title: '当前步骤', key: 'step', width: 150 },
          { title: '状态', key: 'status', width: 90 },
          { title: '发起时间', dataIndex: 'start_time', key: 'start_time', width: 170 },
          { title: '操作', key: 'actions', width: 220, fixed: 'right' },
        ]"
        :data-source="filtered"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 10 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'applicant'">
            {{ record.applicant || record.starter || '-' }}
          </template>
          <template v-else-if="column.key === 'step'">
            {{ record.currentStepName || record.stepName || '-' }}
          </template>
          <template v-else-if="column.key === 'status'">
            <a-tag
              :color="
                record.status === 'completed'
                  ? 'green'
                  : record.status === 'running'
                    ? 'blue'
                    : record.status === 'failed'
                      ? 'red'
                      : 'default'
              "
            >
              {{ STATUS_LABEL[record.status] ?? record.status }}
            </a-tag>
          </template>
          <template v-else-if="column.key === 'actions'">
            <a-space :size="8">
              <template v-if="activeTab === 'approval'">
                <a-button size="small" danger @click="openAction('reject', record)">
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
              <template v-else-if="activeTab === 'assign'">
                <a-button
                  size="small"
                  type="primary"
                  @click="openAction('assign', record)"
                >
                  完成配置
                </a-button>
              </template>
              <template v-else-if="activeTab === 'deliver'">
                <a-button
                  size="small"
                  type="primary"
                  @click="openAction('deliver', record)"
                >
                  确认开通
                </a-button>
              </template>
              <a
                v-else
                href="/ops-center/flow-instance"
                target="_blank"
              >
                流程详情
              </a>
            </a-space>
          </template>
        </template>
      </a-table>

      <!-- 发起申报 -->
      <a-modal
        v-model:open="applyVisible"
        title="办公网 IP 申报"
        :footer="null"
        width="640px"
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
    </div>
  </Page>
</template>
