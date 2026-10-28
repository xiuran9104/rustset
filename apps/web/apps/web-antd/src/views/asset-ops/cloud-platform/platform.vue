<script lang="ts" setup>
import { ref, onMounted, onActivated } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getCloudPlatformList, getCloudZoneList, createCloudPlatform, updateCloudPlatform, deleteCloudPlatform } from '#/api/scan/cloud-platform';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';
interface Platform { id?: number; zone_id: number; platform_name: string; platform_code: string; description?: string; }
interface Option { id: number; label: string; }
const data = ref<Platform[]>([]); const loading = ref(false); const modalVisible = ref(false); const editingId = ref<number | undefined>();
const { filteredRows, searchText } = useLocalTableSearch(data, ['platform_name', 'platform_code', 'description']);
const zones = ref<Option[]>([]);
const rawZones = ref<any[]>([]);
const form = ref<Platform>({ zone_id: 0, platform_name: '', platform_code: '', description: '' });
const columns = [{ title: '平台名称', dataIndex: 'platform_name' }, { title: '平台编码', dataIndex: 'platform_code' }, { title: '所属云资源区', key: 'zone' }, { title: '描述', dataIndex: 'description' }, { title: '操作', key: 'actions', width: 180 }];
async function fetchData() { loading.value = true; try { const [platformRows, zoneRows] = await Promise.all([getCloudPlatformList(), getCloudZoneList()]); data.value=platformRows as any; rawZones.value=zoneRows as any; zones.value=(zoneRows as any[]).map(z=>({id:z.id,label:z.zone_name})); } catch { message.error('加载失败'); } finally { loading.value = false; } }
function getZone(zoneId: number) { return rawZones.value.find(zone => zone.id === zoneId); }
onMounted(fetchData);
// keep-alive 标签页切回时刷新，云资源区在其他页面增改后下拉不会过期
onActivated(fetchData);
async function openCreate() {
  // 页面是 keep-alive 缓存的，资源区可能在别的标签页刚创建，先重新拉取再校验
  await fetchData();
  if (zones.value.length === 0) {
    message.warning('请先为服务商创建云资源区');
    return;
  }
  editingId.value = undefined;
  form.value = { zone_id: zones.value[0]!.id, platform_name: '', platform_code: '', description: '' };
  modalVisible.value = true;
}
async function openEdit(r: Platform) { await fetchData(); editingId.value = r.id; form.value = { ...r }; modalVisible.value = true; }
async function handleSubmit() { if (!form.value.zone_id) { message.warning('请选择云资源区'); return; } if (editingId.value) { await updateCloudPlatform(editingId.value, form.value as any); message.success('更新成功'); } else { await createCloudPlatform(form.value as any); message.success('创建成功'); } modalVisible.value = false; fetchData(); }
</script>
<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="云平台管理" sub-title="管理各云资源区下的云平台" style="margin-bottom:16px;padding:0" />
      <a-space :size="24" style="margin-bottom:20px">
        <a-button v-access:code="['infra:cloud-platform:create']" type="primary" @click="openCreate">新建云平台</a-button>
        <a-input-search v-model:value="searchText" allow-clear placeholder="搜索平台名称、编码或描述" style="width:240px" />
        <a-button @click="fetchData">刷新</a-button>
      </a-space>
      <a-table :columns="columns" :data-source="filteredRows" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'zone'">{{ getZone(record.zone_id)?.zone_name || '-' }}</template>
          <template v-else-if="column.key === 'actions'">
            <a-space>
              <a-button v-access:code="['infra:cloud-platform:update']" size="small" @click="openEdit(record)">编辑</a-button>
              <a-popconfirm title="确认删除该云平台?" @confirm="deleteCloudPlatform(record.id).then(fetchData)"><a-button v-access:code="['infra:cloud-platform:delete']" size="small" danger>删除</a-button></a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>
      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑云平台' : '新建云平台'" @ok="handleSubmit" width="500px">
        <a-form layout="vertical">
          <a-form-item label="平台名称" required><a-input v-model:value="form.platform_name" placeholder="政务云生产平台" /></a-form-item>
          <a-form-item label="编码" required><a-input v-model:value="form.platform_code" placeholder="gov-production" /></a-form-item>
          <a-form-item label="所属云资源区" required><a-select v-model:value="form.zone_id" :options="zones.map(v=>({value:v.id,label:v.label}))" placeholder="请选择服务商下的云资源区" /></a-form-item>
          <a-form-item label="描述"><a-textarea v-model:value="form.description" /></a-form-item>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
