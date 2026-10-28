<script lang="ts" setup>
import { computed, onActivated, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';

import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';

import { getCloudPlatformList, getCloudZoneList } from '#/api/scan/cloud-platform';
import { getUserPage } from '#/api/system/user';
import {
  createCloudResource,
  deleteCloudResource,
  getCloudResourceList,
  updateCloudResource,
} from '#/api/scan/cloud-resource';
import { getMachineRoomList } from '#/api/scan/machine-room';
import {
  createPhysicalResource,
  deletePhysicalResource,
  getPhysicalResourceList,
  updatePhysicalResource,
} from '#/api/scan/physical-resource';

import { useLocalTableSearch } from '../../composables/useLocalTableSearch';
import PhysicalNetworkFields from './PhysicalNetworkFields.vue';

const props = defineProps<{ fixedResourceType: 'cloud' | 'physical' }>();

const router = useRouter();

/** 闭环入口：从台账行直接发起针对该资源的工单（变更/停机/回收）。 */
function openTicket(record: Resource) {
  router.push({
    path: '/ops-center/resource-ticket',
    query: { resource: String(record.id), type: 'migration' },
  });
}

interface Resource {
  id?: number;
  resource_type?: string;
  ecs_name: string;
  ecs_status: string;
  cloud_region: string;
  cloud_category?: string;
  customer_name?: string;
  customer_user_id?: number;
  cpu_cores: number;
  memory_gb: number;
  ip_address: string;
  application_status: string;
  delivery_status: string;
  machine_room_id?: number;
  cloud_platform_id?: number;
  platform_name?: string;
  zone_name?: string;
  cabinet_no?: string;
  rack_start_u?: number;
  rack_end_u?: number;
  serial_number?: string;
  hardware_model?: string;
  ipmi_address?: string;
  warranty_expiry?: string;
  project_name?: string;
  contract_name?: string;
  supplier_name?: string;
  deployment_type?: string;
  management_ip?: string;
  business_ip?: string;
  network_cidr?: string;
  gateway?: string;
  vlan_id?: string;
  dns_servers?: string;
  mac_address?: string;
  ecs_type: string;
  ecs_os: string;
  system_disk: string;
  system_disk_size_gb: number;
  has_security_product: boolean;
}

const isPhysical = computed(() => props.fixedResourceType === 'physical');
const pageMeta = computed(() =>
  isPhysical.value
    ? {
        title: '物理资源',
        subtitle: '维护物理服务器台账、部署归属和网络配置',
        createLabel: '新建物理服务器',
        entity: '物理服务器',
      }
    : {
        title: '云资源',
        subtitle: '维护云服务器台账、规格、区域与交付状态',
        createLabel: '新建云服务器',
        entity: '云服务器',
      },
);

const data = ref<Resource[]>([]);
const loading = ref(false);
const modalVisible = ref(false);
const editingId = ref<number | undefined>();
const rooms = ref<any[]>([]);
const platforms = ref<any[]>([]);
const zones = ref<any[]>([]);
const users = ref<any[]>([]);
const { filteredRows, searchText } = useLocalTableSearch(data, [
  'ecs_name',
  'cloud_region',
  'customer_name',
  'ip_address',
]);

const emptyForm = (): Resource => ({
  resource_type: props.fixedResourceType,
  ecs_name: '',
  ecs_status: isPhysical.value ? 'available' : '运行中',
  cloud_region: '',
  customer_name: '',
  cpu_cores: 4,
  memory_gb: 8,
  ip_address: '',
  application_status: '待审核',
  delivery_status: '待交付',
  deployment_type: isPhysical.value ? 'standalone' : 'cloud_managed',
  ecs_type: '',
  ecs_os: '',
  system_disk: '',
  system_disk_size_gb: 0,
  has_security_product: false,
});
const form = ref<Resource>(emptyForm());

const columns = computed(() => {
  const base = [
    { title: '名称', dataIndex: 'ecs_name' },
    ...(isPhysical.value
      ? [
          { title: '部署方式', key: 'deployment_type' },
          { title: '上架位置', key: 'rack_location' },
        ]
      : []),
    { title: '状态', key: 'ecs_status' },
    { title: '客户', dataIndex: 'customer_name' },
    { title: '规格', key: 'spec' },
    { title: 'IP', key: 'ip' },
    { title: '操作', key: 'actions', width: 260 },
  ];
  return base;
});

const deploymentLabel: Record<string, string> = {
  cloud_managed: '云平台自有节点',
  datacenter_managed: '机柜托管',
  standalone: '本地自建',
};
const statusLabel: Record<string, string> = {
  available: '空闲',
  allocated: '已分配',
  maintenance: '维修中',
  offline: '已下线',
  retired: '已退役',
};

const api = computed(() =>
  isPhysical.value
    ? {
        list: getPhysicalResourceList,
        create: createPhysicalResource,
        update: updatePhysicalResource,
        remove: deletePhysicalResource,
      }
    : {
        list: getCloudResourceList,
        create: createCloudResource,
        update: updateCloudResource,
        remove: deleteCloudResource,
      },
);
const permissionCode = computed(() => (suffix: string) =>
  isPhysical.value
    ? `infra:physical-resource:${suffix}`
    : `infra:cloud-resource:${suffix}`,
);

async function fetchData() {
  loading.value = true;
  try {
    const [resources, roomRows, platformRows, zoneRows, userPage] = await Promise.all([
      api.value.list(),
      getMachineRoomList(),
      getCloudPlatformList(),
      getCloudZoneList(),
      getUserPage({ pageNo: 1, pageSize: 500 }),
    ]);
    data.value = resources as any;
    rooms.value = roomRows as any;
    platforms.value = platformRows as any;
    zones.value = zoneRows as any;
    users.value = ((userPage as any)?.list ?? []) as any;
  } catch {
    message.error('加载失败');
  } finally {
    loading.value = false;
  }
}
onMounted(fetchData);
// keep-alive 标签页切回时刷新，机房/云平台增改后下拉不会过期
onActivated(fetchData);

async function openCreate() {
  await fetchData();
  editingId.value = undefined;
  form.value = emptyForm();
  modalVisible.value = true;
}

async function openEdit(r: Resource) {
  await fetchData();
  editingId.value = r.id;
  // 以 emptyForm 兜底合并：列表行不含 resource_type 等默认字段，直接展开会让
  // 云资源/物理资源的专属表单区块在编辑弹窗里消失，与新建弹窗不一致
  form.value = { ...emptyForm(), ...r, has_security_product: !!r.has_security_product };
  modalVisible.value = true;
}

/** 云资源选定平台后，区域随平台所属资源区自动带出。 */
function changeCloudPlatform(platformId: number) {
  const platform = platforms.value.find((item) => item.id === platformId);
  const zone = zones.value.find((item) => item.id === platform?.zone_id);
  form.value.cloud_platform_id = platformId;
  form.value.platform_name = platform?.platform_name || '';
  form.value.zone_name = zone?.zone_name || '';
  form.value.cloud_region = zone?.zone_name || platform?.platform_name || '';
}

/** 客户从系统用户中选定，名称随选择自动带出。 */
function changeCustomer(userId: number) {
  const u = users.value.find((item) => item.id === userId);
  form.value.customer_user_id = userId;
  form.value.customer_name = u?.nickname || u?.username || '';
}

async function handleSubmit() {
  if (
    form.value.resource_type === 'cloud' &&
    !form.value.cloud_platform_id
  ) {
    message.warning('请选择归属云平台');
    return;
  }
  if (
    form.value.resource_type === 'physical' &&
    form.value.deployment_type === 'cloud_managed' &&
    !form.value.cloud_platform_id
  ) {
    message.warning('云平台自有节点必须选择归属云平台');
    return;
  }
  if (
    form.value.resource_type === 'physical' &&
    form.value.deployment_type !== 'standalone' &&
    !form.value.machine_room_id
  ) {
    message.warning('请选择物理机房');
    return;
  }
  if (
    form.value.resource_type === 'physical' &&
    form.value.deployment_type !== 'standalone' &&
    (!form.value.cabinet_no?.trim() ||
      !form.value.rack_start_u ||
      !form.value.rack_end_u ||
      form.value.rack_end_u < form.value.rack_start_u)
  ) {
    message.warning('请填写机柜编号和有效的起止 U 位');
    return;
  }
  if (
    form.value.resource_type === 'physical' &&
    form.value.deployment_type !== 'cloud_managed' &&
    (!(form.value.management_ip || form.value.business_ip) ||
      !form.value.network_cidr ||
      !form.value.gateway)
  ) {
    message.warning('请填写管理/业务 IP、网段和网关');
    return;
  }
  if (editingId.value) {
    await api.value.update(editingId.value, form.value as any);
    message.success('更新成功');
  } else {
    await api.value.create(form.value as any);
    message.success('创建成功');
  }
  modalVisible.value = false;
  fetchData();
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding: 16px">
      <a-page-header
        :title="pageMeta.title"
        :sub-title="pageMeta.subtitle"
        style="padding: 0; margin-bottom: 16px"
      />
      <a-space :size="24" style="margin-bottom: 20px">
        <a-button
          v-access:code="[permissionCode('create')]"
          type="primary"
          @click="openCreate"
        >
          {{ pageMeta.createLabel }}
        </a-button>
        <a-input-search
          v-model:value="searchText"
          allow-clear
          placeholder="搜索名称、区域、客户或 IP"
          style="width: 260px"
        />
        <a-button @click="fetchData">刷新</a-button>
      </a-space>
      <a-table
        :columns="columns"
        :data-source="filteredRows"
        :loading="loading"
        row-key="id"
        size="middle"
        :pagination="{ pageSize: 20 }"
      >
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'deployment_type'">
            {{ deploymentLabel[record.deployment_type] || '-' }}
          </template>
          <template v-if="column.key === 'rack_location'">
            {{
              rooms.find((room) => room.id === record.machine_room_id)
                ?.room_name ||
              (record.deployment_type === 'standalone' ? '本地自建' : '-')
            }}<span v-if="record.cabinet_no"> / {{ record.cabinet_no }}</span>
            <span v-if="record.rack_start_u">
              / U{{ record.rack_start_u }}-U{{ record.rack_end_u }}</span
            >
          </template>
          <template v-if="column.key === 'ecs_status'">
            <a-tag
              :color="
                ['运行中', 'available', 'allocated'].includes(record.ecs_status)
                  ? 'green'
                  : record.ecs_status === 'maintenance'
                    ? 'orange'
                    : 'default'
              "
            >
              {{ statusLabel[record.ecs_status] || record.ecs_status }}
            </a-tag>
          </template>
          <template v-if="column.key === 'spec'">
            {{ record.cpu_cores }}核 / {{ record.memory_gb }}GB
          </template>
          <template v-if="column.key === 'ip'">
            {{
              isPhysical
                ? record.management_ip || record.business_ip || '-'
                : record.ip_address || '-'
            }}
          </template>
          <template v-if="column.key === 'application_status'">
            <a-tag
              :color="
                record.application_status === '已批准'
                  ? 'green'
                  : record.application_status === '待审核'
                    ? 'orange'
                    : 'default'
              "
            >
              {{ record.application_status }}
            </a-tag>
          </template>
          <template v-if="column.key === 'actions'">
            <a-space :size="12">
              <a-button
                v-access:code="['infra:resource-ticket:create']"
                size="small"
                @click="openTicket(record)"
              >
                工单
              </a-button>
              <a-button
                v-access:code="[permissionCode('update')]"
                size="small"
                @click="openEdit(record)"
              >
                编辑
              </a-button>
              <a-popconfirm
                title="确认删除?"
                @confirm="api.remove(record.id!).then(fetchData)"
              >
                <a-button
                  v-access:code="[permissionCode('delete')]"
                  size="small"
                  danger
                >
                  删除
                </a-button>
              </a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>
      <a-modal
        v-model:open="modalVisible"
        :title="(editingId ? '编辑' : '新建') + pageMeta.entity"
        width="760px"
        @ok="handleSubmit"
      >
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12">
              <a-form-item label="名称" required>
                <a-input v-model:value="form.ecs_name" />
              </a-form-item>
            </a-col>
            <PhysicalNetworkFields
              v-if="form.resource_type === 'physical'"
              v-model:form="form"
              :rooms="rooms"
              :platforms="platforms"
            />
            <a-col :span="8">
              <a-form-item label="状态" required>
                <a-select
                  v-if="form.resource_type === 'physical'"
                  v-model:value="form.ecs_status"
                >
                  <a-select-option value="available">空闲</a-select-option>
                  <a-select-option value="allocated">已分配</a-select-option>
                  <a-select-option value="maintenance">维修中</a-select-option>
                  <a-select-option value="offline">已下线</a-select-option>
                  <a-select-option value="retired">已退役</a-select-option>
                </a-select>
                <a-select v-else v-model:value="form.ecs_status">
                  <a-select-option value="运行中">运行中</a-select-option>
                  <a-select-option value="已停止">已停止</a-select-option>
                  <a-select-option value="异常">异常</a-select-option>
                </a-select>
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="CPU核数">
                <a-input-number
                  v-model:value="form.cpu_cores"
                  :min="1"
                  style="width: 100%"
                />
              </a-form-item>
            </a-col>
            <a-col :span="8">
              <a-form-item label="内存(GB)">
                <a-input-number
                  v-model:value="form.memory_gb"
                  :min="1"
                  style="width: 100%"
                />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'cloud'" :span="12">
              <a-form-item label="归属云平台" required>
                <a-select
                  v-model:value="form.cloud_platform_id"
                  :options="platforms.map(p => ({ value: p.id, label: p.platform_name }))"
                  placeholder="请选择云平台"
                  @update:value="changeCloudPlatform"
                />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'cloud'" :span="12">
              <a-form-item label="区域（随平台自动带出）">
                <a-input v-model:value="form.cloud_region" disabled placeholder="选择云平台后自动带出其资源区" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="区域" required>
                <a-input v-model:value="form.cloud_region" />
              </a-form-item>
            </a-col>
            <a-col :span="12">
              <a-form-item label="客户" required>
                <a-select
                  v-model:value="form.customer_user_id"
                  :options="users.map(u => ({ value: u.id, label: u.nickname || u.username }))"
                  placeholder="请选择客户用户"
                  show-search
                  option-filter-prop="label"
                  @update:value="changeCustomer"
                />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="SN 序列号">
                <a-input v-model:value="form.serial_number" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="硬件型号">
                <a-input v-model:value="form.hardware_model" placeholder="如 Dell R740" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="带外 IP（BMC/IPMI）">
                <a-input v-model:value="form.ipmi_address" placeholder="如 10.10.1.100" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="保修到期">
                <a-input v-model:value="form.warranty_expiry" placeholder="如 2027-12-31" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="采购立项项目">
                <a-input v-model:value="form.project_name" placeholder="如 2026年政务云扩容项目" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="合同编号">
                <a-input v-model:value="form.contract_name" placeholder="如 HT-2026-001" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'physical'" :span="12">
              <a-form-item label="供应商（可选）">
                <a-input v-model:value="form.supplier_name" placeholder="如 联想/戴尔/集成商名称" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'cloud'" :span="12">
              <a-form-item label="IP地址" required>
                <a-input v-model:value="form.ip_address" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'cloud'" :span="12">
              <a-form-item label="合同编号（云框架合同）">
                <a-input v-model:value="form.contract_name" placeholder="如 HT-2026-政务云框架" />
              </a-form-item>
            </a-col>
            <a-col v-if="form.resource_type === 'cloud'" :span="12">
              <a-form-item label="所属业务项目">
                <a-input v-model:value="form.project_name" placeholder="如 惠企通平台 / 不动产系统" />
              </a-form-item>
            </a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
