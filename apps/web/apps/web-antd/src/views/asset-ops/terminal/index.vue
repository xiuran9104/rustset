<script lang="ts" setup>
/**
 * 终端设备台账（资产中心）：办公网IP申报交付后由流程 register_terminal
 * 动作自动入账（MAC/人员/分配IP），页面只读 + 搜索 + 状态筛选。
 */
import { onActivated, onMounted, reactive, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import { requestClient } from '#/api/request';
import { getFlowDefinitionPage, startFlowInstance } from '#/api/asset-ops/flow';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';

interface Terminal {
  id: number;
  mac_address: string;
  hostname?: string;
  device_type?: string;
  ip_address?: string;
  network_zone?: string;
  access_period?: string;
  applicant_name?: string;
  department?: string;
  employee_id?: string;
  phone?: string;
  reason?: string;
  status: string;
  create_time?: string;
}

const data = ref<Terminal[]>([]);
const loading = ref(false);
const activeTab = ref('active');
const { filteredRows, searchText } = useLocalTableSearch(data as any, [
  'macAddress',
  'hostname',
  'ipAddress',
  'applicantName',
  'department',
]);

const tabs = [
  { key: 'active', label: '在用' },
  { key: 'retired', label: '已退役' },
  { key: 'all', label: '全部' },
];

async function fetchData() {
  loading.value = true;
  try {
    const page = await requestClient.get('/infra/terminal/page', {
      params: { pageNo: 1, pageSize: 200 },
    });
    data.value = ((page as any)?.list ?? []) as Terminal[];
  } catch {
    message.error('加载失败');
  } finally {
    loading.value = false;
  }
}

function currentRows() {
  if (activeTab.value === 'all') return filteredRows.value;
  return filteredRows.value.filter((row: any) => row.status === activeTab.value);
}

const columns = [
  { title: 'MAC 地址', dataIndex: 'macAddress', key: 'macAddress', width: 170 },
  { title: '主机名', dataIndex: 'hostname', key: 'hostname', width: 130 },
  { title: '设备类型', dataIndex: 'deviceType', key: 'device_type', width: 90 },
  { title: '分配 IP', dataIndex: 'ipAddress', key: 'ipAddress', width: 120 },
  { title: '接入区域', dataIndex: 'networkZone', key: 'network_zone', width: 90 },
  { title: '使用期限', dataIndex: 'accessPeriod', key: 'access_period', width: 90 },
  { title: '使用人', dataIndex: 'applicantName', key: 'applicantName', width: 90 },
  { title: '部门', dataIndex: 'department', key: 'department', width: 110 },
  { title: '工号', dataIndex: 'employeeId', key: 'employee_id', width: 90 },
  { title: '状态', key: 'status', width: 85 },
  { title: '入账时间', dataIndex: 'createTime', key: 'create_time', width: 170 },
  { title: '用途', dataIndex: 'reason', key: 'reason' },
  { title: '操作', key: 'actions', width: 110, fixed: 'right' },
];

/* ---------- 申请变更（台账只读，退役/信息变更走「终端变更申请流程」） ---------- */
const changeVisible = ref(false);
const changeSubmitting = ref(false);
const changeTarget = ref<any>(null);
const changeForm = reactive({
  operation: 'retire' as 'retire' | 'update',
  newApplicantName: '',
  newDepartment: '',
  newIpAddress: '',
  newAccessPeriod: '',
  applicantName: '',
  department: '',
  supervisorUsername: '',
  reason: '',
});

function openChange(record: any) {
  changeTarget.value = record;
  Object.assign(changeForm, {
    operation: 'retire',
    newApplicantName: '',
    newDepartment: '',
    newIpAddress: '',
    newAccessPeriod: '',
    applicantName: '',
    department: '',
    supervisorUsername: '',
    reason: '',
  });
  changeVisible.value = true;
}

async function submitChange() {
  const record = changeTarget.value;
  if (!record) return;
  if (!changeForm.applicantName.trim() || !changeForm.department.trim()) {
    message.error('申请人、部门为必填项');
    return;
  }
  if (!changeForm.supervisorUsername.trim()) {
    message.error('部门主管账号为必填项（动态审批人）');
    return;
  }
  const page = await getFlowDefinitionPage({ pageNo: 1, pageSize: 100 });
  const definition = (page?.list ?? []).find(
    (d: any) => d.name === '终端变更申请流程' && d.status === 0,
  );
  if (!definition) {
    message.error('未找到启用中的「终端变更申请流程」定义');
    return;
  }
  changeSubmitting.value = true;
  try {
    await startFlowInstance({
      definitionId: definition.id,
      title: `${changeForm.applicantName.trim()} 终端变更 ${record.macAddress}`,
      payload: {
        applicantName: changeForm.applicantName.trim(),
        department: changeForm.department.trim(),
        supervisorUsername: changeForm.supervisorUsername.trim(),
        macAddress: record.macAddress,
        operation: changeForm.operation,
        newApplicantName: changeForm.newApplicantName.trim(),
        newDepartment: changeForm.newDepartment.trim(),
        newIpAddress: changeForm.newIpAddress.trim(),
        newAccessPeriod: changeForm.newAccessPeriod.trim(),
        reason: changeForm.reason.trim(),
      },
    });
    message.success('变更申请已提交，等待部门主管审批');
    changeVisible.value = false;
  } catch (error: any) {
    message.error(error?.message ?? '提交失败');
  } finally {
    changeSubmitting.value = false;
  }
}

onMounted(fetchData);
onActivated(fetchData);
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-page-header
        title="终端设备"
        sub-title="办公网 IP 申报交付后自动入账（MAC / 人员 / 分配 IP）"
        style="margin-bottom: 12px; padding: 0"
      />
      <a-space :size="24" style="margin-bottom: 16px">
        <a-input-search
          v-model:value="searchText"
          allow-clear
          placeholder="搜索 MAC、主机名、IP、使用人或部门"
          style="width: 300px"
        />
        <a-button @click="fetchData">刷新</a-button>
        <a-radio-group v-model:value="activeTab" button-style="solid">
          <a-radio-button v-for="tab in tabs" :key="tab.key" :value="tab.key">
            {{ tab.label }}
          </a-radio-button>
        </a-radio-group>
      </a-space>
      <a-table
        :columns="columns"
        :data-source="currentRows()"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 15 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'mac_address'">
            <a-typography-text copyable style="font-family: monospace">
              {{ record.macAddress }}
            </a-typography-text>
          </template>
          <template v-else-if="column.key === 'status'">
            <a-tag :color="record.status === 'active' ? 'green' : 'default'">
              {{ record.status === 'active' ? '在用' : '已退役' }}
            </a-tag>
          </template>
          <template v-else-if="column.key === 'create_time'">
            {{ String(record.createTime ?? '').slice(0, 19) }}
          </template>
          <template v-else-if="column.key === 'actions'">
            <a-button
              v-access:code="['infra:flow-instance:start']"
              size="small"
              :disabled="record.status !== 'active'"
              @click="openChange(record)"
            >
              申请变更
            </a-button>
          </template>
        </template>
      </a-table>

      <!-- 申请变更（退役 / 信息更新，走终端变更申请流程） -->
      <a-modal
        v-model:open="changeVisible"
        title="终端变更申请"
        :confirm-loading="changeSubmitting"
        width="640px"
        @ok="submitChange"
      >
        <div class="mb-3 text-xs text-gray-400">
          目标终端：{{ changeTarget?.macAddress }}（{{ changeTarget?.hostname || '-' }}，
          当前使用人 {{ changeTarget?.applicantName || '-' }}）；提交后由部门主管审批，
          通过后自动更新台账。
        </div>
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="8">
              <a-form-item label="变更类型" required>
                <a-select v-model:value="changeForm.operation">
                  <a-select-option value="retire">退役（停用）</a-select-option>
                  <a-select-option value="update">信息更新（换人/换IP等）</a-select-option>
                </a-select>
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="申请人" required>
                <a-input v-model:value="changeForm.applicantName" />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="部门" required>
                <a-input v-model:value="changeForm.department" />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="部门主管账号" required>
                <a-input v-model:value="changeForm.supervisorUsername" />
              </a-form-item>
            </a-col>
            <template v-if="changeForm.operation === 'update'">
              <a-col :span="8">
                <a-form-item label="新使用人">
                  <a-input v-model:value="changeForm.newApplicantName" placeholder="留空不变" />
                </a-form-item>
              </a-col>
              <a-col :span="8">
                <a-form-item label="新部门">
                  <a-input v-model:value="changeForm.newDepartment" placeholder="留空不变" />
                </a-form-item>
              </a-col>
              <a-col :span="8">
                <a-form-item label="新 IP">
                  <a-input v-model:value="changeForm.newIpAddress" placeholder="留空不变" />
                </a-form-item>
              </a-col>
              <a-col :span="8">
                <a-form-item label="新使用期限">
                  <a-input v-model:value="changeForm.newAccessPeriod" placeholder="留空不变" />
                </a-form-item>
              </a-col>
            </template>
            <a-col :span="24">
              <a-form-item label="变更原因">
                <a-textarea v-model:value="changeForm.reason" :rows="2" />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
