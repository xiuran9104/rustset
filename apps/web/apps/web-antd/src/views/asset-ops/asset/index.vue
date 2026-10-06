<script lang="ts" setup>
import { ref, onMounted, computed } from 'vue';
import { useRouter } from 'vue-router';
import { Page } from '@vben/common-ui';
import { downloadFileFromBlobPart } from '@vben/utils';
import { message } from 'ant-design-vue';
import { requestClient } from '#/api/request';
import {
  createAsset,
  downloadAssetImportTemplate,
  exportAssetCsv,
  getAssetList,
  importAssetCsv,
  updateAsset,
} from '#/api/scan/asset';
import type { ScanAssetApi } from '#/api/scan/asset';

type Asset = ScanAssetApi.Asset;
type UnifiedAsset = Omit<Asset, 'id'> & { deployment_type?:string;editable:boolean;id:string;record_id:number;source_label:string;source_type:'cloud_platform'|'physical_inventory'|'scan';status:string; };
const data = ref<UnifiedAsset[]>([]); const loading = ref(false); const modalVisible = ref(false); const searchText = ref(''); const editingId = ref<number>();
const form = ref<Asset>({ name:'',ip:'',zone:'Intranet',ports:[],weight:50,labels:[] } as Asset);

// 资产采集表（docs/资产采集表.xlsx·资产排查表）43 个台账字段的默认值。
function inventoryDefaults(): Partial<Asset> {
  return {
    city: '', district: '', organization_name: '', business_department: '', department_contact: '',
    application_name: '', server_name: '', hardware_configuration: '', operating_system: '', database_type: '',
    launch_date: '', decommission_date: '', application_type: '', network_environment: '',
    internet_ipv4: '', internet_ipv6: '', domain_address: '', internal_network_ip: '', government_extranet_ip: '',
    open_ports: '', publishing_endpoint: '', publishes_other_endpoint: false, other_endpoint_name: '',
    security_product_installation: '',
    development_vendor: '', development_vendor_contact: '',
    security_vendor: '', security_vendor_contact: '',
    operations_vendor: '', operations_vendor_contact: '',
    classified_protection_level: '', classified_protection_assessed: false, classified_protection_assessor: '',
    classified_protection_assessment_date: '', classified_protection_score: undefined,
    classified_protection_filed: false, classified_protection_filing_date: '', classified_protection_filing_number: '',
    classified_protection_filing_authority: '',
    cryptography_assessed: false, cryptography_assessment_level: '',
    cryptography_assessment_date: '', cryptography_assessment_number: '',
  };
}

const filtered = computed(() => {
  const kw = searchText.value.toLowerCase();
  return kw ? data.value.filter(a=>a.name.toLowerCase().includes(kw)||a.ip.includes(kw)||(a.owner||'').toLowerCase().includes(kw)||(a.organization_name||'').toLowerCase().includes(kw)||(a.application_name||'').toLowerCase().includes(kw)) : data.value;
});

const zoneInfo:Record<string,{bg:string;color:string;label:string}> = { Internet:{color:'var(--ant-color-error)',bg:'#fff1f0',label:'外网'}, DMZ:{color:'var(--ant-color-warning)',bg:'#fff7e6',label:'DMZ'}, Intranet:{color:'var(--ant-color-success)',bg:'#f6ffed',label:'内网'} };

async function fetchData() { loading.value=true; try { data.value=(await getAssetList()) as any; } catch { message.error('加载失败'); } finally { loading.value=false; } }
onMounted(fetchData);
function openCreate() { editingId.value=undefined; form.value={ name:'',ip:'',zone:'Intranet',ports:[],weight:50,labels:[],...inventoryDefaults() } as Asset; modalVisible.value=true; }
function openEdit(r:UnifiedAsset) { if (!r.editable) return; editingId.value=r.record_id; const { id:_id, record_id:_rid, source_type:_st, source_label:_sl, editable:_ed, status:_s, deployment_type:_d, ...rest }=r; form.value={ ...inventoryDefaults(), ...rest, operating_system: rest.operating_system || rest.os || '' } as Asset; modalVisible.value=true; }
async function handleSubmit() {
  if (editingId.value) { await updateAsset(editingId.value,form.value as any); message.success('已更新'); } else { await createAsset(form.value as any); message.success('已创建'); }
  modalVisible.value=false; fetchData();
}

async function handleExportCsv() {
  const data = await exportAssetCsv();
  downloadFileFromBlobPart({ fileName: 'assets.csv', source: data });
}
async function handleTemplate() {
  const data = await downloadAssetImportTemplate();
  downloadFileFromBlobPart({ fileName: 'assets-import-template.csv', source: data });
}
async function handleImportCsv(file: File) {
  const result: any = await importAssetCsv(file);
  message.success(`导入完成：成功 ${result.created || 0} 条，失败 ${result.failed || 0} 条`);
  await fetchData();
  return false;
}

const viewMode = ref<'grid'|'table'>('grid');

// 需求 D：查看该资产 IP 命中的网络策略（防火墙开通记录），支持跳转到网络策略页。
const policyDrawerVisible = ref(false);
const policyTarget = ref<null | UnifiedAsset>(null);
const policyRows = ref<any[]>([]);
const policyLoading = ref(false);
const router = useRouter();
async function openPolicies(record: UnifiedAsset) {
  policyTarget.value = record;
  policyDrawerVisible.value = true;
  policyLoading.value = true;
  policyRows.value = [];
  try {
    policyRows.value = (await requestClient.get('/cmdb/net-zone/policies-by-ip', {
      params: { ip: record.ip },
    })) as any;
  } catch {
    message.error('查询网络策略失败');
  } finally {
    policyLoading.value = false;
  }
}
const statCards = [
  { title:'资产总数', value:()=>data.value.length, color:'var(--ant-color-primary)', bg:'#e6f4ff', icon:'lucide:hard-drive' },
  { title:'云平台资源', value:()=>data.value.filter(a=>a.source_type==='cloud_platform').length, color:'#1677ff', bg:'#e6f4ff', icon:'lucide:cloud' },
  { title:'物理基础设施', value:()=>data.value.filter(a=>a.source_type==='physical_inventory').length, color:'#722ed1', bg:'#f9f0ff', icon:'lucide:server' },
  { title:'扫描发现资产', value:()=>data.value.filter(a=>a.source_type==='scan').length, color:'var(--ant-color-success)', bg:'#f6ffed', icon:'lucide:scan-search' },
];
</script>

<template>
  <Page auto-content-height>
    <div style="padding:24px;background:var(--ant-color-bg-layout);min-height:100%">
      <a-page-header title="资产管理" sub-title="统一汇总云平台资源、物理基础设施和扫描发现资产" style="background:var(--ant-color-bg-container);border-radius:8px;margin-bottom:16px;padding:16px 24px">
        <template #extra>
          <a-space :size="16">
            <a-input-search v-model:value="searchText" placeholder="搜索资产名称/IP/负责人..." style="width:260px" allow-clear />
            <a-segmented v-model:value="viewMode" :options="[{value:'grid',label:'卡片'},{value:'table',label:'列表'}]" />
            <a-button @click="fetchData"><Icon icon="lucide:refresh-cw" /> 刷新</a-button>
            <a-button v-access:code="['infra:asset:query']" @click="handleExportCsv"><Icon icon="lucide:download" /> 导出 CSV</a-button>
            <a-button v-access:code="['infra:asset:create']" @click="handleTemplate">下载模板</a-button>
            <a-upload :show-upload-list="false" accept=".csv,text/csv" :before-upload="handleImportCsv">
              <a-button v-access:code="['infra:asset:create']"><Icon icon="lucide:upload" /> 导入 CSV</a-button>
            </a-upload>
            <a-button v-access:code="['infra:asset:create']" type="primary" @click="openCreate"><Icon icon="lucide:plus" /> 新建资产</a-button>
          </a-space>
        </template>
      </a-page-header>

      <a-row :gutter="[16,16]">
        <a-col :span="6" v-for="s in statCards" :key="s.title">
          <a-card size="small" style="border-radius:10px">
            <div style="display:flex;align-items:center;gap:12px">
              <div :style="{width:40,height:40,borderRadius:10,background:s.bg,display:'flex',alignItems:'center',justifyContent:'center'}">
                <Icon :icon="s.icon" :style="{fontSize:'20px',color:s.color}" />
              </div>
              <div>
                <div style="font-size:12px;color:var(--ant-color-text-tertiary)">{{ s.title }}</div>
                <div style="font-size:22px;font-weight:700;color:var(--ant-color-text)">{{ typeof s.value === 'function' ? s.value() : s.value }}</div>
              </div>
            </div>
          </a-card>
        </a-col>
      </a-row>

      <!-- Grid View -->
      <a-row :gutter="[16,16]" style="margin-top:16px" v-if="viewMode === 'grid'">
        <a-col :xs="24" :sm="12" :lg="8" :xl="6" v-for="item in filtered" :key="item.id">
          <a-card :hoverable="true" size="small" style="border-radius:10px;overflow:hidden">
            <template #title>
              <div style="display:flex;align-items:center;gap:6px">
                <Icon icon="lucide:server" :style="{color:zoneInfo[item.zone]?.color || '#666'}" />
                <span style="font-weight:600">{{ item.name }}</span>
                <a-tag :color="zoneInfo[item.zone]?.color" size="small" style="margin-left:auto">{{ zoneInfo[item.zone]?.label || item.zone }}</a-tag>
              </div>
            </template>
            <div style="margin-bottom:8px"><a-tag color="blue">{{ item.source_label }}</a-tag><a-tag>{{ item.device_type }}</a-tag><a-tag v-if="item.classified_protection_level" color="geekblue">等保{{ item.classified_protection_level }}</a-tag></div>
            <div v-if="item.organization_name" style="font-size:12px;color:#666;margin-bottom:2px"><Icon icon="lucide:building-2" style="margin-right:4px;color:#999" />{{ item.organization_name }}<span v-if="item.application_name"> · {{ item.application_name }}</span></div>
            <div style="font-size:13px;color:#666;margin-bottom:4px"><Icon icon="lucide:wifi" style="margin-right:4px;color:#999" />{{ item.ip }}</div>
            <div style="font-size:12px;color:#999;margin-bottom:8px">
              <span v-if="item.os"><Icon icon="lucide:monitor" style="margin-right:4px" />{{ item.os }}</span>
              <span v-if="item.owner" style="margin-left:12px"><Icon icon="lucide:user" style="margin-right:4px" />{{ item.owner }}</span>
            </div>
            <div style="margin-bottom:8px">
              <a-tag v-for="p in (item.ports || []).slice(0,8)" :key="p.port" size="small" :color="p.is_open ? 'blue' : 'default'" style="margin:2px">
                {{ p.port }}<span v-if="p.service" style="opacity:0.7">:{{ p.service }}</span>
              </a-tag>
              <a-tag v-if="(item.ports || []).length > 8" size="small" style="margin:2px">+{{ item.ports.length - 8 }}</a-tag>
            </div>
            <a-row :gutter="8">
              <a-col :span="5"><a-button v-access:code="['infra:asset:query']" type="link" size="small" @click="openPolicies(item)">策略</a-button></a-col><a-col :span="11"><a-progress :percent="item.weight" :size="20" :show-info="false" :stroke-color="item.weight > 70 ? 'var(--ant-color-error)' : item.weight > 40 ? 'var(--ant-color-warning)' : 'var(--ant-color-success)'" /></a-col>
              <a-col :span="8"><a-space size="0"><a-button v-if="item.editable" v-access:code="['infra:asset:update']" type="link" size="small" @click="openEdit(item)">编辑</a-button><a-tag v-else>来源只读</a-tag></a-space></a-col>
            </a-row>
          </a-card>
        </a-col>
      </a-row>
      <a-empty v-if="viewMode === 'grid' && !loading && !filtered.length" description="暂无资产数据" style="margin-top:60px"><a-button v-access:code="['infra:asset:create']" type="primary" @click="openCreate">创建第一个资产</a-button></a-empty>

      <!-- Table View -->
      <a-card style="border-radius:10px;margin-top:16px" size="small" v-if="viewMode === 'table'">
        <a-table :columns="[{title:'名称',dataIndex:'name',width:150,fixed:'left'},{title:'所属单位',dataIndex:'organization_name',width:130,ellipsis:true},{title:'应用名称',dataIndex:'application_name',width:130,ellipsis:true},{title:'资产类型',dataIndex:'device_type',width:95},{title:'来源',dataIndex:'source_label',width:140},{title:'IP/URL',dataIndex:'ip',width:140},{title:'区域',key:'zone',width:85},{title:'等保',key:'mlps',width:70},{title:'语言/系统',key:'technology',width:130},{title:'端口指纹',key:'ports',width:85},{title:'漏洞',key:'findings',width:65},{title:'风险评分',key:'weight',width:105},{title:'操作',key:'actions',width:190}]" :data-source="filtered" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:15,showTotal:(t:number)=>`共 ${t} 个`}" :scroll="{x:1600}">
          <template #bodyCell="{ column, record }">
            <template v-if="column.key === 'zone'"><a-tag :color="zoneInfo[record.zone]?.color">{{ zoneInfo[record.zone]?.label }}</a-tag></template>
            <template v-if="column.key === 'mlps'"><a-tag v-if="record.classified_protection_level" color="geekblue">{{ record.classified_protection_level }}</a-tag><span v-else>-</span></template>
            <template v-if="column.key === 'technology'">{{ record.language || record.os || '-' }}</template>
            <template v-if="column.key === 'ports'">{{ (record.ports || []).length }}</template>
            <template v-if="column.key === 'findings'">{{ record.finding_count || 0 }}</template>
            <template v-if="column.key === 'weight'"><a-progress :percent="record.weight" :size="20" :show-info="false" :stroke-color="record.weight > 70 ? 'var(--ant-color-error)' : record.weight > 40 ? 'var(--ant-color-warning)' : 'var(--ant-color-success)'" style="width:60px;display:inline-block" /><span style="font-size:11px;margin-left:4px">{{ record.weight }}</span></template>
            <template v-if="column.key === 'actions'"><a-space><a-button v-access:code="['infra:asset:query']" type="link" size="small" @click="openPolicies(record)">策略</a-button><a-button v-if="record.editable" v-access:code="['infra:asset:update']" type="link" size="small" @click="openEdit(record)">编辑</a-button><a-tag v-else>来源只读</a-tag></a-space></template>
          </template>
        </a-table>
      </a-card>

      <!-- Create/Edit Drawer: 台账字段按采集表分组 -->
      <a-drawer v-model:open="modalVisible" :title="editingId ? '编辑资产台账' : '新建资产'" width="780" @close="modalVisible = false">
        <a-form v-if="form" layout="vertical">
          <a-tabs>
            <a-tab-pane key="basic" tab="基础与归属">
              <a-row :gutter="16">
                <a-col :span="12"><a-form-item label="资产名称" required><a-input v-model:value="form.name" placeholder="例如：web-prod-01" /></a-form-item></a-col>
                <a-col :span="6"><a-form-item label="网络区域" required><a-select v-model:value="form.zone"><a-select-option value="Internet">Internet 外网</a-select-option><a-select-option value="DMZ">DMZ 隔离区</a-select-option><a-select-option value="Intranet">Intranet 内网</a-select-option></a-select></a-form-item></a-col>
                <a-col :span="6"><a-form-item label="设备类型"><a-select v-model:value="form.device_type" show-search><a-select-option value="VM">虚拟机</a-select-option><a-select-option value="BareMetal">物理机</a-select-option><a-select-option value="Container">容器</a-select-option><a-select-option value="Network">网络设备</a-select-option></a-select></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="所属地市"><a-input v-model:value="form.city" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="所属区划"><a-input v-model:value="form.district" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="单位"><a-input v-model:value="form.organization_name" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="业务部门"><a-input v-model:value="form.business_department" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="部门对接人"><a-input v-model:value="form.department_contact" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="负责人"><a-input v-model:value="form.owner" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="联系人"><a-input v-model:value="form.contact_person" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="标签"><a-select v-model:value="form.labels" mode="tags" placeholder="输入标签后回车" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="资产权重"><a-slider v-model:value="form.weight" :min="1" :max="100" :marks="{1:'低',50:'中',100:'高'}" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
            <a-tab-pane key="app" tab="应用与硬件">
              <a-row :gutter="16">
                <a-col :span="12"><a-form-item label="应用名称"><a-input v-model:value="form.application_name" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="服务器名称"><a-input v-model:value="form.server_name" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="硬件配置（CPU/内存/磁盘）"><a-input v-model:value="form.hardware_configuration" placeholder="例如：8C / 32G / 1T" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="操作系统"><a-input v-model:value="form.operating_system" placeholder="Ubuntu 22.04 / Windows Server 2019" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="数据库类型"><a-input v-model:value="form.database_type" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="应用类型"><a-input v-model:value="form.application_type" /></a-form-item></a-col>
                <a-col :span="4"><a-form-item label="上线时间"><a-date-picker v-model:value="form.launch_date" value-format="YYYY-MM-DD" style="width:100%" /></a-form-item></a-col>
                <a-col :span="4"><a-form-item label="停用时间"><a-date-picker v-model:value="form.decommission_date" value-format="YYYY-MM-DD" style="width:100%" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
            <a-tab-pane key="network" tab="网络信息">
              <a-row :gutter="16">
                <a-col :span="8"><a-form-item label="IP 地址" required><a-input v-model:value="form.ip" placeholder="192.168.1.100" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="网络环境"><a-input v-model:value="form.network_environment" placeholder="例如：政务外网" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="开放端口"><a-input v-model:value="form.open_ports" placeholder="例如：80,443,8080-8090" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="互联网IPv4"><a-input v-model:value="form.internet_ipv4" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="互联网IPv6"><a-input v-model:value="form.internet_ipv6" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="域名地址"><a-input v-model:value="form.domain_address" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="内大网IP"><a-input v-model:value="form.internal_network_ip" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="政务外网IP"><a-input v-model:value="form.government_extranet_ip" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="安全产品安装"><a-input v-model:value="form.security_product_installation" placeholder="例如：奇安信天擎、绿盟IPS" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="发布端"><a-input v-model:value="form.publishing_endpoint" placeholder="例如：政务APP、小程序" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="是否发布其他端"><a-switch v-model:checked="form.publishes_other_endpoint" /></a-form-item></a-col>
                <a-col :span="8" v-if="form.publishes_other_endpoint"><a-form-item label="其他端名称"><a-input v-model:value="form.other_endpoint_name" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
            <a-tab-pane key="vendor" tab="厂商信息">
              <a-row :gutter="16">
                <a-col :span="12"><a-form-item label="开发厂商"><a-input v-model:value="form.development_vendor" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="开发厂商联系人"><a-input v-model:value="form.development_vendor_contact" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="安全厂商"><a-input v-model:value="form.security_vendor" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="安全厂商联系人"><a-input v-model:value="form.security_vendor_contact" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="运维厂商"><a-input v-model:value="form.operations_vendor" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="运维厂商联系人"><a-input v-model:value="form.operations_vendor_contact" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
            <a-tab-pane key="mlps" tab="等保信息">
              <a-row :gutter="16">
                <a-col :span="8"><a-form-item label="等保级别"><a-select v-model:value="form.classified_protection_level" allow-clear><a-select-option value="一级">一级</a-select-option><a-select-option value="二级">二级</a-select-option><a-select-option value="三级">三级</a-select-option><a-select-option value="四级">四级</a-select-option></a-select></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="是否等保测评"><a-switch v-model:checked="form.classified_protection_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保测评机构"><a-input v-model:value="form.classified_protection_assessor" :disabled="!form.classified_protection_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保测评时间"><a-date-picker v-model:value="form.classified_protection_assessment_date" value-format="YYYY-MM-DD" style="width:100%" :disabled="!form.classified_protection_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保测评得分"><a-input-number v-model:value="form.classified_protection_score" :min="0" :max="100" style="width:100%" :disabled="!form.classified_protection_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="是否等保备案"><a-switch v-model:checked="form.classified_protection_filed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保备案时间"><a-date-picker v-model:value="form.classified_protection_filing_date" value-format="YYYY-MM-DD" style="width:100%" :disabled="!form.classified_protection_filed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保备案编号"><a-input v-model:value="form.classified_protection_filing_number" :disabled="!form.classified_protection_filed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="等保备案机关"><a-input v-model:value="form.classified_protection_filing_authority" :disabled="!form.classified_protection_filed" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
            <a-tab-pane key="crypto" tab="密码测评">
              <a-row :gutter="16">
                <a-col :span="8"><a-form-item label="是否密码测评"><a-switch v-model:checked="form.cryptography_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="密码测评级别"><a-input v-model:value="form.cryptography_assessment_level" :disabled="!form.cryptography_assessed" /></a-form-item></a-col>
                <a-col :span="8"><a-form-item label="密码测评时间"><a-date-picker v-model:value="form.cryptography_assessment_date" value-format="YYYY-MM-DD" style="width:100%" :disabled="!form.cryptography_assessed" /></a-form-item></a-col>
                <a-col :span="12"><a-form-item label="密码测评编号"><a-input v-model:value="form.cryptography_assessment_number" :disabled="!form.cryptography_assessed" /></a-form-item></a-col>
              </a-row>
            </a-tab-pane>
          </a-tabs>
        </a-form>
        <template #footer>
          <a-space>
            <a-button @click="modalVisible = false">取消</a-button>
            <a-button type="primary" @click="handleSubmit">保存</a-button>
          </a-space>
        </template>
      </a-drawer>

      <!-- 网络策略查看抽屉（需求 D） -->
      <a-drawer v-model:open="policyDrawerVisible" :title="`网络策略 · ${policyTarget?.name ?? ''}（${policyTarget?.ip ?? ''}）`" width="760">
        <a-spin :spinning="policyLoading">
          <a-alert
            v-if="!policyLoading && !policyRows.length"
            type="info"
            show-icon
            message="没有查到该 IP 相关的网络策略记录"
          />
          <a-table
            v-else
            :columns="[
              { title: '防火墙', dataIndex: 'firewallName', width: 110 },
              { title: '方向', dataIndex: 'relation', key: 'relation', width: 80 },
              { title: '源IP', dataIndex: 'sourceIp', width: 130 },
              { title: '目的IP', dataIndex: 'destinationIp', width: 130 },
              { title: '端口', dataIndex: 'servicePort', width: 90 },
              { title: '动作', key: 'action', width: 70 },
              { title: '申请日期', dataIndex: 'applicationDate', width: 105 },
            ]"
            :data-source="policyRows"
            row-key="id"
            size="small"
            :pagination="{ pageSize: 10 }"
          >
            <template #bodyCell="{ column, record }">
              <template v-if="column.key === 'relation'">
                <a-tag :color="record.relation === 'source' ? 'cyan' : 'purple'">{{ record.relation === 'source' ? '源' : '目的' }}</a-tag>
              </template>
              <template v-if="column.key === 'action'">
                <a-tag :color="record.action === 'allow' ? 'green' : 'red'">{{ record.action === 'allow' ? '允许' : '拒绝' }}</a-tag>
              </template>
            </template>
          </a-table>
          <a-button style="margin-top: 12px" @click="router.push('/asset-center/network-policy')">
            前往网络策略台账管理
          </a-button>
        </a-spin>
      </a-drawer>
    </div>
  </Page>
</template>
