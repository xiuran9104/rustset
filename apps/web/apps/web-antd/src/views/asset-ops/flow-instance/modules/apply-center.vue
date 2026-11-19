<script lang="ts" setup>
/**
 * 统一申请中心：办公网IP申报 / 云资源申请 / 网络策略申请 / 通用流程发起。
 * 各申请类型按名字映射到种子流程定义（0066/0067 迁移），payload 契约见迁移注释。
 * 后端引擎负责：fromContext 动态审批人、auto 参数全量插值（数字保型）、
 * create_ticket 落资源台账、通知模板参数替换。
 */
import { onMounted, reactive, ref } from 'vue';

import { message } from 'ant-design-vue';

import {
  getBusinessApplicationList,
} from '#/api/scan/business-application';
import { getCloudPlatformList } from '#/api/scan/cloud-platform';
import {
  getFlowDefinitionPage,
  startFlowInstance,
} from '#/api/asset-ops/flow';
import OfficeIpForm from '../../office-ip/office-ip-form.vue';

const emit = defineEmits(['success']);

type ApplyType = 'cloud' | 'ip' | 'network' | 'generic';

const pickerVisible = ref(false);
const activeType = ref<ApplyType | null>(null);
const submitting = ref(false);

function openPicker() {
  activeType.value = null;
  pickerVisible.value = true;
}
function pickType(type: ApplyType) {
  activeType.value = type;
  pickerVisible.value = false;
  if (type === 'ip') {
    ipVisible.value = true;
  } else if (type === 'cloud') {
    Object.assign(cloudForm, emptyCloudForm());
    cloudVisible.value = true;
  } else if (type === 'network') {
    Object.assign(networkForm, emptyNetworkForm());
    networkVisible.value = true;
  } else {
    genericPayloadText.value = '{}';
    genericVisible.value = true;
  }
}

/** 按名字定位启用中的流程定义（种子定义契约），找不到返回 0。 */
async function resolveDefinition(name: string): Promise<number> {
  try {
    const page = await getFlowDefinitionPage({ pageNo: 1, pageSize: 100 });
    const hit = (page?.list ?? []).find(
      (d: any) => d.name === name && d.status === 0,
    );
    return hit?.id ?? 0;
  } catch {
    return 0;
  }
}

async function submit(
  definitionName: string,
  title: string,
  payload: Record<string, any>,
) {
  const definitionId = await resolveDefinition(definitionName);
  if (!definitionId) {
    message.error(`未找到启用中的「${definitionName}」定义`);
    return;
  }
  submitting.value = true;
  try {
    await startFlowInstance({ definitionId, title, payload });
    message.success('申请已提交，等待审批');
    ipVisible.value = false;
    cloudVisible.value = false;
    networkVisible.value = false;
    genericVisible.value = false;
    emit('success');
  } catch (error: any) {
    message.error(error?.message ?? '提交失败');
  } finally {
    submitting.value = false;
  }
}

function requireSupervisor(form: { supervisorUsername: string }): boolean {
  if (!form.supervisorUsername.trim()) {
    message.error('部门主管账号为必填项（用于动态指派审批人）');
    return false;
  }
  return true;
}

/* ---------- 办公网 IP 申报（表单在共享组件 OfficeIpForm） ---------- */
const ipVisible = ref(false);

/* ---------- 云资源申请 ---------- */
const CLOUD_FLOW_NAME = '云资源申请流程';
const emptyCloudForm = () => ({
  applicantName: '',
  applicationId: undefined as number | undefined,
  applicationName: '',
  cloudPlatformId: undefined as number | undefined,
  cloudPlatformName: '',
  contractName: '',
  cpuCores: 4,
  dataDisk: '',
  department: '',
  ecsName: '',
  ecsOs: 'Linux',
  expireAt: '',
  ipAddress: '',
  memoryGb: 8,
  reason: '',
  resourceCount: 1,
  resourceType: 'cloud',
  supervisorUsername: '',
  systemDisk: '高性能云硬盘',
  systemDiskSizeGb: 100,
});
const cloudVisible = ref(false);
const cloudForm = reactive(emptyCloudForm());
const platforms = ref<any[]>([]);
const applications = ref<any[]>([]);

async function loadCloudOptions() {
  try {
    const [platformRows, applicationRows] = await Promise.all([
      getCloudPlatformList(),
      getBusinessApplicationList(),
    ]);
    platforms.value = platformRows as any[];
    applications.value = applicationRows as any[];
  } catch {
    // 下拉加载失败不阻断申报（平台/应用名可手填兜底）
  }
}
onMounted(loadCloudOptions);

function handlePlatformChange(platformId: number) {
  const hit = platforms.value.find((item: any) => item.id === platformId);
  cloudForm.cloudPlatformName = hit?.platform_name ?? '';
}
function handleApplicationChange(appId: number) {
  const hit = applications.value.find((item: any) => item.id === appId);
  cloudForm.applicationName = hit?.name ?? '';
}

async function submitCloud() {
  const applicantName = cloudForm.applicantName.trim();
  const department = cloudForm.department.trim();
  const ecsName = cloudForm.ecsName.trim();
  if (!applicantName || !department || !ecsName) {
    message.error('姓名、部门、资源名称为必填项');
    return;
  }
  if (!requireSupervisor(cloudForm)) return;
  await submit(CLOUD_FLOW_NAME, `${applicantName} 云资源申请 ${ecsName}`, {
    applicantName,
    applicationName: cloudForm.applicationName,
    cloudPlatformName: cloudForm.cloudPlatformName,
    contractName: cloudForm.contractName.trim(),
    cpuCores: cloudForm.cpuCores,
    dataDisk: cloudForm.dataDisk.trim(),
    department,
    ecsName,
    ecsOs: cloudForm.ecsOs,
    expireAt: cloudForm.expireAt,
    ipAddress: cloudForm.ipAddress.trim(),
    memoryGb: cloudForm.memoryGb,
    reason: cloudForm.reason.trim(),
    resourceCount: cloudForm.resourceCount,
    resourceType: cloudForm.resourceType,
    supervisorUsername: cloudForm.supervisorUsername.trim(),
    systemDisk: cloudForm.systemDisk,
    systemDiskSizeGb: cloudForm.systemDiskSizeGb,
  });
}

/* ---------- 网络策略申请 ---------- */
const NETWORK_FLOW_NAME = '网络策略申请流程';
const emptyNetworkForm = () => ({
  applicantName: '',
  department: '',
  fwDestAddress: '',
  fwDestPort: '',
  fwDestZone: '',
  fwDirection: '正向',
  fwProtocol: 'TCP',
  fwSourceAddress: '',
  fwSourcePort: '',
  fwSourceZone: '',
  fwValidUntil: '',
  policyName: '',
  reason: '',
  supervisorUsername: '',
});
const networkVisible = ref(false);
const networkForm = reactive(emptyNetworkForm());

async function submitNetwork() {
  const applicantName = networkForm.applicantName.trim();
  const department = networkForm.department.trim();
  const {
    fwSourceAddress,
    fwSourcePort,
    fwSourceZone,
    fwDestAddress,
    fwDestPort,
    fwDestZone,
    policyName,
  } = networkForm;
  if (!applicantName || !department) {
    message.error('姓名、部门为必填项');
    return;
  }
  if (!requireSupervisor(networkForm)) return;
  if (!fwSourceZone.trim() || !fwSourceAddress.trim() || !fwDestZone.trim() || !fwDestAddress.trim() || !fwDestPort.trim()) {
    message.error('源区域、源地址、目的区域、目的地址、目的端口为必填项');
    return;
  }
  const ecsName = policyName.trim() || `${fwSourceAddress}→${fwDestAddress}:${fwDestPort}`;
  await submit(NETWORK_FLOW_NAME, `${applicantName} 网络策略申请`, {
    applicantName,
    department,
    ecsName,
    fwDestAddress: fwDestAddress.trim(),
    fwDestPort: fwDestPort.trim(),
    fwDestZone: fwDestZone.trim(),
    fwDirection: networkForm.fwDirection,
    fwProtocol: networkForm.fwProtocol,
    fwSourceAddress: fwSourceAddress.trim(),
    fwSourcePort: fwSourcePort.trim(),
    fwSourceZone: fwSourceZone.trim(),
    fwValidUntil: networkForm.fwValidUntil,
    reason: networkForm.reason.trim(),
    supervisorUsername: networkForm.supervisorUsername.trim(),
  });
}

/* ---------- 通用流程发起 ---------- */
const genericVisible = ref(false);
const definitions = ref<any[]>([]);
const genericDefinitionId = ref<number | undefined>(undefined);
const genericTitle = ref('');
const genericPayloadText = ref('{}');

async function loadDefinitions() {
  try {
    const page = await getFlowDefinitionPage({ pageNo: 1, pageSize: 100 });
    definitions.value = (page?.list ?? []).filter((d: any) => d.status === 0);
  } catch {
    definitions.value = [];
  }
}

async function submitGeneric() {
  if (!genericDefinitionId.value) {
    message.error('请选择流程定义');
    return;
  }
  let payload: Record<string, any>;
  try {
    const parsed = JSON.parse(genericPayloadText.value || '{}');
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('not an object');
    }
    payload = parsed;
  } catch {
    message.error('payload 必须是合法的 JSON 对象');
    return;
  }
  const definition = definitions.value.find(
    (d: any) => d.id === genericDefinitionId.value,
  );
  submitting.value = true;
  try {
    await startFlowInstance({
      definitionId: genericDefinitionId.value,
      title: genericTitle.value.trim() || definition?.name || '流程实例',
      payload,
    });
    message.success('流程已发起');
    genericVisible.value = false;
    emit('success');
  } catch (error: any) {
    message.error(error?.message ?? '发起失败');
  } finally {
    submitting.value = false;
  }
}

defineExpose({ open: openPicker });
</script>

<template>
  <!-- 申请类型选择 -->
  <a-modal
    v-model:open="pickerVisible"
    title="发起申请"
    :footer="null"
    width="560px"
  >
    <div class="grid grid-cols-2 gap-3">
      <div
        class="cursor-pointer rounded border border-gray-200 p-4 transition hover:border-blue-400 hover:bg-blue-50 dark:border-gray-700 dark:hover:bg-gray-800"
        @click="pickType('cloud')"
      >
        <div class="mb-1 font-medium">☁️ 云资源申请</div>
        <div class="text-xs text-gray-500">
          云主机/物理机规格申请，高规格附加安全会签，落资源工单台账
        </div>
      </div>
      <div
        class="cursor-pointer rounded border border-gray-200 p-4 transition hover:border-blue-400 hover:bg-blue-50 dark:border-gray-700 dark:hover:bg-gray-800"
        @click="pickType('network')"
      >
        <div class="mb-1 font-medium">🛡️ 网络策略申请</div>
        <div class="text-xs text-gray-500">
          防火墙开通申请（源/目的/端口），审批通过自动下发策略
        </div>
      </div>
      <div
        class="cursor-pointer rounded border border-gray-200 p-4 transition hover:border-blue-400 hover:bg-blue-50 dark:border-gray-700 dark:hover:bg-gray-800"
        @click="pickType('ip')"
      >
        <div class="mb-1 font-medium">🌐 办公网 IP 申报</div>
        <div class="text-xs text-gray-500">
          终端入网登记（MAC 地址 + 人员信息），主管动态审批
        </div>
      </div>
      <div
        class="cursor-pointer rounded border border-gray-200 p-4 transition hover:border-blue-400 hover:bg-blue-50 dark:border-gray-700 dark:hover:bg-gray-800"
        @click="pickType('generic')"
      >
        <div class="mb-1 font-medium">⚙️ 通用流程发起</div>
        <div class="text-xs text-gray-500">
          选择任意启用的流程定义，自定义 JSON payload
        </div>
      </div>
    </div>
  </a-modal>

  <!-- 办公网 IP 申报（共享组件，与独立页同源） -->
  <a-modal
    v-model:open="ipVisible"
    title="办公网 IP 申报"
    :footer="null"
    width="640px"
  >
    <OfficeIpForm @submitted="ipVisible = false; emit('success')" />
  </a-modal>

  <!-- 云资源申请 -->
  <a-modal
    v-model:open="cloudVisible"
    title="云资源申请"
    :confirm-loading="submitting"
    width="680px"
    @ok="submitCloud"
  >
    <div class="grid grid-cols-3 gap-x-4 gap-y-2">
      <div>
        <div class="mb-1 text-xs text-gray-500">申报人姓名 *</div>
        <a-input v-model:value="cloudForm.applicantName" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">部门 *</div>
        <a-input v-model:value="cloudForm.department" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">部门主管账号 *</div>
        <a-input v-model:value="cloudForm.supervisorUsername" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">资源名称 *</div>
        <a-input v-model:value="cloudForm.ecsName" size="small" placeholder="生产环境 Web 服务器" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">资源类型</div>
        <a-select v-model:value="cloudForm.resourceType" class="w-full" size="small">
          <a-select-option value="cloud">云资源</a-select-option>
          <a-select-option value="physical">物理资源</a-select-option>
        </a-select>
      </div>
      <div v-if="cloudForm.resourceType === 'cloud'">
        <div class="mb-1 text-xs text-gray-500">云平台</div>
        <a-select
          v-model:value="cloudForm.cloudPlatformId"
          class="w-full"
          size="small"
          show-search
          option-filter-prop="label"
          :options="platforms.map((p: any) => ({ value: p.id, label: p.platform_name }))"
          @change="handlePlatformChange"
        />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">归属应用</div>
        <a-select
          v-model:value="cloudForm.applicationId"
          class="w-full"
          size="small"
          show-search
          option-filter-prop="label"
          allow-clear
          :options="applications.map((a: any) => ({ value: a.id, label: a.name }))"
          @change="handleApplicationChange"
        />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">合同编号</div>
        <a-input v-model:value="cloudForm.contractName" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">数量（>4 触发安全会签）</div>
        <a-input-number v-model:value="cloudForm.resourceCount" :min="1" size="small" class="!w-full" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">CPU 核数</div>
        <a-input-number v-model:value="cloudForm.cpuCores" :min="1" size="small" class="!w-full" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">内存 GB</div>
        <a-input-number v-model:value="cloudForm.memoryGb" :min="1" size="small" class="!w-full" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">操作系统</div>
        <a-select v-model:value="cloudForm.ecsOs" class="w-full" size="small">
          <a-select-option value="Linux">Linux</a-select-option>
          <a-select-option value="Windows">Windows</a-select-option>
        </a-select>
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">系统盘类型</div>
        <a-input v-model:value="cloudForm.systemDisk" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">系统盘 GB</div>
        <a-input-number v-model:value="cloudForm.systemDiskSizeGb" :min="20" size="small" class="!w-full" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">数据盘</div>
        <a-input v-model:value="cloudForm.dataDisk" size="small" placeholder="500GB SSD" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">预分配 IP</div>
        <a-input v-model:value="cloudForm.ipAddress" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">有效期至</div>
        <a-input v-model:value="cloudForm.expireAt" size="small" type="date" />
      </div>
      <div class="col-span-3">
        <div class="mb-1 text-xs text-gray-500">用途说明</div>
        <a-textarea v-model:value="cloudForm.reason" :rows="2" size="small" />
      </div>
    </div>
  </a-modal>

  <!-- 网络策略申请 -->
  <a-modal
    v-model:open="networkVisible"
    title="网络策略申请"
    :confirm-loading="submitting"
    width="640px"
    @ok="submitNetwork"
  >
    <div class="grid grid-cols-2 gap-x-4 gap-y-2">
      <div>
        <div class="mb-1 text-xs text-gray-500">申报人姓名 *</div>
        <a-input v-model:value="networkForm.applicantName" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">部门 *</div>
        <a-input v-model:value="networkForm.department" size="small" />
      </div>
      <div class="col-span-2">
        <div class="mb-1 text-xs text-gray-500">部门主管账号 *</div>
        <a-input v-model:value="networkForm.supervisorUsername" size="small" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">源区域 *</div>
        <a-input v-model:value="networkForm.fwSourceZone" size="small" placeholder="办公网" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">源地址 *</div>
        <a-input v-model:value="networkForm.fwSourceAddress" size="small" placeholder="192.168.1.0/24" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">源端口（留空=任意）</div>
        <a-input v-model:value="networkForm.fwSourcePort" size="small" placeholder="任意" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">目的区域 *</div>
        <a-input v-model:value="networkForm.fwDestZone" size="small" placeholder="生产区" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">目的地址 *</div>
        <a-input v-model:value="networkForm.fwDestAddress" size="small" placeholder="10.0.2.15" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">目的端口 *</div>
        <a-input v-model:value="networkForm.fwDestPort" size="small" placeholder="8080 或 9000-9100" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">协议</div>
        <a-select v-model:value="networkForm.fwProtocol" class="w-full" size="small">
          <a-select-option value="TCP">TCP</a-select-option>
          <a-select-option value="UDP">UDP</a-select-option>
          <a-select-option value="ANY">ANY</a-select-option>
        </a-select>
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">方向</div>
        <a-select v-model:value="networkForm.fwDirection" class="w-full" size="small">
          <a-select-option value="正向">正向</a-select-option>
          <a-select-option value="反向">反向</a-select-option>
        </a-select>
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">策略有效期</div>
        <a-input v-model:value="networkForm.fwValidUntil" size="small" type="date" />
      </div>
      <div class="col-span-2">
        <div class="mb-1 text-xs text-gray-500">策略名称（可选）</div>
        <a-input v-model:value="networkForm.policyName" size="small" placeholder="留空自动生成" />
      </div>
      <div class="col-span-2">
        <div class="mb-1 text-xs text-gray-500">用途说明</div>
        <a-textarea v-model:value="networkForm.reason" :rows="2" size="small" />
      </div>
    </div>
  </a-modal>

  <!-- 通用流程发起 -->
  <a-modal
    v-model:open="genericVisible"
    title="通用流程发起"
    :confirm-loading="submitting"
    width="620px"
    @ok="submitGeneric"
  >
    <div class="space-y-2">
      <div>
        <div class="mb-1 text-xs text-gray-500">流程定义 *</div>
        <a-select
          v-model:value="genericDefinitionId"
          class="w-full"
          size="small"
          show-search
          option-filter-prop="label"
          placeholder="选择启用中的流程定义"
          :options="definitions.map((d: any) => ({ value: d.id, label: d.name }))"
          @dropdown-visible-change="(open: boolean) => open && loadDefinitions()"
        />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">实例标题</div>
        <a-input v-model:value="genericTitle" size="small" placeholder="留空使用流程名" />
      </div>
      <div>
        <div class="mb-1 text-xs text-gray-500">
          payload JSON
          <span class="text-blue-400" v-pre>（{{payload.x}} 在流程内可被所有节点参数引用）</span>
        </div>
        <a-textarea
          v-model:value="genericPayloadText"
          :rows="6"
          size="small"
          class="font-mono"
        />
      </div>
    </div>
  </a-modal>
</template>
