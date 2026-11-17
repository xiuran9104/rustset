<script lang="ts" setup>
import type { VxeTableGridOptions } from '#/adapter/vxe-table';
import type {
  AssetOpsFlowApi,
} from '#/api/asset-ops/flow';

import { onMounted, reactive, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import {
  ACTION_ICON,
  TableAction,
  useVbenVxeGrid,
} from '#/adapter/vxe-table';
import {
  approveFlowStep,
  cancelFlowInstance,
  getFlowInstance,
  getFlowInstancePage,
  getFlowTodo,
  rejectFlowStep,
  retryFlowStep,
} from '#/api/asset-ops/flow';
import { stepTypeLabel } from '#/views/asset-ops/flow-definition/data';

const STATUS_LABEL: Record<string, string> = {
  running: '进行中',
  completed: '已完成',
  failed: '已失败',
  cancelled: '已取消',
  pending: '待到达',
  active: '待处理',
  done: '完成',
  skipped: '跳过',
};

const columns: VxeTableGridOptions['columns'] = [
  { type: 'seq', title: '序号', width: 50 },
  { field: 'id', title: '实例', width: 90 },
  { field: 'title', title: '标题', minWidth: 180 },
  { field: 'definitionName', title: '流程', minWidth: 140 },
  { field: 'currentStepName', title: '当前步骤', minWidth: 120 },
  {
    field: 'status',
    title: '状态',
    width: 90,
    slots: { default: 'status' },
  },
  { field: 'starter', title: '发起人', width: 100 },
  { field: 'start_time', title: '发起时间', minWidth: 160 },
  {
    field: 'action',
    title: '操作',
    width: 170,
    fixed: 'right',
    slots: { default: 'action' },
  },
];

const [Grid, gridApi] = useVbenVxeGrid({
  gridOptions: {
    columns,
    height: 'auto',
    keepSource: true,
    proxyConfig: {
      ajax: {
        query: async ({ page }) => {
          return await getFlowInstancePage({
            pageNo: page.currentPage,
            pageSize: page.pageSize,
          });
        },
      },
    },
    rowConfig: { keyField: 'id', isHover: true },
    toolbarConfig: { refresh: true, search: false },
  } as VxeTableGridOptions<any>,
  gridEvents: {},
});

/* ---------- 我的待办 ---------- */
const todos = ref<AssetOpsFlowApi.FlowTodo[]>([]);
async function loadTodos() {
  try {
    todos.value = await getFlowTodo();
  } catch {
    todos.value = [];
  }
}

/* ---------- 详情 ---------- */
const detailVisible = ref(false);
const detail = reactive<{
  instance: null | AssetOpsFlowApi.FlowInstance;
  steps: AssetOpsFlowApi.FlowStepInstance[];
}>({ instance: null, steps: [] });
async function openDetail(row: any) {
  const data = await getFlowInstance(row.id);
  detail.instance = data.instance;
  detail.steps = data.steps;
  detailVisible.value = true;
}

/* ---------- 步骤操作 ---------- */
const actionState = reactive({
  visible: false,
  instanceId: 0,
  stepKey: '',
  stepName: '',
  mode: 'approve' as 'approve' | 'complete' | 'reject',
  comment: '',
});
function openAction(todo: AssetOpsFlowApi.FlowTodo, mode: 'approve' | 'reject') {
  actionState.instanceId = todo.instanceId;
  actionState.stepKey = todo.stepKey;
  actionState.stepName = todo.stepName;
  actionState.mode = mode;
  actionState.comment = '';
  actionState.visible = true;
}
async function submitAction() {
  if (actionState.mode === 'reject' && !actionState.comment.trim()) {
    message.error('拒绝必须填写原因');
    return;
  }
  try {
    if (actionState.mode === 'reject') {
      await rejectFlowStep(actionState.instanceId, actionState.stepKey, {
        comment: actionState.comment,
      });
    } else {
      await approveFlowStep(actionState.instanceId, actionState.stepKey, {
        comment: actionState.comment,
      });
    }
    message.success('已处理');
    actionState.visible = false;
    loadTodos();
    gridApi.query();
    if (detailVisible.value && detail.instance) {
      openDetail(detail.instance);
    }
  } catch (error: any) {
    message.error(error?.message ?? '操作失败');
  }
}

async function handleCancel(row: any) {
  await cancelFlowInstance(row.id);
  message.success('实例已取消');
  gridApi.query();
  loadTodos();
}
async function handleRetryStep(step: AssetOpsFlowApi.FlowStepInstance) {
  if (!detail.instance) return;
  await retryFlowStep(detail.instance.id, step.stepKey);
  message.success('已重新执行');
  openDetail(detail.instance);
  gridApi.query();
}

onMounted(loadTodos);
</script>

<template>
  <Page auto-content-height>
    <Grid table-title="流程实例">

      <template #status="{ row }">
        <span>{{ STATUS_LABEL[row.status] ?? row.status }}</span>
      </template>
      <template #action="{ row }">
        <TableAction
          :actions="[
            {
              label: '详情',
              type: 'link',
              icon: ACTION_ICON.VIEW,
              auth: ['infra:flow-instance:query'],
              onClick: openDetail.bind(null, row),
            },
            {
              label: '取消',
              type: 'link',
              danger: true,
              icon: ACTION_ICON.CLOSE,
              ifShow: row.status === 'running',
              auth: ['infra:flow-instance:operate'],
              popConfirm: {
                title: `确认取消实例 F${row.id}？`,
                confirm: handleCancel.bind(null, row),
              },
            },
          ]"
        />
      </template>
    </Grid>

    <!-- 我的待办 -->
    <a-card
      v-if="todos.length > 0"
      class="mt-4"
      :title="`我的待办（${todos.length}）`"
      size="small"
    >
      <div
        v-for="todo in todos"
        :key="todo.stepId"
        class="flex items-center justify-between border-b border-gray-100 py-2 last:border-0 dark:border-gray-800"
      >
        <div>
          <a-tag :color="todo.type === 'approval' ? 'orange' : 'blue'">
            {{ todo.type === 'approval' ? '审批' : '执行' }}
          </a-tag>
          <span class="font-medium">{{ todo.title }}</span>
          <span class="ml-2 text-gray-500">
            {{ todo.stepName }}（F{{ todo.instanceId }}）
          </span>
        </div>
        <div class="flex gap-2">
          <a-button
            v-if="todo.type === 'approval'"
            size="small"
            @click="openAction(todo, 'reject')"
          >
            拒绝
          </a-button>
          <a-button
            size="small"
            type="primary"
            @click="openAction(todo, 'approve')"
          >
            {{ todo.type === 'approval' ? '通过' : '完成' }}
          </a-button>
        </div>
      </div>
    </a-card>

    <!-- 步骤操作弹窗 -->
    <a-modal
      v-model:open="actionState.visible"
      :title="`${actionState.mode === 'reject' ? '拒绝' : '通过'}：${actionState.stepName}`"
      :mask-closable="false"
      @ok="submitAction"
    >
      <a-textarea
        v-model:value="actionState.comment"
        :rows="3"
        :placeholder="actionState.mode === 'reject' ? '拒绝原因（必填）' : '备注（可选）'"
      />
    </a-modal>

    <!-- 详情：步骤时间线 -->
    <a-modal
      v-model:open="detailVisible"
      :title="detail.instance ? `F${detail.instance.id} ${detail.instance.title}` : '详情'"
      width="760px"
      :footer="null"
    >
      <template v-if="detail.instance">
        <div class="mb-4 text-sm text-gray-500">
          流程：{{ detail.instance.definitionName }} ｜ 状态：{{
            STATUS_LABEL[detail.instance.status]
          }}
          ｜ 发起人：{{ detail.instance.starter }}
        </div>
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
                <a-tag class="ml-2">{{ stepTypeLabel(step.type) }}</a-tag>
                <span class="ml-1 text-xs text-gray-400">
                  {{ STATUS_LABEL[step.status] ?? step.status }}
                </span>
                <div v-if="step.operator" class="text-xs text-gray-500">
                  {{ step.operator }}
                  <template v-if="step.finishedAt">
                    ｜ {{ String(step.finishedAt).slice(0, 19) }}
                  </template>
                  <template v-if="step.comment">｜ {{ step.comment }}</template>
                </div>
                <div
                  v-if="step.result?.lastError"
                  class="text-xs text-red-500"
                >
                  {{ step.result.lastError }}
                </div>
                <div
                  v-if="step.result?.approvers && step.status === 'active'"
                  class="text-xs text-blue-500"
                >
                  会签进度：{{ (step.result.approvers ?? []).length }} 人已通过
                </div>
                <div
                  v-if="step.result?.ticketStatus"
                  class="text-xs text-gray-500"
                >
                  工单状态：{{ step.result.ticketStatus }}
                </div>
              </div>
              <a-button
                v-if="step.status === 'failed' && step.type === 'auto'"
                size="small"
                @click="handleRetryStep(step)"
              >
                重试
              </a-button>
            </div>
          </a-timeline-item>
        </a-timeline>
      </template>
    </a-modal>
  </Page>
</template>
