<script lang="ts" setup>
/**
 * 终端设备台账（资产中心）：办公网IP申报交付后由流程 register_terminal
 * 动作自动入账（MAC/人员/分配IP），页面只读 + 搜索 + 状态筛选。
 */
import { onActivated, onMounted, ref } from 'vue';

import { Page } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import { requestClient } from '#/api/request';
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
];

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
        </template>
      </a-table>
    </div>
  </Page>
</template>
