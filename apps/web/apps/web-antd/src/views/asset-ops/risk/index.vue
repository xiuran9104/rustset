<script lang="ts" setup>
import { computed, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getRiskList, updateRisk } from '#/api/scan/risk';
import { useCrudList } from '../composables/useCrudList';

interface Risk { id:string; asset_ip:string; port:number; severity:string; description:string; solution?:string; status:string; assigned_to?:string; source_type?:string; }
const { data, loading, fetchData } = useCrudList<Risk>({
  api: { list: getRiskList },
  defaultForm: () => ({
    id: '',
    asset_ip: '',
    port: 0,
    severity: 'Medium',
    description: '',
    status: 'open',
  }),
});

const detailVisible = ref(false);
const detailRisk = ref<Risk|null>(null);
const severityFilter = ref('');
const searchText = ref('');

const columns = [
  { title:'资产IP', dataIndex:'asset_ip', key:'asset_ip', width:140 },
  { title:'端口', dataIndex:'port', key:'port', width:70 },
  { title:'严重', dataIndex:'severity', key:'severity', width:80 },
  { title:'来源', dataIndex:'source_type', key:'source_type', width:95 },
  { title:'描述', dataIndex:'description', key:'description', ellipsis:true },
  { title:'状态', dataIndex:'status', key:'status', width:90 },
  { title:'负责人', dataIndex:'assigned_to', key:'assigned_to', width:90 },
  { title:'操作', key:'actions', width:150, fixed:'right' },
];

const sevColor:Record<string,string>={Critical:'#f5222d',High:'#fa8c16',Medium:'#faad14',Low:'#52c41a'};
const stColor:Record<string,string>={open:'red',verified:'orange',resolved:'green',ignored:'default',false_positive:'purple',pending_review:'blue'};
const stLabel:Record<string,string>={open:'未处理',verified:'已确认',resolved:'已解决',ignored:'已忽略',false_positive:'误报',pending_review:'待审核'};

const filtered = computed(() => {
  const keyword = searchText.value.trim().toLowerCase();
  return data.value.filter((risk) => {
    const matchesSeverity = !severityFilter.value || risk.severity === severityFilter.value;
    const matchesKeyword = !keyword || `${risk.asset_ip} ${risk.port} ${risk.description} ${risk.assigned_to || ''}`.toLowerCase().includes(keyword);
    return matchesSeverity && matchesKeyword;
  });
});

function showDetail(r:Risk) { detailRisk.value=r; detailVisible.value=true; }
async function handleStatus(r:Risk,status:string) { await updateRisk(r.id,{status}); message.success('状态已更新'); fetchData(); }
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="风险管理" sub-title="统一查看扫描与网络策略发现的风险并跟踪处置状态" style="margin-bottom:16px;padding:0" />

      <div class="risk-summary">
        <div class="summary-title"><Icon icon="lucide:shield-alert" /><span>风险概览</span><strong>{{ data.length }}</strong></div>
        <div class="summary-divider" />
        <div class="summary-item critical"><span>严重</span><strong>{{ data.filter(r=>r.severity==='Critical').length }}</strong></div>
        <div class="summary-item high"><span>高危</span><strong>{{ data.filter(r=>r.severity==='High').length }}</strong></div>
        <div class="summary-item medium"><span>中危</span><strong>{{ data.filter(r=>r.severity==='Medium').length }}</strong></div>
        <div class="summary-item low"><span>低危</span><strong>{{ data.filter(r=>r.severity==='Low').length }}</strong></div>
      </div>

      <a-card size="small">
        <div class="table-toolbar">
          <a-space :size="16" wrap>
            <a-input-search v-model:value="searchText" placeholder="搜索资产 IP、端口、风险或负责人" allow-clear style="width:300px" />
            <a-select v-model:value="severityFilter" placeholder="全部严重程度" allow-clear style="width:160px">
              <a-select-option value="Critical">严重</a-select-option>
              <a-select-option value="High">高危</a-select-option>
              <a-select-option value="Medium">中危</a-select-option>
              <a-select-option value="Low">低危</a-select-option>
            </a-select>
          </a-space>
          <a-button @click="fetchData"><Icon icon="lucide:refresh-cw" /> 刷新</a-button>
        </div>
        <a-table :columns="columns" :data-source="filtered" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:20}">
          <template #bodyCell="{ column, record }">
            <template v-if="column.key==='severity'"><a-tag :color="sevColor[record.severity]||'default'">{{ record.severity }}</a-tag></template>
            <template v-if="column.key==='source_type'"><a-tag :color="record.source_type==='network_policy'?'volcano':'blue'">{{ record.source_type==='network_policy'?'网络策略':'扫描核查' }}</a-tag></template>
            <template v-if="column.key==='status'"><a-tag :color="stColor[record.status]">{{ stLabel[record.status]||record.status }}</a-tag></template>
            <template v-if="column.key==='actions'">
              <a-space>
                <a-button type="link" size="small" @click="showDetail(record)">详情</a-button>
                <a-dropdown v-access:code="['infra:risk:update']">
                  <a-button type="link" size="small">处置 <Icon icon="lucide:chevron-down" /></a-button>
                  <template #overlay>
                    <a-menu @click="({key}:any)=>handleStatus(record,key)">
                      <a-menu-item key="verified">确认风险</a-menu-item>
                      <a-menu-item key="resolved">标记已解决</a-menu-item>
                      <a-menu-item key="ignored">忽略</a-menu-item>
                      <a-menu-item key="false_positive">误报</a-menu-item>
                    </a-menu>
                  </template>
                </a-dropdown>
              </a-space>
            </template>
          </template>
        </a-table>
      </a-card>

      <a-modal v-model:open="detailVisible" title="风险详情" width="560px" :footer="null">
        <template v-if="detailRisk">
          <a-descriptions bordered size="small" :column="1" style="margin-bottom:16px">
            <a-descriptions-item label="资产IP">{{ detailRisk.asset_ip }}</a-descriptions-item>
            <a-descriptions-item label="端口">{{ detailRisk.port }}</a-descriptions-item>
            <a-descriptions-item label="严重程度"><a-tag :color="sevColor[detailRisk.severity]">{{ detailRisk.severity }}</a-tag></a-descriptions-item>
            <a-descriptions-item label="来源">{{ detailRisk.source_type==='network_policy'?'网络策略高风险端口匹配':'扫描核查' }}</a-descriptions-item>
            <a-descriptions-item label="状态"><a-tag :color="stColor[detailRisk.status]">{{ stLabel[detailRisk.status] }}</a-tag></a-descriptions-item>
            <a-descriptions-item label="负责人">{{ detailRisk.assigned_to||'未分配' }}</a-descriptions-item>
          </a-descriptions>
          <a-card title="风险描述" size="small" style="margin-bottom:12px">{{ detailRisk.description }}</a-card>
          <a-card title="解决方案" size="small">{{ detailRisk.solution||'暂无解决方案，建议人工评估' }}</a-card>
        </template>
      </a-modal>
    </div>
  </Page>
</template>

<style scoped>
.risk-summary { display:flex; align-items:center; gap:28px; min-height:72px; margin-bottom:16px; padding:0 24px; background:var(--ant-color-bg-container); border:1px solid var(--ant-color-border-secondary); border-radius:8px; }
.summary-title,.summary-item { display:flex; align-items:center; gap:9px; white-space:nowrap; }
.summary-title { color:var(--ant-color-text-secondary); }
.summary-title :deep(svg) { color:var(--ant-color-primary); font-size:20px; }
.summary-title strong { color:var(--ant-color-text); font-size:24px; }
.summary-divider { align-self:stretch; width:1px; margin:16px 0; background:var(--ant-color-border-secondary); }
.summary-item span { color:var(--ant-color-text-secondary); }
.summary-item strong { font-size:20px; }
.summary-item.critical strong { color:#cf1322; }
.summary-item.high strong { color:#d46b08; }
.summary-item.medium strong { color:#d48806; }
.summary-item.low strong { color:#389e0d; }
.table-toolbar { display:flex; align-items:center; justify-content:space-between; gap:20px; margin-bottom:16px; }
@media (max-width: 768px) { .risk-summary { flex-wrap:wrap; gap:12px 20px; padding:16px; } .summary-divider { display:none; } .table-toolbar { align-items:flex-start; flex-direction:column; } }
</style>
