<script lang="ts" setup>
import { onMounted, ref } from 'vue';

import { Page } from '@vben/common-ui';

import {
  Button,
  Card,
  message,
  Modal,
  Select,
  Switch,
  Table,
  Tag,
} from 'ant-design-vue';

import { requestClient } from '#/api/request';
import { fromInfraResponse } from '#/api/scan/compat';

interface AutomationConfig {
  id: number;
  automation_type: string;
  enabled: number;
  credential_ref?: number | null;
  scope_config?: string | null;
  circuit_breaker_threshold: number;
  circuit_breaker_count: number;
  circuit_breaker_tripped: number;
  last_executed_at?: string;
  last_result?: string;
}

interface AuditRow {
  id: number;
  automation_type: string;
  ticket_id?: number;
  action: string;
  result: string;
  detail?: string;
  duration_ms?: number;
  create_time: string;
}

const configs = ref<AutomationConfig[]>([]);
const audits = ref<AuditRow[]>([]);
const loading = ref(false);
const auditLoading = ref(false);
const credVisible = ref(false);
const credSaving = ref(false);
const credForm = ref({ automationType: '', apiUsername: '', apiPassword: '' });
const testing = ref(false);
const testResult = ref('');
const scopeVisible = ref(false);
const scopeSaving = ref(false);
const scopeForm = ref({ configId: 0, vendor: 'dry_run', endpoint: '', policyGroup: '' });
const auditPage = ref(1);
const auditTotal = ref(0);

const typeLabels: Record<string, string> = {
  cloud_provision: '云资源自动化',
  firewall_provision: '防火墙策略自动化',
};

const resultColors: Record<string, string> = {
  executed: 'green',
  failed: 'red',
  skipped_manual: 'orange',
  circuit_open: 'purple',
};

const resultLabels: Record<string, string> = {
  executed: '已执行',
  failed: '执行失败',
  skipped_manual: '跳过（人工执行）',
  circuit_open: '熔断断开',
};

const configColumns: any[] = [
  { title: '自动化类型', key: 'automation_type', width: 180 },
  { title: '开关', key: 'enabled', width: 100 },
  { title: '熔断状态', key: 'circuit', width: 140 },
  { title: '连续失败', key: 'circuit_breaker_count', width: 90 },
  { title: '失败阈值', key: 'circuit_breaker_threshold', width: 90 },
  { title: '最近执行', key: 'last_executed_at', width: 170 },
  { title: '最近结果', key: 'last_result', ellipsis: true },
  { title: '操作', key: 'actions', width: 120, fixed: 'right' },
];

const auditColumns: any[] = [
  { title: '时间', dataIndex: 'create_time', key: 'create_time', width: 170 },
  { title: '类型', key: 'automation_type', width: 140 },
  { title: '工单', dataIndex: 'ticket_id', key: 'ticket_id', width: 70 },
  { title: '结果', key: 'result', width: 120 },
  { title: '详情', dataIndex: 'detail', key: 'detail', ellipsis: true },
  { title: '耗时(ms)', dataIndex: 'duration_ms', key: 'duration_ms', width: 85 },
];

async function fetchConfigs() {
  loading.value = true;
  try {
    const rows = await requestClient.get('/infra/automation-config/list');
    configs.value = fromInfraResponse(rows) as any;
  } catch {
    message.error('加载失败');
  } finally {
    loading.value = false;
  }
}

async function fetchAudits() {
  auditLoading.value = true;
  try {
    const page = await requestClient.get('/infra/automation-audit/page', {
      params: { pageNo: auditPage.value, pageSize: 20 },
    });
    const data = fromInfraResponse(page) as any;
    audits.value = data?.list || [];
    auditTotal.value = data?.total || 0;
  } catch {
    // 审计页加载失败不弹提示
  } finally {
    auditLoading.value = false;
  }
}

async function toggle(record: any, checked: boolean) {
  try {
    await requestClient.put('/infra/automation-config/update', {
      id: record.id,
      enabled: checked ? 1 : 0,
    });
    message.success(
      `${typeLabels[record.automation_type] || record.automation_type} 已${checked ? '开启' : '关闭'}`,
    );
    await fetchConfigs();
  } catch {
    message.error('更新失败');
  }
}

async function openCredential(record: any) {
  credForm.value = { automationType: record.automation_type, apiUsername: '', apiPassword: '' };
  try {
    const cred = await requestClient.get('/infra/automation-credential/get', {
      params: { automationType: record.automation_type },
    });
    if (cred) {
      credForm.value.apiUsername = (cred as any).api_username || '';
    }
  } catch { /* 首次配置为空 */ }
  credVisible.value = true;
}

async function saveCredential() {
  if (!credForm.value.apiUsername || !credForm.value.apiPassword) {
    message.warning('请填写 API 用户名和密码');
    return;
  }
  credSaving.value = true;
  try {
    await requestClient.put('/infra/automation-credential/save', {
      automationType: credForm.value.automationType,
      apiUsername: credForm.value.apiUsername,
      apiPassword: credForm.value.apiPassword,
    });
    message.success('凭据已保存');
    credVisible.value = false;
  } catch {
    message.error('保存失败');
  } finally {
    credSaving.value = false;
  }
}

function openScope(record: any) {
  let parsed = { vendor: 'dry_run', endpoint: '', policyGroup: '' };
  try {
    if (record.scope_config) parsed = JSON.parse(record.scope_config);
  } catch { /* 默认值 */ }
  scopeForm.value = { configId: record.id, ...parsed };
  scopeVisible.value = true;
}

async function saveScope() {
  if (!scopeForm.value.endpoint) {
    message.warning('请填写 API 端点');
    return;
  }
  if (!scopeForm.value.policyGroup) {
    message.warning('请填写策略组名称');
    return;
  }
  scopeSaving.value = true;
  try {
    const config = configs.value.find((c: any) => c.id === scopeForm.value.configId);
    await requestClient.put('/infra/automation-config/update', {
      id: scopeForm.value.configId,
      enabled: config?.enabled ?? 0,
      scopeConfig: JSON.stringify({
        vendor: scopeForm.value.vendor,
        endpoint: scopeForm.value.endpoint,
        policyGroup: scopeForm.value.policyGroup,
      }),
    });
    message.success('连接配置已保存');
    scopeVisible.value = false;
    await fetchConfigs();
  } catch {
    message.error('保存失败');
  } finally {
    scopeSaving.value = false;
  }
}

async function testConnection() {
  testing.value = true;
  testResult.value = '';
  try {
    const result = await requestClient.post('/infra/automation-config/test-connection');
    testResult.value = (result as any)?.message || '未知';
    if ((result as any)?.success) {
      message.success('连接测试通过');
    } else {
      message.warning('连接测试失败');
    }
  } catch {
    testResult.value = '请求失败';
  } finally {
    testing.value = false;
  }
}

async function resetBreaker(record: any) {
  try {
    await requestClient.put('/infra/automation-config/update', {
      id: record.id,
      credentialRef: record.credential_ref,
      scopeConfig: record.scope_config,
      circuitBreakerThreshold: record.circuit_breaker_threshold,
      enabled: record.enabled,
    });
    message.success('已重置熔断计数');
    await fetchConfigs();
  } catch {
    message.error('重置失败');
  }
}

onMounted(() => {
  void fetchConfigs();
  void fetchAudits();
});
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <Card title="自动化执行开关" style="margin-bottom: 16px">
        <template #extra>
          <span style="color: var(--ant-color-text-secondary); font-size: 13px">
            关闭时工单仅做审批留痕，人工执行；开启时审批通过后自动调用受限 API
          </span>
        </template>
        <Table
          :columns="configColumns"
          :data-source="configs"
          :loading="loading"
          row-key="id"
          size="middle"
          :pagination="false"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'automation_type'">
              {{ typeLabels[record.automation_type] || record.automation_type }}
            </template>
            <template v-else-if="column.key === 'enabled'">
              <Switch
                :checked="record.enabled === 1"
                checked-children="开"
                un-checked-children="关"
                @change="(v: any) => toggle(record, v)"
              />
            </template>
            <template v-else-if="column.key === 'circuit'">
              <Tag v-if="record.circuit_breaker_tripped === 1" color="red">
                熔断已触发
              </Tag>
              <Tag v-else-if="record.circuit_breaker_count > 0" color="orange">
                {{ record.circuit_breaker_count }}/{{ record.circuit_breaker_threshold }}
              </Tag>
              <Tag v-else color="green">正常</Tag>
            </template>
            <template v-else-if="column.key === 'last_executed_at'">
              {{ record.last_executed_at || '-' }}
            </template>
            <template v-else-if="column.key === 'last_result'">
              {{ record.last_result || '-' }}
            </template>
            <template v-else-if="column.key === 'actions'">
              <a-space :size="8">
                <Button size="small" @click="openScope(record)">配置</Button>
                <Button size="small" @click="openCredential(record)">凭据</Button>
                <Button
                  v-if="record.automation_type === 'firewall_provision'"
                  size="small"
                  :loading="testing"
                  @click="testConnection"
                >
                  测试
                </Button>
                <Button
                  v-if="record.circuit_breaker_count > 0"
                  size="small"
                  @click="resetBreaker(record)"
                >
                  重置熔断
                </Button>
              </a-space>
            </template>
          </template>
        </Table>
      </Card>

      <Modal
        v-model:open="credVisible"
        :title="`${typeLabels[credForm.automationType] || credForm.automationType} — API 凭据`"
        :confirm-loading="credSaving"
        @ok="saveCredential"
      >
        <a-form layout="vertical">
          <a-form-item label="API 用户名" required>
            <a-input
              v-model:value="credForm.apiUsername"
              placeholder="防火墙最小权限服务账号（仅限策略组操作）"
            />
          </a-form-item>
          <a-form-item label="API 密码" required>
            <a-input-password
              v-model:value="credForm.apiPassword"
              placeholder="密码独立存储，不与云平台凭据共用"
            />
          </a-form-item>
          <a-alert
            v-if="testResult"
            :type="testResult.includes('通过') ? 'success' : 'warning'"
            :message="testResult"
            show-icon
            style="margin-top: 12px"
          />
        </a-form>
      </Modal>

      <Modal
        v-model:open="scopeVisible"
        :title="`${typeLabels[configs.find((c: any) => c.id === scopeForm.configId)?.automation_type || ''] || ''} — 连接配置`"
        :confirm-loading="scopeSaving"
        @ok="saveScope"
      >
        <a-form layout="vertical">
          <a-form-item label="适配器类型">
            <Select v-model:value="scopeForm.vendor" placeholder="选择防火墙适配器">
              <a-select-option value="dry_run">dry_run（测试模式，不实际调设备）</a-select-option>
              <a-select-option value="hillstone">Hillstone（山石网科）</a-select-option>
              <a-select-option value="fortinet">Fortinet（飞塔）</a-select-option>
              <a-select-option value="huawei_usg">Huawei USG（华为）</a-select-option>
              <a-select-option value="generic_rest">Generic REST（通用）</a-select-option>
            </Select>
          </a-form-item>
          <a-form-item label="API 端点" required>
            <a-input
              v-model:value="scopeForm.endpoint"
              placeholder="https://10.0.255.1:443/api"
            />
          </a-form-item>
          <a-form-item label="策略组名称" required>
            <a-input
              v-model:value="scopeForm.policyGroup"
              placeholder="AUTO-GOV（自动化策略专属组）"
            />
          </a-form-item>
          <a-alert
            type="info"
            show-icon
            message="适配器只能操作指定策略组内的策略，不会触碰基线规则；dry_run 模式仅记录日志不实际下发。"
            style="margin-top: 12px"
          />
        </a-form>
      </Modal>

      <Card title="自动化执行审计日志">
        <template #extra>
          <Button size="small" @click="fetchAudits">刷新</Button>
        </template>
        <Table
          :columns="auditColumns"
          :data-source="audits"
          :loading="auditLoading"
          row-key="id"
          size="small"
          :pagination="{
            current: auditPage,
            pageSize: 20,
            total: auditTotal,
            onChange: (p: number) => { auditPage = p; fetchAudits(); },
            showTotal: (t: number) => `共 ${t} 条`,
          }"
        >
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'automation_type'">
              {{ typeLabels[record.automation_type] || record.automation_type }}
            </template>
            <template v-else-if="column.key === 'result'">
              <Tag :color="resultColors[record.result] || 'default'">
                {{ resultLabels[record.result] || record.result }}
              </Tag>
            </template>
          </template>
        </Table>
      </Card>
    </div>
  </Page>
</template>
