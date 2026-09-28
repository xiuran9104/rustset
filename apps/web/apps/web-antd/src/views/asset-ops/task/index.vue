<script lang="ts" setup>
import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { onMounted, onUnmounted } from 'vue';
import { cancelTask, getTaskList, createTask, deleteTask, retryTask } from '#/api/scan/task';
import type { ScanTaskApi } from '#/api/scan/task';
import { useCrudList } from '../composables/useCrudList';

type Task = ScanTaskApi.ScanTask;
const { loading, modalVisible, searchText, form, filtered, fetchData, openCreate, handleSubmit, handleDelete } = useCrudList<Task>({
  api: { list: getTaskList, create: createTask, del: (id)=>deleteTask(String(id)) },
  defaultForm: () => ({ id:'',name:'',target:'',status:'pending',foundAssets:0,foundRisks:0,portPolicy:'COMMON',serviceDetection:true,domainBrute:false,osDetection:false,siteIdentify:false,attemptCount:0,maxAttempts:3,cancelRequested:false,completedTargets:0,createTime:'',nextAttemptAt:'',scanPorts:[],taskKind:'scan',timeoutSeconds:300,totalTargets:0,updateTime:'' }),
  searchKeys: ['name', 'target'],
});
const columns = [
  { title:'名称', dataIndex:'name', width:180 },{ title:'目标', dataIndex:'target', ellipsis:true },
  { title:'状态', dataIndex:'status', width:105 },{ title:'策略', dataIndex:'portPolicy', width:80 },
  { title:'尝试', key:'attempts', width:70 },
  { title:'资产', dataIndex:'foundAssets', width:60 },{ title:'风险', dataIndex:'foundRisks', width:60 },
  { title:'时间', dataIndex:'startTime', width:160 },{ title:'操作', key:'actions', width:150, fixed:'right' },
];
const stC:Record<string,string>={pending:'default',queued:'blue',retrying:'orange',running:'processing',completed:'green',failed:'red',cancelled:'default'};
const stL:Record<string,string>={pending:'待执行',queued:'排队中',retrying:'等待重试',running:'运行中',completed:'已完成',failed:'失败',cancelled:'已取消'};
async function handleCancel(id:string){await cancelTask(id);message.success('已请求取消任务');await fetchData();}
async function handleRetry(id:string){await retryTask(id);message.success('任务已重新排队');await fetchData();}
let refreshTimer:number|undefined;
onMounted(()=>{refreshTimer=window.setInterval(()=>{if(filtered.value.some((task)=>['queued','retrying','running'].includes(task.status)))fetchData();},3000);});
onUnmounted(()=>{if(refreshTimer)window.clearInterval(refreshTimer);});
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="扫描任务" sub-title="配置目标和扫描策略" style="margin-bottom:16px;padding:0" />
      <a-space :size="20" style="margin-bottom:20px" wrap><a-button v-access:code="['infra:task:create']" type="primary" @click="openCreate"><Icon icon="lucide:plus" /> 创建扫描任务</a-button><a-input-search v-model:value="searchText" placeholder="搜索任务或目标" style="width:260px" allow-clear/><a-button @click="fetchData"><Icon icon="lucide:refresh-cw" /> 刷新</a-button></a-space>
      <a-card style="border-radius:8px" size="small">
        <a-table :columns="columns" :data-source="filtered" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:15}">
          <template #bodyCell="{ column, record }">
            <template v-if="column.key==='status'"><a-tooltip :title="record.errorMessage"><a-badge :status="record.status==='running'?'processing':record.status==='completed'?'success':record.status==='failed'?'error':'default'" /><a-tag :color="stC[record.status]" style="margin-left:4px">{{ stL[record.status]||record.status }}</a-tag></a-tooltip></template>
            <template v-if="column.key==='attempts'">{{ record.attemptCount||0 }}/{{ record.maxAttempts||3 }}</template>
            <template v-if="column.key==='actions'"><a-space size="small"><a-popconfirm v-if="['queued','retrying','running'].includes(record.status)" title="确认取消任务?" @confirm="handleCancel(record.id)"><a-button v-access:code="['infra:task:execute']" type="link" size="small">取消</a-button></a-popconfirm><a-button v-if="['failed','cancelled'].includes(record.status)" v-access:code="['infra:task:execute']" type="link" size="small" @click="handleRetry(record.id)">重试</a-button><a-popconfirm title="确认删除?" @confirm="handleDelete(record.id)"><a-button v-access:code="['infra:task:delete']" type="link" size="small" danger>删除</a-button></a-popconfirm></a-space></template>
          </template>
        </a-table>
      </a-card>
      <a-modal v-model:open="modalVisible" title="新建扫描任务" @ok="handleSubmit" width="500px">
        <a-form v-if="form" layout="vertical">
          <a-form-item label="任务名称" required><a-input v-model:value="form.name" placeholder="例如：核心网段扫描" /></a-form-item>
          <a-form-item label="目标" required extra="每个任务最多 64 个明确 IP，使用逗号、分号或空格分隔"><a-textarea v-model:value="form.target" placeholder="192.168.1.10, 10.0.0.20" :rows="2" /></a-form-item>
          <a-row :gutter="16">
            <a-col :span="12"><a-form-item label="端口策略"><a-select v-model:value="form.portPolicy"><a-select-option value="COMMON">常用端口</a-select-option><a-select-option value="TOP1000">1-1000 + 常用高端口</a-select-option><a-select-option value="ALL">全端口</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="服务识别"><a-switch v-model:checked="form.serviceDetection" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
