<script lang="ts" setup>
import { computed, onActivated, onMounted, ref } from 'vue';

import { Page } from '@vben/common-ui';

import {
  Button,
  Input,
  message,
  Popconfirm,
  Select,
  Space,
  Table,
  Tag,
  Textarea,
} from 'ant-design-vue';

import {
  approveTicket,
  deliverTicket,
  getResourceTicketList,
} from '#/api/scan/resource-ticket';
import { getFlowDefinitionPage, startFlowInstance } from '#/api/asset-ops/flow';
import { getBusinessApplicationList } from '#/api/scan/business-application';
import { getUserPage } from '#/api/system/user';
import { useUserStore } from '@vben/stores';

interface NetworkTicket {
  id?: number;
  created_by?: string;
  applicant_user_id?: number;
  ecs_name?: string;
  ticket_status?: string;
  risk_level?: string;
  applicant_name?: string;
  application_name?: string;
  fw_firewall_name?: string;
  fw_source_zone?: string;
  fw_source_address?: string;
  fw_dest_zone?: string;
  fw_dest_address?: string;
  fw_dest_port?: string;
  fw_protocol?: string;
  fw_direction?: string;
  fw_valid_until?: string;
  created_at?: string;
  approve_comment?: string;
  flowInstanceId?: number;
  flowStatus?: string;
}

const data = ref<NetworkTicket[]>([]);
const loading = ref(false);
const modalVisible = ref(false);
const submitting = ref(false);
const applications = ref<any[]>([]);
const users = ref<any[]>([]);

const activeTab = ref('pending');

const form = ref({
  ecs_name: '',
  application_id: undefined as number | undefined,
  application_name: '',

  applicant_user_id: undefined as number | undefined,
  applicant_name: '',
  fw_firewall_name: '',
  fw_source_zone: '',
  fw_source_address: '',
  fw_source_port: '',
  fw_dest_zone: '',
  fw_dest_address: '',
  fw_dest_port: '',
  fw_protocol: 'TCP',
  fw_direction: '正向',
  fw_valid_until: '',
  remarks: '',
  department: '',
  supervisorUsername: '',
});

const statusMap: Record<string, { color: string; label: string }> = {
  pending_approval: { color: 'orange', label: '待审批' },
  pending_delivery: { color: 'purple', label: '待下发' },
  delivered: { color: 'green', label: '已下发' },
  rejected: { color: 'red', label: '已拒绝' },
};

const userStore = useUserStore();
const tabs = [
  { key: 'mine', label: '我的申请', statuses: [] },
  { key: 'approval', label: '待审批', statuses: ['pending_approval'] },
  { key: 'deploy', label: '待下发', statuses: ['pending_delivery'] },
  { key: 'completed', label: '已完成', statuses: ['delivered', 'rejected'] },
  { key: 'all', label: '全部', statuses: [] },
];

const filteredTickets = computed(() => {
  const tab = tabs.find((item) => item.key === activeTab.value);
  if (!tab) return data.value;
  if (tab.key === 'mine') {
    const uid = userStore.userInfo?.id;
    return data.value.filter((t: any) => t.applicant_user_id === uid || t.created_by === userStore.userInfo?.username);
  }
  if (!tab.statuses.length) return data.value;
  return data.value.filter((ticket) => tab.statuses.includes(ticket.ticket_status || ''));
});

const columns: any[] = [
  { title: '策略单名称', dataIndex: 'ecs_name', key: 'ecs_name', width: 200 },
  { title: '归属应用', dataIndex: 'application_name', key: 'application_name', width: 120 },
  { title: '防火墙', dataIndex: 'fw_firewall_name', key: 'fw_firewall_name', width: 100 },
  { title: '源地址 → 目的地址', key: 'addresses', width: 240 },
  { title: '端口', dataIndex: 'fw_dest_port', key: 'fw_dest_port', width: 80 },
  { title: '协议', dataIndex: 'fw_protocol', key: 'fw_protocol', width: 70 },
  { title: '风险', dataIndex: 'risk_level', key: 'risk_level', width: 80 },
  { title: '状态', dataIndex: 'ticket_status', key: 'ticket_status', width: 90 },
  { title: '申请人', dataIndex: 'applicant_name', key: 'applicant_name', width: 90 },
  { title: '申请日期', dataIndex: 'created_at', key: 'created_at', width: 105 },
  { title: '操作', key: 'actions', width: 220, fixed: 'right' },
];

async function fetchData() {
  loading.value = true;
  try {
    const [tickets, appRows, userPage] = await Promise.all([
      getResourceTicketList(),
      getBusinessApplicationList(),
      getUserPage({ pageNo: 1, pageSize: 500 }),
    ]);
    const all = tickets as any[];
    // 原生流程路径落单 ticketType=network_change（0067 种子流程），与存量
    // create 类网络工单一并展示。
    data.value = all.filter(
      (ticket) =>
        ticket.resource_type === 'network' &&
        ['create', 'network_change'].includes(ticket.ticket_type || ''),
    );
    applications.value = appRows as any;
    users.value = ((userPage as any)?.list ?? []) as any;
  } catch {
    message.error('加载失败');
  } finally {
    loading.value = false;
  }
}
onMounted(fetchData);
onActivated(fetchData);

function openCreate() {
  form.value = {
    ecs_name: '',
    application_id: undefined,
    application_name: '',
    applicant_user_id: undefined,
    applicant_name: '',
    fw_firewall_name: '',
    fw_source_zone: '',
    fw_source_address: '',
    fw_source_port: '',
    fw_dest_zone: '',
    fw_dest_address: '',
    fw_dest_port: '',
    fw_protocol: 'TCP',
    fw_direction: '正向',
    fw_valid_until: '',
    remarks: '',
    department: '',
    supervisorUsername: '',
  };
  modalVisible.value = true;
}

function changeApplication(appId: number) {
  const app = applications.value.find((item) => item.id === appId);
  form.value.application_id = appId;
  form.value.application_name = app?.name || '';
}



async function handleSubmit() {
  if (!form.value.ecs_name.trim()) {
    message.warning('请输入策略单名称');
    return;
  }
  if (!form.value.fw_dest_address.trim() || !form.value.fw_dest_port.trim()) {
    message.warning('请填写目的地址和端口');
    return;
  }
  // 原生流程：按名字定位启用中的「网络策略申请流程」（0067 种子）。
  const page = await getFlowDefinitionPage({ pageNo: 1, pageSize: 100 });
  const definition = (page?.list ?? []).find(
    (d: any) => d.name === '网络策略申请流程' && d.status === 0,
  );
  if (!definition) {
    message.error('未找到启用中的「网络策略申请流程」定义');
    return;
  }
  const applicantName =
    userStore.userInfo?.nickname || userStore.userInfo?.username || '';
  submitting.value = true;
  try {
    await startFlowInstance({
      definitionId: definition.id,
      title: `${applicantName} 网络策略申请 ${form.value.ecs_name}`,
      payload: {
        applicantName,
        department: form.value.department.trim(),
        supervisorUsername: form.value.supervisorUsername.trim(),
        ecsName: form.value.ecs_name.trim(),
        fwSourceZone: form.value.fw_source_zone.trim(),
        fwSourceAddress: form.value.fw_source_address.trim(),
        fwSourcePort: form.value.fw_source_port.trim(),
        fwFirewallName: form.value.fw_firewall_name.trim(),
        fwDestZone: form.value.fw_dest_zone.trim(),
        fwDestAddress: form.value.fw_dest_address.trim(),
        fwDestPort: form.value.fw_dest_port.trim(),
        fwProtocol: form.value.fw_protocol,
        fwDirection: form.value.fw_direction,
        fwValidUntil: form.value.fw_valid_until,
        reason: form.value.remarks,
      },
    });
    message.success('策略申请已提交流程审批（我的待办/流程实例处理）');
    modalVisible.value = false;
    await fetchData();
  } finally {
    submitting.value = false;
  }
}

async function handleApprove(ticket: NetworkTicket, approved: boolean) {
  const result: any = await approveTicket(ticket.id!, {
    approved,
    comment: approved ? '审批通过' : '审批拒绝',
  });
  message.success(result?.message || '操作成功');
  await fetchData();
}

async function handleDeliver(ticket: NetworkTicket) {
  await deliverTicket(ticket.id!, { comment: '策略已下发设备' });
  message.success('策略已下发，台账已登记');
  await fetchData();
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-space :size="24" style="margin-bottom: 16px">
        <Button
          v-access:code="['infra:resource-ticket:create']"
          type="primary"
          @click="openCreate"
        >
          策略申请
        </Button>
        <Button @click="fetchData">刷新</Button>
        <a-radio-group v-model:value="activeTab" button-style="solid">
          <a-radio-button v-for="tab in tabs" :key="tab.key" :value="tab.key">
            {{ tab.label }}
          </a-radio-button>
        </a-radio-group>
      </a-space>

      <Table
        :columns="columns"
        :data-source="filteredTickets"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 20 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'addresses'">
            {{ record.fw_source_address || '-' }} → {{ record.fw_dest_address || '-' }}
          </template>
          <template v-else-if="column.key === 'risk_level'">
            <Tag :color="record.risk_level === 'high' ? 'red' : 'default'">
              {{ record.risk_level === 'high' ? '高危端口' : '正常' }}
            </Tag>
          </template>
          <template v-else-if="column.key === 'ticket_status'">
            <Tag :color="statusMap[record.ticket_status]?.color || 'default'">
              {{ statusMap[record.ticket_status]?.label || record.ticket_status }}
            </Tag>
          </template>
          <template v-else-if="column.key === 'actions'">
            <Space :size="8">
              <template v-if="record.ticket_status === 'pending_approval'">
                <a-tooltip
                  v-if="record.flowStatus === 'running'"
                  title="原生流程审批中，请到 运营流程 → 流程实例 / 我的待办 处理"
                >
                  <Tag color="blue">流程审批中</Tag>
                </a-tooltip>
                <template v-else>
                  <Button
                    v-access:code="['infra:resource-ticket:approve']"
                    type="link"
                    size="small"
                    @click="handleApprove(record, true)"
                  >
                    审批通过
                  </Button>
                  <Popconfirm title="确认拒绝该策略申请?" @confirm="handleApprove(record, false)">
                    <Button
                      v-access:code="['infra:resource-ticket:approve']"
                      type="link"
                      size="small"
                      danger
                    >
                      拒绝
                    </Button>
                  </Popconfirm>
                </template>
              </template>
              <Popconfirm
                v-if="record.ticket_status === 'pending_delivery'"
                title="确认已在防火墙设备下发该策略?"
                @confirm="handleDeliver(record)"
              >
                <Button
                  v-access:code="['infra:resource-ticket:deliver']"
                  type="link"
                  size="small"
                >
                  确认下发
                </Button>
              </Popconfirm>
            </Space>
          </template>
        </template>
      </Table>

      <!-- eslint-disable-next-line vue/no-v-html -->
      <a-modal
        v-model:open="modalVisible"
        title="网络策略申请"
        :confirm-loading="submitting"
        width="760px"
        @ok="handleSubmit"
      >
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12">
              <a-form-item label="策略单名称" required>
                <Input
                  v-model:value="form.ecs_name"
                  placeholder="如 OA系统-开放443端口"
                />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="归属应用" required>
                <Select
                  v-model:value="form.application_id"
                  :options="applications.map((a) => ({ value: a.id, label: a.name }))"
                  placeholder="请选择已登记的业务应用"
                  show-search
                  option-filter-prop="label"
                  @update:value="(v: any) => changeApplication(Number(v))"
                />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="申请人（当前登录人）">
                <Input :value="userStore.userInfo?.nickname || userStore.userInfo?.username" disabled />
              </a-form-item>
            </a-col>

            <a-col :span="8">
              <a-form-item label="防火墙">
                <Input v-model:value="form.fw_firewall_name" placeholder="如 FW-核心" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="源安全区域">
                <Input v-model:value="form.fw_source_zone" placeholder="如 办公区" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="目的安全区域">
                <Input v-model:value="form.fw_dest_zone" placeholder="如 服务器区" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="源地址" required>
                <Input
                  v-model:value="form.fw_source_address"
                  placeholder="如 10.0.0.0/24"
                />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="目的地址" required>
                <Input
                  v-model:value="form.fw_dest_address"
                  placeholder="如 10.0.1.10"
                />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="源端口">
                <Input v-model:value="form.fw_source_port" placeholder="任意" />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="目的端口" required>
                <Input v-model:value="form.fw_dest_port" placeholder="如 443 或 8000-8010" />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="协议">
                <Select v-model:value="form.fw_protocol">
                  <a-select-option value="TCP">TCP</a-select-option>
                  <a-select-option value="UDP">UDP</a-select-option>
                  <a-select-option value="ANY">ANY</a-select-option>
                </Select>
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="方向">
                <Select v-model:value="form.fw_direction">
                  <a-select-option value="正向">正向</a-select-option>
                  <a-select-option value="反向">反向</a-select-option>
                </Select>
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="有效期至">
                <Input v-model:value="form.fw_valid_until" placeholder="如 2027-12-31" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="部门（流程通知用）">
                <Input v-model:value="form.department" placeholder="所属部门" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="部门主管账号（可选）">
                <Input
                  v-model:value="form.supervisorUsername"
                  placeholder="留空由系统管理员审批"
                />
              </a-form-item>
            </a-col>
            <a-col :span="24">
              <a-form-item label="申请说明">
                <Textarea v-model:value="form.remarks" :rows="2" />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
