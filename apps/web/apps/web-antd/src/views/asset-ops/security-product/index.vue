<script lang="ts" setup>
import { ref, onMounted, onActivated } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getSecurityProductList, createSecurityProduct, updateSecurityProduct, deleteSecurityProduct } from '#/api/scan/security-product';
import { getMachineRoomList } from '#/api/scan/machine-room';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';
interface Product { id?: number; name: string; category: string; vendor: string; model: string; version: string; license_type: string; license_expiry?: string; management_ip?: string; deployment_mode: string; machine_room_id?: number; serial_number?: string; status: string; contact_person: string; contact_phone: string; }
const data = ref<Product[]>([]); const loading = ref(false); const modalVisible = ref(false); const editingId = ref<number | undefined>();
const rooms = ref<any[]>([]);
const { filteredRows, searchText } = useLocalTableSearch(data, ['name', 'category', 'vendor', 'model', 'version', 'management_ip']);
const form = ref<Product>({ name: '', category: '', vendor: '', model: '', version: '', license_type: '', deployment_mode: 'standalone', status: 'active', contact_person: '', contact_phone: '' });
const columns = [{ title: '名称', dataIndex: 'name' }, { title: '类别', dataIndex: 'category' }, { title: '厂商', dataIndex: 'vendor' }, { title: '型号', dataIndex: 'model' }, { title: '版本', dataIndex: 'version' }, { title: '部署模式', dataIndex: 'deployment_mode' }, { title: '状态', dataIndex: 'status' }, { title: '操作', key: 'actions', width: 200 }];
async function fetchData() { loading.value = true; try { const [rows, roomRows] = await Promise.all([getSecurityProductList(), getMachineRoomList()]); data.value = rows as any; rooms.value = roomRows as any; } catch { message.error('加载失败'); } finally { loading.value = false; } }
onMounted(fetchData);
// keep-alive 标签页切回时刷新，机房增改后下拉不会过期
onActivated(fetchData);
async function openCreate() { await fetchData(); editingId.value = undefined; form.value = { name: '', category: '', vendor: '', model: '', version: '', license_type: '', deployment_mode: 'standalone', status: 'active', contact_person: '', contact_phone: '' }; modalVisible.value = true; }
async function openEdit(r: Product) { await fetchData(); editingId.value = r.id; form.value = { ...r }; modalVisible.value = true; }
async function handleSubmit() {
  const payload = { ...form.value };
  // Convert Dayjs object from date-picker to string for API
  if (payload.license_expiry && typeof payload.license_expiry !== 'string') {
    payload.license_expiry = (payload.license_expiry as any).format?.('YYYY-MM-DD') ?? String(payload.license_expiry);
  }
  try {
    if (editingId.value) { await updateSecurityProduct(editingId.value, payload as any); message.success('更新成功'); }
    else { await createSecurityProduct(payload as any); message.success('创建成功'); }
    modalVisible.value = false; fetchData();
  } catch { message.error('保存失败'); }
}
</script>
<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="安全设备" sub-title="维护防火墙、WAF、堡垒机、VPN、IDS/IPS 等安全基础设施" style="padding:0;margin-bottom:16px" />
      <a-space :size="24" style="margin-bottom:20px"><a-button v-access:code="['infra:security-product:create']" type="primary" @click="openCreate">新建安全设备</a-button><a-input-search v-model:value="searchText" allow-clear placeholder="搜索名称、类别、厂商或型号" style="width:260px" /><a-button @click="fetchData">刷新</a-button></a-space>
      <a-table :columns="columns" :data-source="filteredRows" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key === 'status'"><a-tag :color="record.status === 'active' ? 'green' : 'red'">{{ record.status }}</a-tag></template>
          <template v-if="column.key === 'actions'">
            <a-space :size="12"><a-button v-access:code="['infra:security-product:update']" size="small" @click="openEdit(record)">编辑</a-button><a-popconfirm title="确认删除?" @confirm="deleteSecurityProduct(record.id!).then(fetchData)"><a-button v-access:code="['infra:security-product:delete']" size="small" danger>删除</a-button></a-popconfirm></a-space>
          </template>
        </template>
      </a-table>
      <a-modal v-model:open="modalVisible" :title="editingId ? '编辑安全设备' : '新建安全设备'" @ok="handleSubmit" width="600px">
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12"><a-form-item label="名称" required><a-input v-model:value="form.name" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="类别" required><a-input v-model:value="form.category" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="厂商" required><a-input v-model:value="form.vendor" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="型号" required><a-input v-model:value="form.model" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="版本" required><a-input v-model:value="form.version" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="许可类型" required><a-input v-model:value="form.license_type" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="许可到期"><a-date-picker v-model:value="form.license_expiry" style="width:100%" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="管理IP"><a-input v-model:value="form.management_ip" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="部署方式" required><a-select v-model:value="form.deployment_mode"><a-select-option value="cloud_managed">云平台托管</a-select-option><a-select-option value="datacenter_managed">机房托管</a-select-option><a-select-option value="standalone">独立部署</a-select-option></a-select></a-form-item></a-col>
            <a-col v-if="form.deployment_mode !== 'standalone'" :span="12"><a-form-item label="物理机房" required><a-select v-model:value="form.machine_room_id" :options="rooms.map(room=>({value:room.id,label:room.room_name}))" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="序列号"><a-input v-model:value="form.serial_number" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="联系人" required><a-input v-model:value="form.contact_person" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="电话" required><a-input v-model:value="form.contact_phone" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
