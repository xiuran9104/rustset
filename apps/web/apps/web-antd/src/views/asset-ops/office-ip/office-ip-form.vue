<script lang="ts" setup>
/**
 * 办公网 IP 申报表单（共享组件）：申请中心弹窗与独立「办公网IP申报」页
 * 复用同一表单，payload 契约见迁移 0066（办公网IP申报流程）。
 */
import { reactive, ref } from 'vue';

import { message } from 'ant-design-vue';

import {
  getFlowDefinitionPage,
  startFlowInstance,
} from '#/api/asset-ops/flow';

const emit = defineEmits(['submitted']);

const OFFICE_IP_FLOW_NAME = '办公网IP申报流程';

const emptyForm = () => ({
  accessPeriod: '长期',
  applicantName: '',
  department: '',
  deviceType: '笔记本',
  employeeId: '',
  hostname: '',
  macAddress: '',
  networkZone: '办公区',
  phone: '',
  reason: '',
  requestedIp: '',
  supervisorUsername: '',
});

const form = reactive(emptyForm());
const submitting = ref(false);

async function handleSubmit() {
  const applicantName = form.applicantName.trim();
  const department = form.department.trim();
  const macAddress = form.macAddress.trim().toUpperCase();
  if (!applicantName || !department) {
    message.error('姓名、部门为必填项');
    return;
  }
  if (!form.supervisorUsername.trim()) {
    message.error('部门主管账号为必填项（用于动态指派审批人）');
    return;
  }
  if (!/^([0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}$/.test(macAddress)) {
    message.error('MAC 地址格式无效，示例：AA:BB:CC:DD:EE:FF');
    return;
  }
  const requestedIp = form.requestedIp.trim();
  if (requestedIp && !/^(\d{1,3}\.){3}\d{1,3}$/.test(requestedIp)) {
    message.error('申请 IP 格式无效');
    return;
  }
  let definitionId = 0;
  try {
    const page = await getFlowDefinitionPage({ pageNo: 1, pageSize: 100 });
    const hit = (page?.list ?? []).find(
      (d: any) => d.name === OFFICE_IP_FLOW_NAME && d.status === 0,
    );
    if (hit) definitionId = hit.id;
  } catch {
    // 定义列表读取失败按未找到处理
  }
  if (!definitionId) {
    message.error(`未找到启用中的「${OFFICE_IP_FLOW_NAME}」定义`);
    return;
  }
  submitting.value = true;
  try {
    await startFlowInstance({
      definitionId,
      title: `${applicantName} 办公网IP申报`,
      payload: {
        accessPeriod: form.accessPeriod,
        applicantName,
        department,
        deviceType: form.deviceType,
        employeeId: form.employeeId.trim(),
        hostname: form.hostname.trim(),
        macAddress,
        networkZone: form.networkZone,
        phone: form.phone.trim(),
        reason: form.reason.trim(),
        requestedIp,
        supervisorUsername: form.supervisorUsername.trim(),
      },
    });
    message.success('申报已提交，等待部门主管审批');
    Object.assign(form, emptyForm());
    emit('submitted');
  } catch (error: any) {
    message.error(error?.message ?? '提交失败');
  } finally {
    submitting.value = false;
  }
}

defineExpose({ submit: handleSubmit });
</script>

<template>
  <div class="grid grid-cols-2 gap-x-4 gap-y-2">
    <div>
      <div class="mb-1 text-xs text-gray-500">申报人姓名 *</div>
      <a-input v-model:value="form.applicantName" size="small" placeholder="真实姓名" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">部门 *</div>
      <a-input v-model:value="form.department" size="small" placeholder="所属部门" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">工号</div>
      <a-input v-model:value="form.employeeId" size="small" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">联系电话</div>
      <a-input v-model:value="form.phone" size="small" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">设备 MAC 地址 *</div>
      <a-input v-model:value="form.macAddress" size="small" placeholder="AA:BB:CC:DD:EE:FF" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">设备类型</div>
      <a-select v-model:value="form.deviceType" class="w-full" size="small">
        <a-select-option value="台式机">台式机</a-select-option>
        <a-select-option value="笔记本">笔记本</a-select-option>
        <a-select-option value="移动终端">移动终端</a-select-option>
        <a-select-option value="打印机">打印机</a-select-option>
        <a-select-option value="其他">其他</a-select-option>
      </a-select>
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">主机名</div>
      <a-input v-model:value="form.hostname" size="small" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">申请 IP（留空自动分配）</div>
      <a-input v-model:value="form.requestedIp" size="small" placeholder="192.168.1.100" />
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">接入区域</div>
      <a-select v-model:value="form.networkZone" class="w-full" size="small">
        <a-select-option value="办公区">办公区</a-select-option>
        <a-select-option value="访客区">访客区</a-select-option>
        <a-select-option value="研发区">研发区</a-select-option>
      </a-select>
    </div>
    <div>
      <div class="mb-1 text-xs text-gray-500">使用期限</div>
      <a-select v-model:value="form.accessPeriod" class="w-full" size="small">
        <a-select-option value="长期">长期</a-select-option>
        <a-select-option value="3个月">3个月</a-select-option>
        <a-select-option value="6个月">6个月</a-select-option>
        <a-select-option value="12个月">12个月</a-select-option>
      </a-select>
    </div>
    <div class="col-span-2">
      <div class="mb-1 text-xs text-gray-500">用途说明</div>
      <a-textarea v-model:value="form.reason" :rows="2" size="small" placeholder="接入办公网的用途" />
    </div>
    <div class="col-span-2">
      <div class="mb-1 text-xs text-gray-500">部门主管系统账号 *</div>
      <a-input
        v-model:value="form.supervisorUsername"
        size="small"
        placeholder="主管的用户名，审批将动态指派给该用户"
      />
    </div>
    <div class="col-span-2 text-right">
      <a-button
        v-access:code="['infra:flow-instance:start']"
        type="primary"
        :loading="submitting"
        @click="handleSubmit"
      >
        提交申报
      </a-button>
    </div>
  </div>
</template>
