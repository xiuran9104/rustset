<script lang="ts" setup>
import { computed, onMounted, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import {
  createMachineRoom,
  deleteMachineRoom,
  getMachineRoomList,
  updateMachineRoom,
} from '#/api/scan/machine-room';
import type { ScanMachineRoomApi } from '#/api/scan/machine-room';
import { getServiceProviderList } from '#/api/scan/service-provider';
import { useCrudList } from '../composables/useCrudList';

type MachineRoom = ScanMachineRoomApi.MachineRoom;
const providers = ref<any[]>([]);
const treeVisible = ref(false);

const {
  loading, modalVisible, editingId, searchText, form,
  filtered, fetchData, openCreate: openCreateForm, openEdit, handleSubmit, handleDelete,
} = useCrudList<MachineRoom>({
  api: {
    list: getMachineRoomList,
    create: (d) => createMachineRoom(d as any),
    update: (id, d) => updateMachineRoom(id, d as any),
    del: (id) => deleteMachineRoom(id),
  },
  defaultForm: () => ({ room_name:'',room_code:'',facility_type:'',address:'',provider_id:0,room_type:'核心机房',contact_person:'',contact_phone:'',status:'active' }),
  searchKeys: ['room_name', 'room_code', 'address'],
});
onMounted(async () => { providers.value = await getServiceProviderList() as any; });
const roomTree = computed(() => {
  const groups = new Map<number, any>();
  for (const room of filtered.value) {
    const provider = providers.value.find((item) => item.id === room.provider_id);
    const providerId = room.provider_id || 0;
    let providerNode = groups.get(providerId);
    if (!providerNode) {
      providerNode = { key: `provider-${providerId}`, title: provider?.provider_name || '未分配服务商', children: [] };
      groups.set(providerId, providerNode);
    }
    const typeKey = `${providerId}-${room.room_type || '未分类'}`;
    let typeNode = providerNode.children.find((item: any) => item.key === `type-${typeKey}`);
    if (!typeNode) {
      typeNode = { key: `type-${typeKey}`, title: room.room_type || '未分类', children: [] };
      providerNode.children.push(typeNode);
    }
    typeNode.children.push({ key: `room-${room.id}`, title: `${room.room_name}（${room.room_code}）`, isLeaf: true });
  }
  return [...groups.values()];
});
function openCreate() {
  if (!providers.value.length) {
    message.warning('请先创建服务商');
    return;
  }
  openCreateForm();
  if (form.value) form.value.provider_id = providers.value[0].id;
}

const columns = [
  { title:'机房名称', dataIndex:'room_name', key:'room_name', width:160 },
  { title:'编码', dataIndex:'room_code', key:'room_code', width:100 },
  { title:'类型', dataIndex:'room_type', key:'room_type', width:90 },
  { title:'设施', dataIndex:'facility_type', key:'facility_type', width:80 },
  { title:'服务商', key:'provider_id', width:120 },
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
      <a-page-header title="物理机房管理" sub-title="管理云平台下的物理机房" style="margin-bottom:16px;padding:0" />
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
            <template v-if="column.key==='provider_id'">{{ providers.find(p=>p.id===record.provider_id)?.provider_name || '-' }}</template>
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
            <a-col :span="8"><a-form-item label="联系人" required><a-input v-model:value="form.contact_person" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="联系电话" required><a-input v-model:value="form.contact_phone" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="所属服务商" required><a-select v-model:value="form.provider_id" :options="providers.map(p=>({value:p.id,label:p.provider_name}))" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="楼层"><a-input v-model:value="form.floor" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="机柜数量"><a-input-number v-model:value="form.cabinet_count" :min="0" style="width:100%" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="面积"><a-input v-model:value="form.area_size" placeholder="m²" /></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="备注"><a-textarea v-model:value="form.remarks" :rows="2" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
      <a-drawer v-model:open="treeVisible" title="机房层级视图" width="420px">
        <a-empty v-if="!roomTree.length" description="暂无机房" />
        <a-tree v-else :tree-data="roomTree" default-expand-all />
      </a-drawer>
    </div>
  </Page>
</template>
