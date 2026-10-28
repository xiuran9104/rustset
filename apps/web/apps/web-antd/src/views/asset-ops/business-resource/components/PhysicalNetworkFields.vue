<script lang="ts" setup>
import { computed } from 'vue';
interface RoomOption { id: number; room_name: string }
interface PlatformOption { id: number; platform_name: string }
interface PhysicalForm {
  deployment_type?: string; machine_room_id?: number; cloud_platform_id?: number;
  cabinet_no?: string; rack_start_u?: number; rack_end_u?: number;
  management_ip?: string; business_ip?: string; network_cidr?: string; gateway?: string;
  vlan_id?: string; dns_servers?: string; mac_address?: string;
}

const form = defineModel<PhysicalForm>('form', { required: true });
const props = defineProps<{ rooms: RoomOption[]; platforms: PlatformOption[] }>();

const isCloudOwned = computed(() => form.value.deployment_type === 'cloud_managed');
const isStandalone = computed(() => form.value.deployment_type === 'standalone');
// 云自有/机柜托管必须登记机柜和 U 位；本地自建可选（办公室环境可能没有标准机架）
const positionRequired = computed(() => !isStandalone.value);

function changeDeployment(value: string) {
  form.value.deployment_type = value;
  if (value !== 'cloud_managed') {
    form.value.cloud_platform_id = undefined;
  }
  if (value === 'standalone') {
    form.value.machine_room_id = undefined;
    form.value.cabinet_no = undefined;
    form.value.rack_start_u = undefined;
    form.value.rack_end_u = undefined;
  }
  if (value === 'cloud_managed') {
    // 云自有节点的网络由平台侧维护，不在此重复录入
    for (const key of ['management_ip', 'business_ip', 'network_cidr', 'gateway', 'vlan_id', 'dns_servers', 'mac_address'] as const) {
      form.value[key] = '';
    }
  }
}
</script>

<template>
  <a-col :span="12">
    <a-form-item label="部署方式" required>
      <a-select :value="form.deployment_type" @update:value="changeDeployment">
        <a-select-option value="cloud_managed">云平台自有节点</a-select-option>
        <a-select-option value="datacenter_managed">机柜托管（产权自有）</a-select-option>
        <a-select-option value="standalone">本地自建</a-select-option>
      </a-select>
    </a-form-item>
  </a-col>
  <a-col v-if="isCloudOwned" :span="12">
    <a-form-item label="归属云平台" required>
      <a-select
        v-model:value="form.cloud_platform_id"
        placeholder="请选择云平台"
        :options="props.platforms.map(p => ({ value: p.id, label: p.platform_name }))"
      />
    </a-form-item>
  </a-col>
  <a-col v-if="!isStandalone" :span="12">
    <a-form-item label="部署机房" required>
      <a-select
        v-model:value="form.machine_room_id"
        placeholder="请选择物理机房"
        :options="props.rooms.map(room => ({ value: room.id, label: room.room_name }))"
      />
    </a-form-item>
  </a-col>
  <a-col v-else :span="12">
    <a-form-item label="所在机房（可选）">
      <a-select
        v-model:value="form.machine_room_id"
        placeholder="办公室/本地机房可先在机房台账登记"
        allow-clear
        :options="props.rooms.map(room => ({ value: room.id, label: room.room_name }))"
      />
    </a-form-item>
  </a-col>
  <a-col :span="8">
    <a-form-item :label="positionRequired ? '机柜编号' : '机柜编号（可选）'" :required="positionRequired">
      <a-input v-model:value="form.cabinet_no" placeholder="如 A-01" />
    </a-form-item>
  </a-col>
  <a-col :span="8">
    <a-form-item label="起始 U 位" :required="positionRequired">
      <a-input-number v-model:value="form.rack_start_u" :min="1" :max="60" style="width:100%" />
    </a-form-item>
  </a-col>
  <a-col :span="8">
    <a-form-item label="结束 U 位" :required="positionRequired">
      <a-input-number v-model:value="form.rack_end_u" :min="1" :max="60" style="width:100%" />
    </a-form-item>
  </a-col>
  <a-col :span="24">
    <a-alert
      v-if="isCloudOwned"
      type="info"
      show-icon
      message="云平台自有节点：网络信息由平台侧维护，不在此重复录入；机房所属资源区可在机房台账层级视图中查看。"
      style="margin-bottom:16px"
    />
    <a-alert
      v-else-if="form.deployment_type === 'datacenter_managed'"
      type="warning"
      show-icon
      message="机柜托管：机房/机柜为云运营商设施，服务器产权自有；网络由资产侧维护，请填写管理/业务 IP、网段和网关。"
      style="margin-bottom:16px"
    />
    <a-alert
      v-else
      type="warning"
      show-icon
      message="本地自建：产权、供电、网络全部自理，请填写管理/业务 IP、网段和网关。"
      style="margin-bottom:16px"
    />
  </a-col>
  <template v-if="!isCloudOwned">
    <a-col :span="12"><a-form-item label="管理 IP"><a-input v-model:value="form.management_ip" placeholder="如 10.10.1.20" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="业务 IP"><a-input v-model:value="form.business_ip" placeholder="如 172.16.1.20" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="网段" required><a-input v-model:value="form.network_cidr" placeholder="如 10.10.1.0/24" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="网关" required><a-input v-model:value="form.gateway" placeholder="如 10.10.1.1" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="VLAN"><a-input v-model:value="form.vlan_id" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="DNS"><a-input v-model:value="form.dns_servers" placeholder="多个地址用逗号分隔" /></a-form-item></a-col>
    <a-col :span="12"><a-form-item label="MAC 地址"><a-input v-model:value="form.mac_address" /></a-form-item></a-col>
  </template>
</template>
