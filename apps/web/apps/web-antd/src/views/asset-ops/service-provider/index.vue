<script lang="ts" setup>
import { ref, onMounted, onActivated } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getServiceProviderList, createServiceProvider, updateServiceProvider, deleteServiceProvider } from '#/api/scan/service-provider';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';
interface Provider { id?: number; provider_name: string; provider_code: string; short_name: string; contact_person: string; contact_phone: string; contact_email: string; headquarters: string; service_area: string; business_license: string; status: string; }
const data = ref<Provider[]>([]); const loading = ref(false); const modalVisible = ref(false); const editingId = ref<number | undefined>();
const { filteredRows, searchText } = useLocalTableSearch(data, ['provider_name','provider_code','short_name','headquarters','service_area']);
const form = ref<Provider>({ provider_name: '', provider_code: '', short_name: '', contact_person: '', contact_phone: '', contact_email: '', headquarters: '', service_area: '', business_license: '', status: 'active' });
const columns = [{ title: '服务商名称', dataIndex: 'provider_name' }, { title: '编码', dataIndex: 'provider_code' }, { title: '简称', dataIndex: 'short_name' }, { title: '总部', dataIndex: 'headquarters' }, { title: '服务区域', dataIndex: 'service_area' }, { title: '状态', dataIndex: 'status' }, { title: '操作', key: 'actions', width: 200 }];
async function fetchData() { loading.value = true; try { data.value = await getServiceProviderList() as any; } catch { message.error('加载失败'); } finally { loading.value = false; } }
onMounted(fetchData);
// keep-alive 标签页切回时刷新列表
onActivated(fetchData);
async function openCreate() { await fetchData(); editingId.value = undefined; form.value = { provider_name: '', provider_code: '', short_name: '', contact_person: '', contact_phone: '', contact_email: '', headquarters: '', service_area: '', business_license: '', status: 'active' }; modalVisible.value = true; }
async function openEdit(r: Provider) { await fetchData(); editingId.value = r.id; form.value = { ...r }; modalVisible.value = true; }
async function handleSubmit() { if (editingId.value) { await updateServiceProvider(editingId.value, form.value as any); message.success('更新成功'); } else { await createServiceProvider(form.value as any); message.success('创建成功'); } modalVisible.value = false; fetchData(); }
</script>
<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-space :size="24" style="margin-bottom:20px"><a-button v-access:code="['infra:service-provider:create']" type="primary" @click="openCreate">新建服务商</a-button><a-input-search v-model:value="searchText" placeholder="搜索服务商" allow-clear style="width:260px" /><a-button @click="fetchData">刷新</a-button></a-space>
      <a-table :columns="columns" :data-source="filteredRows" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'status'"><a-tag :color="record.status === 'active' ? 'green' : 'red'">{{ record.status }}</a-tag></template>
          <template v-if="column.key === 'actions'">
            <a-space><a-button v-access:code="['infra:service-provider:update']" size="small" @click="openEdit(record)">编辑</a-button><a-popconfirm title="确认删除?" @confirm="deleteServiceProvider(record.id!).then(fetchData)"><a-button v-access:code="['infra:service-provider:delete']" size="small" danger>删除</a-button></a-popconfirm></a-space>
          </template>
        </template>
      </a-table>
      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑服务商' : '新建服务商'" @ok="handleSubmit" width="600px">
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="8"><a-form-item label="名称" required><a-input v-model:value="form.provider_name" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="编码" required><a-input v-model:value="form.provider_code" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="简称" required><a-input v-model:value="form.short_name" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="联系人" required><a-input v-model:value="form.contact_person" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="电话" required><a-input v-model:value="form.contact_phone" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="邮箱" required><a-input v-model:value="form.contact_email" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="总部" required><a-input v-model:value="form.headquarters" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="服务区域" required><a-input v-model:value="form.service_area" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="营业执照" required><a-input v-model:value="form.business_license" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
