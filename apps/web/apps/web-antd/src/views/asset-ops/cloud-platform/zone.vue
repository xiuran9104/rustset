<script lang="ts" setup>
import { ref, onMounted, onActivated } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getCloudZoneList, createCloudZone, updateCloudZone, deleteCloudZone } from '#/api/scan/cloud-platform';
import { getServiceProviderList } from '#/api/scan/service-provider';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';
interface CloudZone { id?: number; zone_name: string; zone_code: string; provider_id?: number; description?: string; }
const data = ref<CloudZone[]>([]); const loading = ref(false); const modalVisible = ref(false); const editingId = ref<number | undefined>();
const providers = ref<any[]>([]);
const { filteredRows, searchText } = useLocalTableSearch(data, ['zone_name', 'zone_code', 'description']);
const form = ref<CloudZone>({ zone_name: '', zone_code: '', provider_id: undefined, description: '' });
const columns = [{ title: '资源区名称', dataIndex: 'zone_name' }, { title: '编码', dataIndex: 'zone_code' }, { title: '所属服务商', key: 'provider_id', width: 140 }, { title: '描述', dataIndex: 'description' }, { title: '操作', key: 'actions', width: 200 }];
async function fetchData() { loading.value = true; try { const [zoneRows, providerRows] = await Promise.all([getCloudZoneList(), getServiceProviderList()]); data.value = zoneRows as any; providers.value = providerRows as any; } catch { message.error('加载失败'); } finally { loading.value = false; } }
onMounted(fetchData);
// keep-alive 标签页切回时刷新，服务商在其他页面增改后下拉和列表不会过期
onActivated(fetchData);
async function openCreate() { await fetchData(); if (!providers.value.length) { message.warning('请先创建服务商'); return; } editingId.value = undefined; form.value = { zone_name: '', zone_code: '', provider_id: undefined, description: '' }; modalVisible.value = true; }
async function openEdit(r: CloudZone) { await fetchData(); editingId.value = r.id; form.value = { ...r }; modalVisible.value = true; }
async function handleSubmit() {
  if (!form.value.zone_name.trim()) { message.warning('请输入资源区名称'); return; }
  if (!form.value.provider_id) { message.warning('请选择所属服务商'); return; }
  if (editingId.value) { await updateCloudZone(editingId.value, form.value as any); message.success('更新成功'); } else { await createCloudZone(form.value as any); message.success('创建成功'); }
  modalVisible.value = false; fetchData();
}
</script>
<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-space :size="24" style="margin-bottom:20px"><a-button v-access:code="['infra:cloud-zone:create']" type="primary" @click="openCreate">新建云资源区</a-button><a-input-search v-model:value="searchText" allow-clear placeholder="搜索名称、编码或描述" style="width:260px" /><a-button @click="fetchData">刷新</a-button></a-space>
      <a-table :columns="columns" :data-source="filteredRows" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'provider_id'">{{ providers.find(p => p.id === record.provider_id)?.provider_name || '-' }}</template>
          <template v-else-if="column.key === 'actions'">
            <a-space :size="12"><a-button v-access:code="['infra:cloud-zone:update']" size="small" @click="openEdit(record)">编辑</a-button><a-popconfirm title="确认删除?" @confirm="deleteCloudZone(record.id!).then(fetchData)"><a-button v-access:code="['infra:cloud-zone:delete']" size="small" danger>删除</a-button></a-popconfirm></a-space>
          </template>
        </template>
      </a-table>
      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑云资源区' : '新建云资源区'" @ok="handleSubmit" width="500px">
        <a-form layout="vertical">
          <a-form-item label="资源区名称" required><a-input v-model:value="form.zone_name" placeholder="华东区域" /></a-form-item>
          <a-form-item label="编码" required><a-input v-model:value="form.zone_code" placeholder="east" /></a-form-item>
          <a-form-item label="所属服务商" required><a-select v-model:value="form.provider_id" :options="providers.map(p => ({ value: p.id, label: p.provider_name }))" placeholder="请选择服务商" /></a-form-item>
          <a-form-item label="描述"><a-textarea v-model:value="form.description" /></a-form-item>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
