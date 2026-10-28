<script lang="ts" setup>
import { computed, onActivated, onMounted, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import {
  createMachineRoom,
  deleteMachineRoom,
  getMachineRoomList,
  updateMachineRoom,
} from '#/api/scan/machine-room';
import type { ScanMachineRoomApi } from '#/api/scan/machine-room';
import { getCloudZoneList } from '#/api/scan/cloud-platform';
import { getServiceProviderList } from '#/api/scan/service-provider';
import { useCrudList } from '../composables/useCrudList';

type MachineRoom = ScanMachineRoomApi.MachineRoom;
const zones = ref<any[]>([]);
const providers = ref<any[]>([]);
const treeVisible = ref(false);

const {
  loading, modalVisible, editingId, searchText, form,
  filtered, fetchData, openCreate: openCreateForm, openEdit: openEditForm, handleSubmit, handleDelete,
} = useCrudList<MachineRoom>({
  api: {
    list: getMachineRoomList,
    create: (d) => createMachineRoom(d as any),
    update: (id, d) => updateMachineRoom(id, d as any),
    del: (id) => deleteMachineRoom(id),
  },
  defaultForm: () => ({ room_name:'',room_code:'',facility_type:'',address:'',zone_id:undefined as number|undefined,room_type:'核心机房',contact_person:'',contact_phone:'',status:'active' }),
  searchKeys: ['room_name', 'room_code', 'address'],
});
async function loadRefs() {
  const [zoneRows, providerRows] = await Promise.all([getCloudZoneList(), getServiceProviderList()]);
  zones.value = zoneRows as any;
  providers.value = providerRows as any;
}
onMounted(loadRefs);
// keep-alive 标签页切回时刷新参照数据（表格由 useCrudList 的 onActivated 刷新）
onActivated(loadRefs);
function zoneLabel(zoneId?: number) {
  if (!zoneId) return '未划分区域';
  const zone = zones.value.find(item => item.id === zoneId);
  if (!zone) return '未划分区域';
  const provider = providers.value.find(item => item.id === zone.provider_id);
  return provider ? `${provider.provider_name} / ${zone.zone_name}` : zone.zone_name;
}
// 服务商 -> 云资源区 -> 机房 三级层级；未划分区域的机房挂在独立分组下
const roomTree = computed(() => {
  const groups = new Map<string, any>();
  for (const room of filtered.value) {
    const zone = zones.value.find((item) => item.id === room.zone_id);
    let parentNode: any;
    if (!zone) {
      parentNode = groups.get('unassigned');
      if (!parentNode) {
        parentNode = { key: 'unassigned', title: '未划分区域', children: [] };
        groups.set('unassigned', parentNode);
      }
    } else {
      const provider = providers.value.find((item) => item.id === zone.provider_id);
      const providerId = zone.provider_id || 0;
      const groupKey = `provider-${providerId}`;
      parentNode = groups.get(groupKey);
      if (!parentNode) {
        parentNode = { key: groupKey, title: provider?.provider_name || '未指定服务商', children: [] };
        groups.set(groupKey, parentNode);
      }
      const zoneKey = `${groupKey}-zone-${zone.id}`;
      let zoneNode = parentNode.children.find((item: any) => item.key === zoneKey);
      if (!zoneNode) {
        zoneNode = { key: zoneKey, title: zone.zone_name, children: [] };
        parentNode.children.push(zoneNode);
      }
      parentNode = zoneNode;
    }
    parentNode.children.push({ key: `room-${room.id}`, title: `${room.room_name}（${room.room_code}）`, isLeaf: true });
  }
  return [...groups.values()];
});
async function openCreate() {
  await loadRefs();
  if (!zones.value.length) {
    message.warning('请先为服务商创建云资源区');
    return;
  }
  openCreateForm();
  if (form.value) form.value.zone_id = zones.value[0].id;
}
async function openEdit(room: MachineRoom) {
  await loadRefs();
  openEditForm(room);
}

const columns = [
  { title:'机房名称', dataIndex:'room_name', key:'room_name', width:160 },
  { title:'编码', dataIndex:'room_code', key:'room_code', width:100 },
  { title:'类型', dataIndex:'room_type', key:'room_type', width:90 },
  { title:'设施', dataIndex:'facility_type', key:'facility_type', width:80 },
  { title:'所属云资源区', key:'zone_id', width:170 },
  { title:'地址', dataIndex:'address', key:'address', ellipsis:true },
  { title:'联系人', dataIndex:'contact_person', key:'contact_person', width:90 },
  { title:'机柜', key:'cabinet_count', width:70 },
  { title:'状态', dataIndex:'status', key:'status', width:65 },
  { title:'操作', key:'actions', width:130, fixed:'right' },
];

const statusMap:Record<string,{color:string;label:string}> = { active:{color:'green',label:'运营中'}, inactive:{color:'red',label:'停用'}, maintenance:{color:'orange',label:'维护中'} };
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="物理机房管理" sub-title="管理云资源区下的物理机房" style="margin-bottom:16px;padding:0" />
      <a-space :size="24" style="margin-bottom:20px">
        <a-button v-access:code="['infra:machine-room:create']" type="primary" @click="openCreate">新建物理机房</a-button>
        <a-button @click="fetchData">刷新</a-button>
        <a-input-search v-model:value="searchText" placeholder="搜索机房" style="width:240px" allow-clear />
      </a-space>
        <a-button style="margin-bottom: 12px" @click="treeVisible = true">层级视图</a-button>
        <a-table :columns="columns" :data-source="filtered" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
          <template #bodyCell="{ column, record }">
            <template v-if="column.key==='room_name'"><Icon icon="lucide:warehouse" style="color:var(--ant-color-primary);margin-right:6px" />{{ record.room_name }}</template>
            <template v-if="column.key==='status'"><a-tag :color="statusMap[record.status]?.color||'default'">{{ statusMap[record.status]?.label||record.status }}</a-tag></template>
            <template v-if="column.key==='cabinet_count'">{{ record.cabinet_count ?? '-' }}</template>
            <template v-if="column.key==='zone_id'">{{ zoneLabel(record.zone_id) }}</template>
            <template v-if="column.key==='actions'">
              <a-space>
                <a-button v-access:code="['infra:machine-room:update']" type="link" size="small" @click="openEdit(record)">编辑</a-button>
                <a-popconfirm title="确认删除该机房?" ok-text="删除" ok-type="danger" cancel-text="取消" @confirm="handleDelete(record.id!)"><a-button v-access:code="['infra:machine-room:delete']" type="link" size="small" danger>删除</a-button></a-popconfirm>
              </a-space>
            </template>
          </template>
        </a-table>
      <a-modal v-model:open="modalVisible" :title="editingId?'编辑机房':'新建机房'" @ok="handleSubmit" width="700px">
        <a-form v-if="form" layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12"><a-form-item label="机房名称" required><a-input v-model:value="form.room_name" placeholder="例如：北京核心机房" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="机房编码" required><a-input v-model:value="form.room_code" placeholder="例如：BJ-CORE" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="设施类型"><a-select v-model:value="form.facility_type"><a-select-option value="IDC">IDC 数据中心</a-select-option><a-select-option value="EDC">EDC 边缘机房</a-select-option><a-select-option value="Cloud">云机房</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="机房类型"><a-select v-model:value="form.room_type"><a-select-option value="核心机房">核心机房</a-select-option><a-select-option value="汇聚机房">汇聚机房</a-select-option><a-select-option value="接入机房">接入机房</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="状态"><a-select v-model:value="form.status"><a-select-option value="active">运营中</a-select-option><a-select-option value="inactive">停用</a-select-option><a-select-option value="maintenance">维护中</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="地址" required><a-input v-model:value="form.address" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="所属云资源区" required><a-select v-model:value="form.zone_id" :options="zones.map(z => ({ value: z.id, label: zoneLabel(z.id) }))" placeholder="请选择云资源区" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="联系人" required><a-input v-model:value="form.contact_person" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="联系电话" required><a-input v-model:value="form.contact_phone" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="楼层"><a-input v-model:value="form.floor" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="机柜数量"><a-input-number v-model:value="form.cabinet_count" :min="0" style="width:100%" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="面积"><a-input v-model:value="form.area_size" placeholder="m²" /></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="备注"><a-textarea v-model:value="form.remarks" :rows="2" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
      <a-drawer v-model:open="treeVisible" title="机房层级视图（服务商 / 云资源区 / 机房）" width="460px">
        <a-empty v-if="!roomTree.length" description="暂无机房" />
        <a-tree v-else :tree-data="roomTree" default-expand-all />
      </a-drawer>
    </div>
  </Page>
</template>
