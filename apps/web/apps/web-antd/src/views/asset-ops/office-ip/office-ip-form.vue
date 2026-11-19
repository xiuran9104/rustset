<script lang="ts" setup>
/**
 * 办公网 IP 申报表单（共享组件）：申请中心弹窗与独立「办公网IP申报」页
 * 复用同一表单，payload 契约见迁移 0066（办公网IP申报流程）。
 * 表单结构与资源工单创建弹窗同风格（a-form 垂直布局 + row/col 栅格）。
 */
import { computed, reactive, ref } from 'vue';

import { message } from 'ant-design-vue';
import { useUserStore } from '@vben/stores';

import {
  getFlowDefinitionPage,
  startFlowInstance,
} from '#/api/asset-ops/flow';

const emit = defineEmits(['submitted']);

const userStore = useUserStore();
const currentUserLabel = computed(
  () => userStore.userInfo?.nickname || userStore.userInfo?.username || '',
);

const OFFICE_IP_FLOW_NAME = '办公网IP申报流程';

const emptyForm = () => ({
  accessPeriod: '长期',
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
  const macAddress = form.macAddress.trim().toUpperCase();
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
      title: `${currentUserLabel.value} 办公网IP申报`,
      payload: {
        accessPeriod: form.accessPeriod,
        applicantName: currentUserLabel.value,
        department: form.department.trim(),
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
  <a-form layout="vertical">
    <a-row :gutter="16">
      <a-col :span="8">
        <a-form-item label="申报人（当前登录人）">
          <a-input :value="currentUserLabel" disabled />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="部门（系统自动带出）">
          <a-input v-model:value="form.department" placeholder="无部门归属时填写，提交后自动带出" />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="部门主管系统账号" required>
          <a-input
            v-model:value="form.supervisorUsername"
            placeholder="主管用户名，审批将动态指派"
          />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="设备 MAC 地址" required>
          <a-input v-model:value="form.macAddress" placeholder="AA:BB:CC:DD:EE:FF" />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="设备类型">
          <a-select v-model:value="form.deviceType">
            <a-select-option value="台式机">台式机</a-select-option>
            <a-select-option value="笔记本">笔记本</a-select-option>
            <a-select-option value="移动终端">移动终端</a-select-option>
            <a-select-option value="打印机">打印机</a-select-option>
            <a-select-option value="其他">其他</a-select-option>
          </a-select>
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="主机名">
          <a-input v-model:value="form.hostname" placeholder="如 zhangsan-laptop" />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="申请 IP（留空自动分配）">
          <a-input v-model:value="form.requestedIp" placeholder="192.168.1.100" />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="接入区域">
          <a-select v-model:value="form.networkZone">
            <a-select-option value="办公区">办公区</a-select-option>
            <a-select-option value="访客区">访客区</a-select-option>
            <a-select-option value="研发区">研发区</a-select-option>
          </a-select>
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="使用期限">
          <a-select v-model:value="form.accessPeriod">
            <a-select-option value="长期">长期</a-select-option>
            <a-select-option value="3个月">3个月</a-select-option>
            <a-select-option value="6个月">6个月</a-select-option>
            <a-select-option value="12个月">12个月</a-select-option>
          </a-select>
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="工号">
          <a-input v-model:value="form.employeeId" />
        </a-form-item>
      </a-col>
      <a-col :span="8">
        <a-form-item label="联系电话">
          <a-input v-model:value="form.phone" />
        </a-form-item>
      </a-col>
      <a-col :span="24">
        <a-form-item label="用途说明">
          <a-textarea v-model:value="form.reason" :rows="3" placeholder="接入办公网的用途" />
        </a-form-item>
      </a-col>
    </a-row>
    <div class="text-right">
      <a-button
        v-access:code="['infra:flow-instance:start']"
        type="primary"
        :loading="submitting"
        @click="handleSubmit"
      >
        提交申报
      </a-button>
    </div>
  </a-form>
</template>
