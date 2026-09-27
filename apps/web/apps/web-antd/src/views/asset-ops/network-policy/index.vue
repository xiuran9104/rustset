<script lang="ts" setup>
import { Page } from '@vben/common-ui';
import { downloadFileFromBlobPart } from '@vben/utils';
import {
  createNetworkPolicy,
  deleteNetworkPolicy,
  downloadNetworkPolicyImportTemplate,
  exportNetworkPolicyCsv,
  getNetworkPolicyList,
  importNetworkPolicyCsv,
  recheckNetworkPolicyRisks,
  updateNetworkPolicy,
} from '#/api/scan/network-policy';
import type { NetworkPolicy } from '#/api/scan/network-policy';
import { useCrudList } from '../composables/useCrudList';
import { message } from 'ant-design-vue';
import { ref } from 'vue';

const {
  loading, modalVisible, editingId, searchText, form,
  filtered, fetchData, openCreate, openEdit, handleSubmit, handleDelete,
} = useCrudList<NetworkPolicy>({
  api: {
    list: getNetworkPolicyList,
    create: createNetworkPolicy,
    update: updateNetworkPolicy,
    del: deleteNetworkPolicy,
  },
  defaultForm: () => ({
    firewall_name: '', destination_organization: '', destination_project: '',
    source_organization: '', source_project: '', source_security_zone: '',
    source_ip: '', destination_security_zone: '', destination_ip: '',
    service_port: '', applicant: '', application_date: '',
    traffic_direction: '正向', action: 'allow',
    implementer: '', implementation_date: '', delivery_date: '',
  }),
  searchKeys: ['firewall_name', 'source_ip', 'destination_ip', 'source_organization', 'destination_organization', 'applicant'],
});

const columns = [
  { title: '防火墙', dataIndex: 'firewall_name', key: 'firewall_name', width: 110, fixed: 'left' },
  { title: '源IP', dataIndex: 'source_ip', key: 'source_ip', width: 130 },
  { title: '源单位', dataIndex: 'source_organization', key: 'source_organization', width: 140, ellipsis: true },
  { title: '目的IP', dataIndex: 'destination_ip', key: 'destination_ip', width: 130 },
  { title: '目的单位', dataIndex: 'destination_organization', key: 'destination_organization', width: 140, ellipsis: true },
  { title: '服务端口', dataIndex: 'service_port', key: 'service_port', width: 100 },
  { title: '高风险预警', dataIndex: 'high_risk_count', key: 'high_risk_count', width: 105 },
  { title: '流量方向', dataIndex: 'traffic_direction', key: 'traffic_direction', width: 85 },
  { title: '动作', key: 'action', width: 75 },
  { title: '申请人', dataIndex: 'applicant', key: 'applicant', width: 85 },
  { title: '申请日期', dataIndex: 'application_date', key: 'application_date', width: 105 },
  { title: '开通人', dataIndex: 'implementer', key: 'implementer', width: 85 },
  { title: '开通日期', dataIndex: 'implementation_date', key: 'implementation_date', width: 105 },
  { title: '交付日期', dataIndex: 'delivery_date', key: 'delivery_date', width: 105 },
  { title: '操作', key: 'actions', width: 130, fixed: 'right' },
];

const actionMap: Record<string, { color: string; label: string }> = {
  allow: { color: 'green', label: '允许' },
  deny: { color: 'red', label: '拒绝' },
};

const riskChecking = ref(false);
async function handleRiskRecheck() {
  riskChecking.value = true;
  try {
    const result = await recheckNetworkPolicyRisks();
    message.success(
      `已检查 ${result.policiesChecked} 条策略，匹配 ${result.warningsMatched} 条高风险端口预警`,
    );
    await fetchData();
  } finally {
    riskChecking.value = false;
  }
}
async function handleExportCsv() {
  const data = await exportNetworkPolicyCsv();
  downloadFileFromBlobPart({ fileName: 'network-policies.csv', source: data });
}
async function handleTemplate() {
  const data = await downloadNetworkPolicyImportTemplate();
  downloadFileFromBlobPart({ fileName: 'network-policies-import-template.csv', source: data });
}
async function handleImportCsv(file: File) {
  const result: any = await importNetworkPolicyCsv(file);
  message.success(`导入完成：成功 ${result.created || 0} 条，失败 ${result.failed || 0} 条`);
  await fetchData();
  return false;
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="网络策略台账" sub-title="防火墙开通与交付记录（来源：资产采集表·网络策略排查整改表）" style="margin-bottom:16px;padding:0" />
      <a-space :size="24" style="margin-bottom:20px">
        <a-button v-access:code="['infra:network-policy:create']" type="primary" @click="openCreate">新增策略</a-button>
        <a-button v-access:code="['infra:network-policy:update']" :loading="riskChecking" @click="handleRiskRecheck">重新匹配风险</a-button>
        <a-button v-access:code="['infra:network-policy:query']" @click="handleExportCsv">导出 CSV</a-button>
        <a-button v-access:code="['infra:network-policy:create']" @click="handleTemplate">下载模板</a-button>
        <a-upload :show-upload-list="false" accept=".csv,text/csv" :before-upload="handleImportCsv">
          <a-button v-access:code="['infra:network-policy:create']">导入 CSV</a-button>
        </a-upload>
        <a-button @click="fetchData">刷新</a-button>
        <a-input-search v-model:value="searchText" placeholder="搜索防火墙/IP/单位/申请人" style="width:280px" allow-clear />
      </a-space>
      <a-table
        :columns="columns" :data-source="filtered" :loading="loading" row-key="id" size="middle"
        :pagination="{ pageSize: 20, showTotal: (t:number)=>`共 ${t} 条` }" :scroll="{ x: 1500 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'action'">
            <a-tag :color="actionMap[record.action]?.color || 'default'">{{ actionMap[record.action]?.label || record.action }}</a-tag>
          </template>
          <template v-if="column.key === 'high_risk_count'">
            <a-badge
              v-if="record.high_risk_count"
              :count="record.high_risk_count"
              :overflow-count="99"
            />
            <span v-else style="color:var(--ant-color-text-tertiary)">无</span>
          </template>
          <template v-if="column.key === 'source_ip'">
            <div style="font-size:12px;color:var(--ant-color-text-secondary)">{{ record.source_security_zone }}</div>
            {{ record.source_ip }}
          </template>
          <template v-if="column.key === 'destination_ip'">
            <div style="font-size:12px;color:var(--ant-color-text-secondary)">{{ record.destination_security_zone }}</div>
            {{ record.destination_ip }}
          </template>
          <template v-if="column.key === 'actions'">
            <a-space>
              <a-button v-access:code="['infra:network-policy:update']" type="link" size="small" @click="openEdit(record)">编辑</a-button>
              <a-popconfirm title="确认删除该策略记录?" ok-text="删除" ok-type="danger" cancel-text="取消" @confirm="handleDelete(record.id!)">
                <a-button v-access:code="['infra:network-policy:delete']" type="link" size="small" danger>删除</a-button>
              </a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>

      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑网络策略' : '新增网络策略'" @ok="handleSubmit" width="860px">
        <a-form v-if="form" layout="vertical">
          <a-divider orientation="left" orientation-margin="0">端点与防火墙</a-divider>
          <a-row :gutter="16">
            <a-col :span="8"><a-form-item label="防火墙" required><a-input v-model:value="form.firewall_name" placeholder="例如：核心防火墙-A" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="源安全域" required><a-input v-model:value="form.source_security_zone" placeholder="例如：互联网域" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="目的安全域名" required><a-input v-model:value="form.destination_security_zone" placeholder="例如：服务器域" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="源IP" required><a-input v-model:value="form.source_ip" placeholder="支持网段，如 10.0.0.0/24" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="目的IP" required><a-input v-model:value="form.destination_ip" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="服务端口" required extra="保存后自动匹配高风险端口并生成租户风险预警"><a-input v-model:value="form.service_port" placeholder="如 22,443、8080-8090、tcp/3389" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="源端IP所属单位" required><a-input v-model:value="form.source_organization" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="源端IP所属项目" required><a-input v-model:value="form.source_project" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="目的IP所属单位" required><a-input v-model:value="form.destination_organization" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="目的IP所属项目" required><a-input v-model:value="form.destination_project" /></a-form-item></a-col>
          </a-row>
          <a-divider orientation="left" orientation-margin="0">申请与动作</a-divider>
          <a-row :gutter="16">
            <a-col :span="8"><a-form-item label="申请人" required><a-input v-model:value="form.applicant" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="申请日期" required><a-date-picker v-model:value="form.application_date" value-format="YYYY-MM-DD" style="width:100%" /></a-form-item></a-col>
            <a-col :span="4"><a-form-item label="流量方向"><a-select v-model:value="form.traffic_direction"><a-select-option value="正向">正向</a-select-option><a-select-option value="反向">反向</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="4"><a-form-item label="动作"><a-select v-model:value="form.action"><a-select-option value="allow">允许</a-select-option><a-select-option value="deny">拒绝</a-select-option></a-select></a-form-item></a-col>
          </a-row>
          <a-divider orientation="left" orientation-margin="0">开通与交付</a-divider>
          <a-row :gutter="16">
            <a-col :span="8"><a-form-item label="开通人"><a-input v-model:value="form.implementer" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="开通日期"><a-date-picker v-model:value="form.implementation_date" value-format="YYYY-MM-DD" style="width:100%" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="交付日期"><a-date-picker v-model:value="form.delivery_date" value-format="YYYY-MM-DD" style="width:100%" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
