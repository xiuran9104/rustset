<script lang="ts" setup>
import { computed, ref, onMounted, onActivated } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getCloudProviderConfigList, createCloudProviderConfig, updateCloudProviderConfig, deleteCloudProviderConfig, testCloudConnection } from '#/api/scan/cloud-provider-config';
import { getCloudPlatformList } from '#/api/scan/cloud-platform';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';

interface Config { id?: number; platform_id: number; provider: string; region_id: string; region_name: string; account_name: string; access_key_id: string; access_key_secret?: string; auth_type?:string; auth_username?:string; auth_password?:string; auth_domain?:string; project_id?:string; status: string; available_zones?: string[]; }

const data = ref<Config[]>([]);
const { filteredRows, searchText } = useLocalTableSearch(data, ['provider', 'region_id', 'region_name', 'account_name']);
const platforms = ref<any[]>([]);
const loading = ref(false);
const modalVisible = ref(false);
const editingId = ref<number>();
const form = ref<Config>({ platform_id:0,provider:'huawei',region_id:'',region_name:'',account_name:'',access_key_id:'',access_key_secret:'',auth_type:'access_key',status:'active' });

const providerMap:Record<string,{name:string;icon:string;color:string;bg:string}> = {
  huawei:{name:'华为云',icon:'lucide:cloud-cog',color:'#cf0a2c',bg:'#fff1f0'},
  aliyun:{name:'阿里云',icon:'lucide:cloud-sun',color:'#ff6a00',bg:'#fff7e6'},
  tencent:{name:'腾讯云',icon:'lucide:cloud-lightning',color:'#0052d9',bg:'#e6f4ff'},
  hcs:{name:'华为云 Stack (HCS)',icon:'lucide:cloud-cog',color:'#cf0a2c',bg:'#fff1f0'},
  apsara:{name:'阿里云专有云',icon:'lucide:cloud-sun',color:'#ff6a00',bg:'#fff7e6'},
  tstack:{name:'腾讯专有云 TCE',icon:'lucide:cloud-lightning',color:'#0052d9',bg:'#e6f4ff'},
  openstack:{name:'OpenStack',icon:'lucide:boxes',color:'#ed1944',bg:'#fff1f0'},
};
const authOptions=[{value:'access_key',label:'AK/SK（AccessKey）签名认证'},{value:'secret_key',label:'SecretId / SecretKey 签名认证'},{value:'iam_password',label:'IAM 账号获取 Token'},{value:'application_credential',label:'Application Credential'},{value:'keystone_password',label:'Keystone 账号获取 Token'}];
const usesAccountToken=computed(()=>['iam_password','keystone_password'].includes(form.value.auth_type||''));
const credentialLabels=computed(()=>form.value.auth_type==='secret_key'?{id:'SecretId',secret:'SecretKey'}:form.value.auth_type==='application_credential'?{id:'Application Credential ID',secret:'Application Credential Secret'}:{id:'Access Key ID',secret:'Access Key Secret'});
function handleProviderChange(){form.value.auth_type=form.value.provider==='tencent'?'secret_key':'access_key';form.value.access_key_id='';form.value.access_key_secret='';form.value.auth_username='';form.value.auth_password='';form.value.auth_domain='';form.value.project_id='';}
const columns = [
  { title:'所属云平台', key:'platform_id' },
  { title:'云厂商', key:'provider' },
  { title:'区域', dataIndex:'region_name' },
  { title:'端点地址', dataIndex:'region_id', ellipsis:true },
  { title:'账号', dataIndex:'account_name' },
  { title:'状态', key:'status', width:90 },
  { title:'操作', key:'actions', width:220 },
];

async function fetchData() { loading.value=true; try { const [configRows, platformRows] = await Promise.all([getCloudProviderConfigList(), getCloudPlatformList()]); data.value=configRows as any; platforms.value=platformRows as any; } catch { message.error('加载失败'); } finally { loading.value=false; } }
onMounted(fetchData);
// keep-alive 标签页切回时刷新，云平台增改后下拉不会过期
onActivated(fetchData);

async function openCreate() { await fetchData(); if (!platforms.value.length) { message.warning('请先创建云平台'); return; } editingId.value=undefined; form.value={ platform_id:platforms.value[0].id,provider:'huawei',region_id:'',region_name:'',account_name:'',access_key_id:'',access_key_secret:'',auth_type:'access_key',status:'active' }; modalVisible.value=true; }
async function openEdit(r:Config) { await fetchData(); editingId.value=r.id; form.value={...r}; modalVisible.value=true; }
async function handleSubmit() {
  if (!form.value.platform_id) { message.warning('请选择所属云平台'); return; }
  if (!form.value.region_id || !form.value.region_name || !form.value.account_name) { message.warning('请完整填写端点、区域和账号名称'); return; }
  if (usesAccountToken.value && (!form.value.auth_username || (!editingId.value && !form.value.auth_password) || !form.value.auth_domain || !form.value.project_id)) { message.warning('请完整填写 IAM/Keystone 认证信息'); return; }
  if (!usesAccountToken.value && (!form.value.access_key_id || (!editingId.value && !form.value.access_key_secret))) { message.warning(`请完整填写 ${credentialLabels.value.id} 和密钥`); return; }
  if (editingId.value) { await updateCloudProviderConfig(editingId.value,form.value as any); message.success('更新成功'); }
  else { await createCloudProviderConfig(form.value as any); message.success('创建成功'); }
  modalVisible.value=false; fetchData();
}
async function handleTest(r:Config) {
  const hide = message.loading({content:'测试连接中...',duration:0});
  try { const res:any = await testCloudConnection(r.id!); hide(); if(res?.success){message.success(res.message||'连接正常');}else{message.error(res?.message||'连接失败');} } catch { hide(); message.error('连接失败'); }
}
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="云厂商对接" sub-title="管理云平台的接口凭据并测试连接" style="margin-bottom:16px;padding:0" />
      <a-space :size="24" style="margin-bottom:20px">
        <a-button v-access:code="['infra:cloud-provider-config:create']" type="primary" @click="openCreate">新建云厂商对接</a-button>
        <a-input-search v-model:value="searchText" allow-clear placeholder="搜索厂商、区域或账号" style="width:240px" />
        <a-button @click="fetchData" :loading="loading">刷新</a-button>
      </a-space>

      <a-table :columns="columns" :data-source="filteredRows" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
        <template #bodyCell="{ column, record }">
          <template v-if="column.key==='platform_id'">{{ platforms.find(platform=>platform.id===record.platform_id)?.platform_name || '-' }}</template>
          <template v-else-if="column.key==='provider'"><a-tag :color="providerMap[record.provider]?.color">{{ providerMap[record.provider]?.name || record.provider }}</a-tag></template>
          <template v-else-if="column.key==='status'"><a-tag :color="record.status==='active'?'green':'default'">{{ record.status==='active'?'已启用':'已停用' }}</a-tag></template>
          <template v-else-if="column.key==='actions'">
            <a-space>
              <a-button v-access:code="['infra:cloud-provider-config:update']" size="small" @click="handleTest(record)">测试连接</a-button>
              <a-button v-access:code="['infra:cloud-provider-config:update']" size="small" @click="openEdit(record)">编辑</a-button>
              <a-popconfirm title="确认删除?" @confirm="deleteCloudProviderConfig(record.id!).then(fetchData)"><a-button v-access:code="['infra:cloud-provider-config:delete']" size="small" danger>删除</a-button></a-popconfirm>
            </a-space>
          </template>
        </template>
      </a-table>

      <a-modal v-model:open="modalVisible" :title="editingId?'编辑云厂商对接':'新建云厂商对接'" @ok="handleSubmit" width="560px">
        <a-form layout="vertical">
          <a-form-item label="所属云平台" required>
            <a-select v-model:value="form.platform_id" :options="platforms.map(platform=>({value:platform.id,label:platform.platform_name}))" placeholder="请选择云平台" />
          </a-form-item>
          <a-form-item label="云厂商" required>
            <a-select v-model:value="form.provider" @change="handleProviderChange">
              <a-select-option value="huawei">华为云</a-select-option>
              <a-select-option value="aliyun">阿里云</a-select-option>
              <a-select-option value="tencent">腾讯云</a-select-option>
            </a-select>
          </a-form-item>
          <a-form-item label="认证方式" required><a-select v-model:value="form.auth_type" :options="authOptions" /></a-form-item>
          <a-form-item label="API / IAM 端点地址" required><a-input v-model:value="form.region_id" placeholder="https://endpoint.example.com" /></a-form-item>
          <a-form-item :label="form.provider==='openstack'?'Project / Region':'区域名称'" required><a-input v-model:value="form.region_name" placeholder="region-1" /></a-form-item>
          <a-form-item :label="form.provider==='openstack'?'凭据名称/账号':'账号名称'" required><a-input v-model:value="form.account_name" /></a-form-item>
          <template v-if="!usesAccountToken"><a-form-item :label="credentialLabels.id" required><a-input v-model:value="form.access_key_id" /></a-form-item><a-form-item :label="credentialLabels.secret" required><a-input-password v-model:value="form.access_key_secret" /></a-form-item></template>
          <template v-else><a-form-item label="IAM 用户名" required><a-input v-model:value="form.auth_username" /></a-form-item><a-form-item label="IAM 密码" required><a-input-password v-model:value="form.auth_password" /></a-form-item><a-form-item label="Domain" required><a-input v-model:value="form.auth_domain" placeholder="Default" /></a-form-item><a-form-item label="Project ID / 名称" required><a-input v-model:value="form.project_id" /></a-form-item></template>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
