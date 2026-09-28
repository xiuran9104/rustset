<script lang="ts" setup>
import { computed, onUnmounted, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import {
  getInspectionBaseline,
  getInspectionList,
  getInspectionResults,
  runInspection,
  saveInspectionBaseline,
} from '#/api/scan/inspection';
import type {
  InspectionApi,
} from '#/api/scan/inspection';
import { updateRisk } from '#/api/scan/risk';

const loading = ref(false);
const tasks = ref<InspectionApi.InspectionTask[]>([]);
const searchText = ref('');

const stColor: Record<string, string> = {
  pending: 'default',
  queued: 'blue',
  retrying: 'orange',
  running: 'processing',
  completed: 'green',
  failed: 'red',
  cancelled: 'default',
};
const stLabel: Record<string, string> = {
  pending: '待执行',
  queued: '排队中',
  retrying: '等待重试',
  running: '运行中',
  completed: '已完成',
  failed: '失败',
  cancelled: '已取消',
};

const diffLabel: Record<string, string> = {
  unknown_asset: '台账未登记',
  unexpected_port: '端口超基线',
  baseline_missing: '基线未确认',
  no_open_ports: '无开放端口',
};
const sevColor: Record<string, string> = {
  Critical: '#f5222d',
  High: '#fa8c16',
  Medium: '#faad14',
  Low: '#52c41a',
  Info: '#8c8c8c',
};
const riskStColor: Record<string, string> = {
  open: 'red',
  verified: 'orange',
  resolved: 'green',
  ignored: 'default',
  false_positive: 'purple',
  pending_review: 'blue',
};
const riskStLabel: Record<string, string> = {
  open: '未处理',
  verified: '已确认',
  resolved: '已解决',
  ignored: '已忽略',
  false_positive: '误报',
  pending_review: '待审核',
};

const filtered = computed(() => {
  const keyword = searchText.value.trim().toLowerCase();
  if (!keyword) return tasks.value;
  return tasks.value.filter((task) =>
    `${task.name} ${task.target}`.toLowerCase().includes(keyword),
  );
});

const taskColumns = [
  { title: '名称', dataIndex: 'name', width: 170, ellipsis: true },
  { title: '目标', dataIndex: 'target', ellipsis: true },
  { title: '状态', key: 'status', width: 100 },
  { title: '进度', key: 'progress', width: 110 },
  { title: '在线', dataIndex: 'foundAssets', width: 70 },
  { title: '预警', dataIndex: 'foundRisks', width: 70 },
  { title: '开始时间', dataIndex: 'startTime', width: 160 },
  { title: '操作', key: 'actions', width: 170, fixed: 'right' },
];

async function fetchData() {
  loading.value = true;
  try {
    tasks.value = await getInspectionList();
  } catch {
    message.error('加载核查任务失败');
  } finally {
    loading.value = false;
    schedulePoll();
  }
}
fetchData();

// Running tasks finish quickly; refresh automatically so progress and
// results appear without manual polling by the user.
let pollTimer: ReturnType<typeof setInterval> | null = null;
function schedulePoll() {
  const active = tasks.value.some((task) =>
    ['queued', 'retrying', 'running'].includes(task.status),
  );
  if (active && pollTimer === null) {
    pollTimer = setInterval(async () => {
      if (loading.value) return;
      await fetchData();
      if (detailVisible.value && detailTask.value) {
        const fresh = tasks.value.find((task) => task.id === detailTask.value?.id);
        if (fresh) detailTask.value = fresh;
        await loadResults(detailTask.value.id);
      }
    }, 4000);
  } else if (!active && pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}
onUnmounted(() => {
  if (pollTimer !== null) clearInterval(pollTimer);
});

// ---- 新建核查 ----
const runVisible = ref(false);
const runForm = ref({ name: '', targetIps: '', ports: [] as string[] });

async function openRun(task?: InspectionApi.InspectionTask) {
  let ips = task?.target ?? '';
  if (task && task.status !== 'running' && task.completedTargets > 0) {
    // The task row only keeps a summary for multi-IP targets; recover the
    // exact list from the stored results so a re-check covers the same IPs.
    try {
      const rows = await getInspectionResults(task.id);
      if (rows.length > 0) ips = rows.map((row) => row.ip).join('\n');
    } catch {
      // Fall back to the summary text; the user can adjust before starting.
    }
  }
  runForm.value = {
    name: task ? `${task.name}（复核）` : '',
    targetIps: ips,
    ports: (task?.scanPorts ?? []).map(String),
  };
  runVisible.value = true;
}

function parseIps(raw: string): string[] {
  return [
    ...new Set(
      raw
        .split(/[\s,;，；]+/)
        .map((part) => part.trim())
        .filter(Boolean),
    ),
  ];
}

async function handleRun() {
  const ips = parseIps(runForm.value.targetIps);
  // IPv4 dotted quad or IPv6 (full and compressed forms); the backend parser
  // is authoritative, this only catches obvious typos early.
  const ipPattern = /^((\d{1,3}\.){3}\d{1,3}|[0-9a-fA-F:.]+)$/;
  if (ips.length === 0 || ips.length > 64) {
    message.warning('每次核查需要 1–64 个明确的 IP 地址');
    return;
  }
  if (ips.some((ip) => !ipPattern.test(ip))) {
    message.warning('目标必须是有效 IP 地址，不支持域名或网段表达式');
    return;
  }
  const ports = [
    ...new Set(
      runForm.value.ports
        .map((port) => Number.parseInt(port, 10))
        .filter((port) => Number.isInteger(port)),
    ),
  ];
  if (
    ports.length === 0 ||
    ports.length > 128 ||
    ports.some((port) => port < 1 || port > 65_535)
  ) {
    message.warning('端口必须在 1–65535 之间，每次最多 128 个');
    return;
  }
  try {
    await runInspection({
      name: runForm.value.name || undefined,
      ports,
      targetIps: ips,
    });
    message.success('核查任务已排队');
    runVisible.value = false;
    await fetchData();
  } catch {
    message.error('启动核查失败');
  }
}

// ---- 查看结果 ----
const detailVisible = ref(false);
const detailTask = ref<InspectionApi.InspectionTask | null>(null);
const results = ref<InspectionApi.InspectionResult[]>([]);
const resultsLoading = ref(false);

const resultColumns = [
  { title: 'IP', dataIndex: 'ip', width: 140 },
  { title: '台账登记', key: 'registered', width: 90 },
  { title: '基线端口', key: 'baseline', width: 150, ellipsis: true },
  { title: '开放端口', key: 'open', ellipsis: true },
  { title: '不确定端口', key: 'uncertain', width: 130, ellipsis: true },
  { title: '差异', key: 'differences', width: 90 },
  { title: '操作', key: 'actions', width: 110, fixed: 'right' },
];

async function loadResults(taskId: string) {
  resultsLoading.value = true;
  try {
    results.value = await getInspectionResults(taskId);
  } catch {
    message.error('加载核查结果失败');
  } finally {
    resultsLoading.value = false;
  }
}

async function openDetail(task: InspectionApi.InspectionTask) {
  detailTask.value = task;
  detailVisible.value = true;
  await loadResults(task.id);
}

const openRisks = computed(() =>
  results.value.flatMap((result) =>
    result.risks.filter((risk) => risk.status === 'open'),
  ),
);

function portText(ports: number[] | null | undefined) {
  if (!ports || ports.length === 0) return '—';
  return ports.join('、');
}

async function handleRiskStatus(risk: InspectionApi.InspectionRiskRef, status: string) {
  try {
    await updateRisk(risk.id, { status });
    message.success('预警状态已更新');
    if (detailTask.value) await loadResults(detailTask.value.id);
  } catch {
    message.error('更新预警状态失败');
  }
}

// ---- 确认基线 ----
const baselineVisible = ref(false);
const baselineSaving = ref(false);
const baselineForm = ref({
  ip: '',
  allowedPorts: [] as string[],
  reason: '',
  updateTime: '',
  updatedBy: '',
});

async function openBaseline(result: InspectionApi.InspectionResult) {
  baselineForm.value = {
    ip: result.ip,
    allowedPorts: (result.baselinePorts ?? result.openPorts).map(String),
    reason: '',
    updateTime: '',
    updatedBy: '',
  };
  baselineVisible.value = true;
  try {
    const existing = await getInspectionBaseline(result.ip);
    if (existing) {
      baselineForm.value.allowedPorts = existing.allowedPorts.map(String);
      baselineForm.value.reason = existing.reason;
      baselineForm.value.updateTime = existing.updateTime ?? '';
      baselineForm.value.updatedBy = existing.updatedBy;
    }
  } catch {
    message.error('加载端口基线失败');
  }
}

async function handleBaseline() {
  const ports = [
    ...new Set(
      baselineForm.value.allowedPorts
        .map((port) => Number.parseInt(port, 10))
        .filter((port) => Number.isInteger(port)),
    ),
  ];
  if (
    ports.length === 0 ||
    ports.length > 128 ||
    ports.some((port) => port < 1 || port > 65_535)
  ) {
    message.warning('基线端口必须在 1–65535 之间，最多 128 个');
    return;
  }
  if (!baselineForm.value.reason.trim()) {
    message.warning('请填写基线确认依据');
    return;
  }
  baselineSaving.value = true;
  try {
    await saveInspectionBaseline({
      allowedPorts: ports,
      ip: baselineForm.value.ip,
      reason: baselineForm.value.reason.trim(),
    });
    message.success('端口基线已确认');
    baselineVisible.value = false;
    if (detailTask.value) await loadResults(detailTask.value.id);
  } catch {
    message.error('保存端口基线失败');
  } finally {
    baselineSaving.value = false;
  }
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header
        title="资产核查"
        sub-title="扫描结果对比资产台账与端口基线，仅对差异生成预警"
        style="margin-bottom:16px;padding:0"
      />

      <a-card size="small">
        <div class="table-toolbar">
          <a-space :size="16" wrap>
            <a-button
              v-access:code="['infra:task:execute']"
              type="primary"
              @click="openRun()"
            >
              <Icon icon="lucide:scan-search" /> 发起核查
            </a-button>
            <a-input-search
              v-model:value="searchText"
              placeholder="搜索任务或目标"
              allow-clear
              style="width:260px"
            />
          </a-space>
          <a-button @click="fetchData"><Icon icon="lucide:refresh-cw" /> 刷新</a-button>
        </div>

        <a-table
          :columns="taskColumns"
          :data-source="filtered"
          :loading="loading"
          row-key="id"
          size="middle"
          :pagination="{ pageSize: 15 }"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'status'">
              <a-tooltip v-if="record.status === 'failed' && record.errorMessage" :title="record.errorMessage">
                <a-badge status="error" /><a-tag color="red">失败</a-tag>
              </a-tooltip>
              <template v-else>
                <a-badge :status="stColor[record.status]" />
                <a-tag :color="stColor[record.status]">{{ stLabel[record.status] || record.status }}</a-tag>
              </template>
            </template>
            <template v-if="column.key === 'progress'">
              {{ record.completedTargets }}/{{ record.totalTargets }}
            </template>
            <template v-if="column.key === 'actions'">
              <a-space>
                <a-button type="link" size="small" :disabled="record.totalTargets === 0" @click="openDetail(record)">
                  查看结果
                </a-button>
                <a-button
                  v-access:code="['infra:task:execute']"
                  type="link"
                  size="small"
                  :disabled="['queued','retrying','running'].includes(record.status)"
                  @click="openRun(record)"
                >
                  重新核查
                </a-button>
              </a-space>
            </template>
          </template>
        </a-table>
      </a-card>

      <a-modal v-model:open="runVisible" title="发起资产核查" width="560px" @ok="handleRun">
        <a-form layout="vertical">
          <a-form-item label="任务名称">
            <a-input v-model:value="runForm.name" placeholder="例如：核心业务区端口基线核查" />
          </a-form-item>
          <a-form-item label="目标 IP" required>
            <a-textarea
              v-model:value="runForm.targetIps"
              placeholder="每行一个明确的 IP 地址，最多 64 个（不支持域名或网段）"
              :rows="4"
            />
          </a-form-item>
          <a-form-item label="核查端口" required>
            <a-select
              v-model:value="runForm.ports"
              mode="tags"
              :token-separators="[',', ' ', '，']"
              placeholder="输入端口后回车，例如 22、80、443，最多 128 个"
              style="width:100%"
            />
          </a-form-item>
          <a-alert
            type="info"
            show-icon
            message="仅对「台账未登记」和「超出已确认基线的开放端口」生成预警；连接超时视为不确定，不产生预警。"
          />
        </a-form>
      </a-modal>

      <a-drawer v-model:open="detailVisible" width="72%" :title="detailTask?.name ?? '核查结果'">
        <template v-if="detailTask">
          <a-descriptions size="small" :column="3" style="margin-bottom:12px">
            <a-descriptions-item label="目标">{{ detailTask.target }}</a-descriptions-item>
            <a-descriptions-item label="状态">
              <a-tag :color="stColor[detailTask.status]">{{ stLabel[detailTask.status] || detailTask.status }}</a-tag>
            </a-descriptions-item>
            <a-descriptions-item label="开始时间">{{ detailTask.startTime || '—' }}</a-descriptions-item>
          </a-descriptions>
          <a-alert
            v-if="detailTask.status === 'failed' && detailTask.errorMessage"
            type="error"
            :message="detailTask.errorMessage"
            style="margin-bottom:12px"
          />
          <a-alert
            v-else-if="detailTask.status === 'running'"
            type="info"
            show-icon
            message="核查进行中，结果将随进度更新"
            style="margin-bottom:12px"
          />
          <a-alert
            v-else-if="openRisks.length > 0"
            type="warning"
            show-icon
            :message="`仍有 ${openRisks.length} 条未处理预警：请确认资产归属或端口基线后复核`"
            style="margin-bottom:12px"
          />

          <a-table
            :columns="resultColumns"
            :data-source="results"
            :loading="resultsLoading"
            row-key="id"
            size="small"
            :pagination="false"
            :expand-row-by-click="true"
          >
            <template #bodyCell="{ column, record }">
              <template v-if="column.key === 'registered'">
                <a-tag v-if="record.registered" color="green">已登记</a-tag>
                <a-tag v-else color="red">未登记</a-tag>
              </template>
              <template v-if="column.key === 'open'">{{ portText(record.openPorts) }}</template>
              <template v-if="column.key === 'uncertain'">{{ portText(record.uncertainPorts) }}</template>
              <template v-if="column.key === 'baseline'">
                <a-tag v-if="record.baselinePorts" color="blue">{{ record.baselinePorts.length }} 个已确认</a-tag>
                <a-tag v-else>未确认</a-tag>
              </template>
              <template v-if="column.key === 'differences'">
                <a-tooltip :title="record.differences.length > 0 ? '存在差异，展开查看明细' : '无差异'">
                  <a-badge
                    :count="record.differences.length"
                    :number-style="{ backgroundColor: record.differences.length > 0 ? '#fa8c16' : '#52c41a' }"
                  />
                </a-tooltip>
              </template>
              <template v-if="column.key === 'actions'">
                <a-button
                  v-access:code="['infra:asset:update']"
                  type="link"
                  size="small"
                  :disabled="!record.registered"
                  :title="record.registered ? '确认该 IP 允许开放的端口基线' : '请先在资产台账登记该 IP'"
                  @click.stop="openBaseline(record)"
                >
                  确认基线
                </a-button>
              </template>
            </template>
            <template #expandedRowRender="{ record }">
              <div class="result-detail">
                <div class="detail-block">
                  <div class="block-title">差异明细</div>
                  <span v-if="record.differences.length === 0" class="no-diff">无差异，资产台账与端口基线均匹配</span>
                  <ul v-else class="diff-list">
                    <li v-for="(diff, index) in record.differences" :key="index">
                      <a-tag :color="sevColor[diff.severity] || 'default'">{{ diff.severity }}</a-tag>
                      <a-tag>{{ diffLabel[diff.kind] || diff.kind }}</a-tag>
                      <span>{{ diff.description }}</span>
                    </li>
                  </ul>
                </div>
                <div v-if="record.risks.length > 0" class="detail-block">
                  <div class="block-title">差异预警（复核）</div>
                  <a-table
                    :data-source="record.risks"
                    row-key="id"
                    size="small"
                    :pagination="false"
                  >
                    <a-table-column title="端口" data-index="port" :width="70" />
                    <a-table-column title="严重程度" :width="90">
                      <template #default="{ text }">
                        <a-tag :color="sevColor[text] || 'default'">{{ text }}</a-tag>
                      </template>
                    </a-table-column>
                    <a-table-column title="描述" data-index="description" :ellipsis="true" />
                    <a-table-column title="状态" :width="90">
                      <template #default="{ text }">
                        <a-tag :color="riskStColor[text]">{{ riskStLabel[text] || text }}</a-tag>
                      </template>
                    </a-table-column>
                    <a-table-column title="操作" :width="110">
                      <template #default="{ record: risk }">
                        <a-dropdown v-access:code="['infra:risk:update']">
                          <a-button type="link" size="small">复核 <Icon icon="lucide:chevron-down" /></a-button>
                          <template #overlay>
                            <a-menu @click="({ key }: any) => handleRiskStatus(risk, key)">
                              <a-menu-item key="verified">确认预警</a-menu-item>
                              <a-menu-item key="resolved">标记已解决</a-menu-item>
                              <a-menu-item key="false_positive">误报</a-menu-item>
                              <a-menu-item key="ignored">忽略</a-menu-item>
                            </a-menu>
                          </template>
                        </a-dropdown>
                      </template>
                    </a-table-column>
                  </a-table>
                </div>
              </div>
            </template>
          </a-table>
        </template>
      </a-drawer>

      <a-modal
        v-model:open="baselineVisible"
        :title="`确认端口基线 — ${baselineForm.ip}`"
        width="560px"
        :confirm-loading="baselineSaving"
        @ok="handleBaseline"
      >
        <a-form layout="vertical">
          <a-form-item required>
            <template #label>
              允许开放的 TCP 端口
              <span class="label-hint">（基线确认后，超出的开放端口将生成预警）</span>
            </template>
            <a-select
              v-model:value="baselineForm.allowedPorts"
              mode="tags"
              :token-separators="[',', ' ', '，']"
              placeholder="输入端口后回车，例如 22、80、443"
              style="width:100%"
            />
          </a-form-item>
          <a-form-item label="确认依据" required>
            <a-textarea
              v-model:value="baselineForm.reason"
              placeholder="例如：业务系统对外服务清单 2026-09 版确认仅开放以下端口"
              :rows="3"
              :maxlength="1000"
              show-count
            />
          </a-form-item>
          <a-alert
            v-if="baselineForm.updateTime"
            type="info"
            :message="`当前基线由 ${baselineForm.updatedBy} 于 ${baselineForm.updateTime} 确认，保存后将覆盖`"
          />
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>

<style scoped>
.table-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 16px;
}
.result-detail {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 4px 0;
}
.detail-block .block-title {
  margin-bottom: 8px;
  font-weight: 600;
  color: var(--ant-color-text-secondary);
}
.diff-list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.diff-list li {
  display: flex;
  gap: 8px;
  align-items: baseline;
  padding: 3px 0;
}
.no-diff {
  color: var(--ant-color-text-tertiary);
}
.label-hint {
  font-weight: 400;
  color: var(--ant-color-text-tertiary);
}
@media (max-width: 768px) {
  .table-toolbar {
    flex-direction: column;
    align-items: flex-start;
  }
}
</style>
