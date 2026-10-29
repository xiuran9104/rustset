<script lang="ts" setup>
import { computed, onActivated, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';

import { Page } from '@vben/common-ui';
import { message } from 'ant-design-vue';
import { getCloudPlatformList, getCloudZoneList } from '#/api/scan/cloud-platform';
import { getMachineRoomList } from '#/api/scan/machine-room';
import { approveTicket, createResourceTicket, deleteResourceTicket, deliverTicket, getResourceTicketList, getTicketAttachments, getTicketApprovalHistory, provisionTicket } from '#/api/scan/resource-ticket';
import { uploadFile } from '#/api/infra/file';
import { requestClient } from '#/api/request';
import { getCloudResourceList } from '#/api/scan/cloud-resource';
import { getPhysicalResourceList } from '#/api/scan/physical-resource';
import { getSecurityProductList } from '#/api/scan/security-product';
import { getUserPage } from '#/api/system/user';
import { getCloudProviderConfigList } from '#/api/scan/cloud-provider-config';
import { useLocalTableSearch } from '../composables/useLocalTableSearch';

interface Ticket { id?:number; ticket_type:string; risk_level:string; target_resource_id?:number; target_config?:string; maintenance_window?:string; allow_interruption?:boolean; backup_confirmed?:boolean; rollback_plan?:string; retention_until?:string; expire_at?:string; expiry_remind_stage?:number; approval_stage:number; approval_total:number; current_approval_role?:string; ecs_name:string; resource_type:string; ticket_status:string; customer_name?:string; cloud_region?:string; cpu_cores:number; memory_gb:number; created_at:string; approver?:string; approve_comment?:string; provisioner?:string; provision_details?:string; deliverer?:string; deliver_comment?:string; ecs_type?:string; ecs_os?:string; ip_address?:string; applicant_name?:string; cloud_platform_id?:number; machine_room_id?:number; resource_count?:number; system_disk?:string; system_disk_size_gb?:number; data_disk?:string; security_products?:string; remarks?:string; }

const data=ref<Ticket[]>([]); const loading=ref(false); const activeTab=ref('all');
const { filteredRows: searchedTickets, searchText } = useLocalTableSearch(data, ['ecs_name', 'applicant_name', 'customer_name', 'cloud_region', 'ticket_status']);
const detailVisible=ref(false); const detailTicket=ref<Ticket|null>(null); const createVisible=ref(false);
const approvalHistory=ref<any[]>([]);
const provisionVisible=ref(false); const provisionTicketRecord=ref<Ticket|null>(null); const provisioning=ref(false);
const emptyProvisionForm=()=>({config_id:undefined as number|undefined,image_id:'',flavor:'',availability_zone:'',subnet_id:'',vpc_id:'',security_groups:[] as string[],details:''});
const provisionForm=ref(emptyProvisionForm());
const zones=ref<any[]>([]); const platforms=ref<any[]>([]); const rooms=ref<any[]>([]); const securityProducts=ref<any[]>([]);
const providerConfigs=ref<any[]>([]); const businessResources=ref<any[]>([]); const users=ref<any[]>([]);
const targetResourceType=ref<'cloud'|'physical'>('cloud');
const defaultForm=()=>({idempotency_key:crypto.randomUUID(),ticket_type:'create',target_resource_id:undefined as number|undefined,target_config:'',maintenance_window:'',allow_interruption:false,backup_confirmed:false,rollback_plan:'',retention_until:'',expire_at:'',ecs_name:'',resource_type:'cloud',applicant_user_id:undefined as number|undefined,applicant_name:'',provider_id:undefined as number|undefined,cloud_platform_id:undefined as number|undefined,cloud_platform_name:'',machine_room_id:undefined as number|undefined,cloud_region:'',cloud_category:'',zone_name:'',zone_cabinet:'',rack_units:0,customer_name:'',customer_user_id:undefined as number|undefined,project_name:'',application_name:'',contract_name:'',ecs_type:'通用型',ecs_os:'Linux',resource_count:1,cpu_cores:4,memory_gb:8,system_disk:'高性能云硬盘',system_disk_size_gb:100,data_disk:'',has_security_product:false,security_products:[] as number[],ip_address:'',remarks:''});
const createForm=ref(defaultForm());
/** 申请人从系统用户中选定；不选则由后端默认取创建人。 */
function changeApplicant(userId:number){const u=users.value.find(item=>item.id===userId);createForm.value.applicant_user_id=userId;createForm.value.applicant_name=u?.nickname||u?.username||'';}
/** 客户与云资源台账一致：从系统用户中选定，名称随选择带出。 */
function changeCustomer(userId:number){const u=users.value.find(item=>item.id===userId);createForm.value.customer_user_id=userId;createForm.value.customer_name=u?.nickname||u?.username||'';}

// ---- 附件上传（合同 / 交付材料），复用 infra_file + RustFS 通道 ----
interface AttachmentItem { uid: number|string; name: string; id: number; status?: string }
const uploading=ref(false);
const contractFiles=ref<AttachmentItem[]>([]);
const deliverVisible=ref(false); const deliverTarget=ref<Ticket|null>(null);
const deliverComment=ref(''); const deliverFiles=ref<AttachmentItem[]>([]);
const detailAttachments=ref<any[]>([]);
async function uploadTo(list:{value:AttachmentItem[]}, file: File){
  try{
    uploading.value=true;
    const resp=await uploadFile({file}) as any;
    list.value.push({uid:resp.id,name:resp.name||file.name,id:resp.id,status:'done'});
  }catch{message.error('附件上传失败');}
  finally{uploading.value=false;}
  return false; // 阻止 a-upload 默认行为，已手动上传
}
const uploadContractFile=(file:File)=>uploadTo(contractFiles,file);
const uploadDeliverFile=(file:File)=>uploadTo(deliverFiles,file);
const removeContractAttachment=(f:any)=>removeAttachment(contractFiles,f.uid);
const removeDeliverAttachment=(f:any)=>removeAttachment(deliverFiles,f.uid);
function removeAttachment(list:{value:AttachmentItem[]}, uid:number|string){list.value=list.value.filter(item=>item.uid!==uid);return true;}
async function downloadAttachment(file:any){
  try{
    const blob=await requestClient.get<Blob>(file.url,{responseType:'blob'});
    const link=document.createElement('a');
    link.href=URL.createObjectURL(blob);
    link.download=file.name||'attachment';
    link.click();
    URL.revokeObjectURL(link.href);
  }catch{message.error('附件下载失败');}
}
function openDeliver(ticket:Ticket){deliverTarget.value=ticket;deliverComment.value='';deliverFiles.value=[];deliverVisible.value=true;}
async function confirmDeliver(){
  await deliverTicket(deliverTarget.value!.id!,{comment:deliverComment.value,fileIds:deliverFiles.value.map(item=>item.id)});
  message.success('工单已交付');deliverVisible.value=false;fetchData();
}

const statusMap:Record<string,{color:string;label:string}>={pending_approval:{color:'orange',label:'待审批'},approved:{color:'blue',label:'已批准'},rejected:{color:'red',label:'已拒绝'},pending_provision:{color:'cyan',label:'待配置'},provisioning:{color:'blue',label:'配置中'},pending_delivery:{color:'purple',label:'待交付'},delivered:{color:'green',label:'已交付'},expired:{color:'red',label:'已到期'},archived:{color:'default',label:'已归档'}};
const stepIndex:Record<string,number>={pending_approval:0,approved:1,pending_provision:1,provisioning:2,pending_delivery:3,delivered:4,expired:4,archived:4};
const typeLabel:Record<string,string>={cloud:'云资源',network:'网络策略',physical:'物理资源'};
const ticketTypeLabel:Record<string,string>={create:'新建资源',resize:'配置变更',disk_expand:'磁盘扩容',network_change:'网络变更',renew:'续期',migration:'资源迁移',stop:'停机',start:'启用',reclaim:'资源回收'};
const isExistingResourceTicket=computed(()=>createForm.value.ticket_type!=='create');
const isHighRiskTicket=computed(()=>['migration','stop','reclaim'].includes(createForm.value.ticket_type));
const tabs=[{key:'all',label:'全部工单',statuses:[]},{key:'approval',label:'待我审批',statuses:['pending_approval']},{key:'provision',label:'待配置',statuses:['approved','pending_provision','provisioning']},{key:'delivery',label:'待交付',statuses:['pending_delivery']},{key:'completed',label:'已完成',statuses:['delivered','expired','archived','rejected']}];
const filteredTickets=computed(()=>{const tab=tabs.find(item=>item.key===activeTab.value);return !tab?.statuses.length?searchedTickets.value:searchedTickets.value.filter(ticket=>tab.statuses.includes(ticket.ticket_status));});
const columns=[{title:'工单名称',dataIndex:'ecs_name'},{title:'工单类型',key:'ticket_type'},{title:'申请人',dataIndex:'applicant_name'},{title:'资源类型',key:'resource_type'},{title:'状态',key:'ticket_status'},{title:'审批进度',key:'approval_progress'},{title:'云平台',key:'cloud_platform_id'},{title:'规格',key:'spec'},{title:'客户',dataIndex:'customer_name'},{title:'创建时间',dataIndex:'created_at'},{title:'操作',key:'actions',width:300}];

async function fetchData(){loading.value=true;try{const rows=await Promise.all([getResourceTicketList(),getCloudZoneList(),getCloudPlatformList(),getMachineRoomList(),getSecurityProductList(),getCloudProviderConfigList(),getCloudResourceList(),getPhysicalResourceList(),getUserPage({pageNo:1,pageSize:500})]);data.value=rows[0] as any;zones.value=rows[1] as any;platforms.value=rows[2] as any;rooms.value=rows[3] as any;securityProducts.value=rows[4] as any;providerConfigs.value=rows[5] as any;businessResources.value=[...(rows[6] as any[]).map(item=>({...item,resource_type:'cloud'})),...(rows[7] as any[]).map(item=>({...item,resource_type:'physical'}))];users.value=((rows[8] as any)?.list??[]) as any;}catch{message.error('加载失败');}finally{loading.value=false;}}
const route = useRoute();
const router = useRouter();
onMounted(async () => {
  await fetchData();
  // 闭环入口：从云资源/物理资源台账跳转过来时预填目标资源并打开创建弹窗。
  const resourceId = Number(route.query.resource);
  if (resourceId) {
    createForm.value.ticket_type = (route.query.type as string) || 'migration';
    createForm.value.target_resource_id = resourceId;
    const hit = businessResources.value.find((item) => item.id === resourceId);
    if (hit) {
      targetResourceType.value = hit.resource_type;
      createForm.value.resource_type = hit.resource_type;
    }
    createVisible.value = true;
  }
});
// keep-alive 标签页切回时刷新，资源区/云平台/机房等参照数据增改后不会过期
onActivated(fetchData);
async function openCreate(){await fetchData();createForm.value=defaultForm();createVisible.value=true;}
async function showDetail(ticket:Ticket){detailTicket.value=ticket;approvalHistory.value=ticket.id?await getTicketApprovalHistory(ticket.id):[];detailAttachments.value=ticket.id?await getTicketAttachments(ticket.id):[];detailVisible.value=true;}
function handlePlatformChange(platformId:number){const platform=platforms.value.find(item=>item.id===platformId);const zone=zones.value.find(item=>item.id===platform?.zone_id);createForm.value.provider_id=zone?.provider_id;createForm.value.zone_name=zone?.zone_name||'';createForm.value.cloud_platform_name=platform?.platform_name||'';createForm.value.cloud_region=zone?.zone_name||platform?.platform_name||'';createForm.value.machine_room_id=undefined;}
function handleResourceTypeChange(){createForm.value.cloud_platform_id=undefined;createForm.value.machine_room_id=undefined;createForm.value.provider_id=undefined;createForm.value.zone_name='';}
async function handleCreate(){if(!createForm.value.ecs_name){message.warning('请输入工单名称');return;}if(createForm.value.ticket_type==='renew'&&!createForm.value.expire_at){message.warning('续期工单必须填写新的有效期');return;}if(isExistingResourceTicket.value&&!createForm.value.target_resource_id){message.warning('请选择目标业务资源');return;}if(isHighRiskTicket.value&&(!createForm.value.maintenance_window||!createForm.value.backup_confirmed||!createForm.value.rollback_plan)){message.warning('高风险工单必须填写维护窗口、确认备份并填写回滚方案');return;}if(createForm.value.ticket_type==='reclaim'&&!createForm.value.retention_until){message.warning('资源回收必须设置观察保留期');return;}if(createForm.value.ticket_type==='create'&&createForm.value.resource_type==='cloud'&&!createForm.value.cloud_platform_id){message.warning('请选择云平台');return;}const payload={...createForm.value,contract_file_ids:contractFiles.value.map(item=>item.id),cloud_platform_id:createForm.value.resource_type==='cloud'?createForm.value.cloud_platform_id:undefined,target_resource_type:targetResourceType.value,security_products:createForm.value.security_products.join(',')};await createResourceTicket(payload);message.success('工单已提交多级审批');createVisible.value=false;contractFiles.value=[];if(route.query.resource){router.replace({query:{}});}fetchData();}
async function handleApprove(ticket:Ticket){const result:any=await approveTicket(ticket.id!,{approved:true,comment:'审批通过'});message.success(result?.message||'本级审批完成');fetchData();}async function handleReject(ticket:Ticket){await approveTicket(ticket.id!,{approved:false,comment:'审批拒绝'});message.success('已拒绝');fetchData();}
function openProvision(ticket:Ticket){
  provisionTicketRecord.value=ticket;
  const configs=providerConfigs.value.filter(item=>item.platform_id===ticket.cloud_platform_id&&['active','enabled'].includes(item.status));
  let saved:Record<string,any>={};
  try { const parsed=JSON.parse(ticket.target_config||'{}'); if(parsed&&typeof parsed==='object'&&!Array.isArray(parsed)){saved=parsed;} } catch { /* Older tickets may contain a free-text change description. */ }
  provisionForm.value={...emptyProvisionForm(),config_id:saved.configId??(configs.length===1?configs[0]?.id:undefined),image_id:saved.imageId||'',flavor:saved.flavor||ticket.ecs_type||'',availability_zone:saved.availabilityZone||'',subnet_id:saved.subnetId||'',vpc_id:saved.vpcId||'',security_groups:Array.isArray(saved.securityGroups)?saved.securityGroups:[]};
  provisionVisible.value=true;
}
async function handleProvision(){if(!provisionTicketRecord.value?.id){return;}const form=provisionForm.value;if(!form.config_id||!form.image_id.trim()||!form.flavor.trim()||!form.availability_zone.trim()||!form.subnet_id.trim()||!form.security_groups.length){message.warning('请填写凭据、镜像、规格、可用区、子网和安全组');return;}const config=providerConfigs.value.find(item=>item.id===form.config_id);if(['tencent','tencentcloud'].includes(config?.provider)&&!form.vpc_id.trim()){message.warning('腾讯云开通需要 VPC ID');return;}provisioning.value=true;try{await provisionTicket(provisionTicketRecord.value.id,form);message.success('云资源开通完成，工单进入待交付');provisionVisible.value=false;await fetchData();}catch{message.error('开通失败，请查看工单执行日志');}finally{provisioning.value=false;}}
</script>

<template>
  <Page auto-content-height>
    <div style="padding:16px">
      <a-page-header title="资源工单审批" sub-title="资源申请、审批、配置和交付" style="margin-bottom:16px;padding:0" />
      <a-space :size="24" style="margin-bottom:20px"><a-button v-access:code="['infra:resource-ticket:create']" type="primary" @click="openCreate">创建资源工单</a-button><a-input-search v-model:value="searchText" allow-clear placeholder="搜索工单、申请人、客户或区域" style="width:280px" /><a-button @click="fetchData">刷新</a-button></a-space>
      <a-tabs v-model:active-key="activeTab">
        <a-tab-pane v-for="tab in tabs" :key="tab.key"><template #tab>{{ tab.label }} <a-badge :count="tab.statuses.length ? data.filter(ticket=>tab.statuses.includes(ticket.ticket_status)).length : data.length" :number-style="{backgroundColor:'#8c8c8c'}" /></template></a-tab-pane>
      </a-tabs>
      <a-table :columns="columns" :data-source="filteredTickets" :loading="loading" row-key="id" size="middle" :pagination="{pageSize:15}">
        <template #bodyCell="{column,record}">
          <template v-if="column.key==='ticket_type'"><a-tag :color="record.risk_level==='critical'?'red':record.risk_level==='high'?'orange':'blue'">{{ ticketTypeLabel[record.ticket_type]||record.ticket_type }}</a-tag></template>
          <template v-else-if="column.key==='resource_type'"><a-tag>{{ typeLabel[record.resource_type]||record.resource_type }}</a-tag></template>
          <template v-else-if="column.key==='ticket_status'"><a-tag :color="statusMap[record.ticket_status]?.color">{{ statusMap[record.ticket_status]?.label||record.ticket_status }}</a-tag></template>
          <template v-else-if="column.key==='approval_progress'">{{ record.ticket_status==='pending_approval'?`${record.approval_stage}/${record.approval_total} · ${record.current_approval_role||''}`:'-' }}</template>
          <template v-else-if="column.key==='cloud_platform_id'">{{ platforms.find(item=>item.id===record.cloud_platform_id)?.platform_name||'-' }}</template>
          <template v-else-if="column.key==='spec'">{{ record.resource_count||1 }}台 · {{ record.cpu_cores }}核/{{ record.memory_gb }}GB</template>
          <template v-else-if="column.key==='actions'"><a-space :size="12" wrap>
            <a-button size="small" @click="showDetail(record)">详情</a-button>
            <a-button v-if="record.ticket_status==='pending_approval'" v-access:code="['infra:resource-ticket:approve']" type="primary" size="small" @click="handleApprove(record)">通过</a-button>
            <a-button v-if="record.ticket_status==='pending_approval'" v-access:code="['infra:resource-ticket:approve']" danger size="small" @click="handleReject(record)">拒绝</a-button>
            <a-button v-if="(record.ticket_status==='pending_provision'||record.ticket_status==='approved')&&record.ticket_type==='create'&&record.resource_type==='cloud'" v-access:code="['infra:resource-ticket:provision']" type="primary" size="small" @click="openProvision(record)">登记配置</a-button>
            <a-tag v-else-if="record.ticket_status==='pending_provision'||record.ticket_status==='approved'" color="orange">待安全执行</a-tag>
            <a-button v-if="record.ticket_status==='pending_delivery'" v-access:code="['infra:resource-ticket:deliver']" type="primary" size="small" @click="openDeliver(record)">确认交付</a-button>
            <a-popconfirm v-if="record.ticket_status==='rejected'" title="确认删除已拒绝工单?" @confirm="deleteResourceTicket(record.id!).then(fetchData)"><a-button v-access:code="['infra:resource-ticket:delete']" danger size="small">删除</a-button></a-popconfirm>
          </a-space></template>
        </template>
      </a-table>

      <a-modal v-model:open="detailVisible" title="工单审批详情" width="760px" :footer="null">
        <template v-if="detailTicket">
          <a-steps :current="stepIndex[detailTicket.ticket_status]??0" size="small" style="margin-bottom:24px"><a-step title="提交申请"/><a-step title="审批通过"/><a-step title="资源配置"/><a-step title="待交付"/><a-step title="完成"/></a-steps>
          <a-descriptions bordered size="small" :column="2">
            <a-descriptions-item label="工单名称">{{ detailTicket.ecs_name }}</a-descriptions-item><a-descriptions-item label="申请人">{{ detailTicket.applicant_name||'-' }}</a-descriptions-item>
            <a-descriptions-item label="工单类型">{{ ticketTypeLabel[detailTicket.ticket_type]||detailTicket.ticket_type }}</a-descriptions-item><a-descriptions-item label="风险等级">{{ detailTicket.risk_level }}</a-descriptions-item>
            <a-descriptions-item label="状态"><a-tag :color="statusMap[detailTicket.ticket_status]?.color">{{ statusMap[detailTicket.ticket_status]?.label }}</a-tag></a-descriptions-item><a-descriptions-item label="资源类型">{{ typeLabel[detailTicket.resource_type] }}</a-descriptions-item>
            <a-descriptions-item label="资源规格">{{ detailTicket.resource_count||1 }}台 · {{ detailTicket.cpu_cores }}核/{{ detailTicket.memory_gb }}GB</a-descriptions-item><a-descriptions-item label="操作系统">{{ detailTicket.ecs_os||'-' }}</a-descriptions-item>
            <a-descriptions-item label="审批人">{{ detailTicket.approver||'-' }}</a-descriptions-item><a-descriptions-item label="审批意见">{{ detailTicket.approve_comment||'-' }}</a-descriptions-item>
            <a-descriptions-item label="配置人">{{ detailTicket.provisioner||'-' }}</a-descriptions-item><a-descriptions-item label="配置结果">{{ detailTicket.provision_details||'-' }}</a-descriptions-item>
            <a-descriptions-item label="交付人">{{ detailTicket.deliverer||'-' }}</a-descriptions-item><a-descriptions-item label="交付意见">{{ detailTicket.deliver_comment||'-' }}</a-descriptions-item>
            <a-descriptions-item v-if="detailTicket.expire_at" label="有效期至"><a-tag color="red">{{ detailTicket.expire_at }}</a-tag></a-descriptions-item><a-descriptions-item v-if="detailTicket.expire_at" label="到期提醒">{{ {0:'未到提醒时间',1:'已提醒（3天前）',2:'已提醒（1天前）',3:'已到期转回收'}[detailTicket.expiry_remind_stage ?? 0] || '-' }}</a-descriptions-item>
            <a-descriptions-item v-if="detailTicket.maintenance_window" label="维护窗口">{{ detailTicket.maintenance_window }}</a-descriptions-item><a-descriptions-item v-if="detailTicket.rollback_plan" label="回滚方案">{{ detailTicket.rollback_plan }}</a-descriptions-item>
          </a-descriptions>
          <template v-if="detailAttachments.length">
            <a-divider orientation="left">附件</a-divider>
            <a-space direction="vertical" style="width:100%">
              <div v-if="detailAttachments.some(item=>item.kind==='contract')"><span style="font-weight:600;margin-right:8px">合同附件：</span><a v-for="item in detailAttachments.filter(i=>i.kind==='contract')" :key="item.id" style="margin-right:12px" @click.prevent="downloadAttachment(item)">{{ item.name }}（{{ (item.size/1024).toFixed(0) }}KB）</a></div>
              <div v-if="detailAttachments.some(item=>item.kind==='delivery')"><span style="font-weight:600;margin-right:8px">交付材料：</span><a v-for="item in detailAttachments.filter(i=>i.kind==='delivery')" :key="item.id" style="margin-right:12px" @click.prevent="downloadAttachment(item)">{{ item.name }}（{{ (item.size/1024).toFixed(0) }}KB）</a></div>
            </a-space>
          </template>
          <a-divider orientation="left">审批记录</a-divider>
          <a-timeline><a-timeline-item v-for="item in approvalHistory" :key="item.id" :color="item.decision==='approved'?'green':'red'">第 {{ item.stage_no }} 级 · {{ item.role_name }} · {{ item.approver }} · {{ item.decision==='approved'?'通过':'拒绝' }}<div style="color:var(--ant-color-text-secondary)">{{ item.comment||'无意见' }}</div></a-timeline-item></a-timeline>
        </template>
      </a-modal>

      <a-modal v-model:open="createVisible" title="创建资源工单" @ok="handleCreate" width="820px">
        <a-form layout="vertical">
          <a-row :gutter="16">
            <a-col :span="12"><a-form-item label="工单类型" required><a-select v-model:value="createForm.ticket_type"><a-select-option v-for="(label,key) in ticketTypeLabel" :key="key" :value="key">{{ label }}</a-select-option></a-select></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="工单名称" required><a-input v-model:value="createForm.ecs_name" placeholder="生产环境 Web 服务器" /></a-form-item></a-col>
            <a-col v-if="isExistingResourceTicket" :span="24"><a-form-item label="目标业务资源" required><a-select v-model:value="createForm.target_resource_id" show-search :options="businessResources.map(item=>({value:item.id,label:`${item.resource_type==='physical'?'【物理】':'【云】'}${item.ecs_name} / ${item.resource_type==='physical'?item.management_ip||item.business_ip||'无 IP':item.ip_address||'无 IP'}`}))" @change="(value:number)=>{const hit=businessResources.find(item=>item.id===value);if(hit){targetResourceType=hit.resource_type;createForm.resource_type=hit.resource_type;}}" /></a-form-item></a-col>
            <a-col v-if="isExistingResourceTicket" :span="24"><a-form-item label="目标配置/变更内容" required><a-textarea v-model:value="createForm.target_config" :rows="2" placeholder="说明变更后的配置或操作范围" /></a-form-item></a-col>
            <a-col v-if="isHighRiskTicket" :span="12"><a-form-item label="维护窗口" required><a-input v-model:value="createForm.maintenance_window" type="datetime-local" /></a-form-item></a-col>
            <a-col v-if="createForm.ticket_type==='reclaim'" :span="12"><a-form-item label="回收保留期截止时间"><a-input v-model:value="createForm.retention_until" type="datetime-local" /></a-form-item></a-col>
            <a-col v-if="isHighRiskTicket" :span="24"><a-form-item label="回滚方案" required><a-textarea v-model:value="createForm.rollback_plan" :rows="2" /></a-form-item></a-col>
            <a-col v-if="isHighRiskTicket" :span="12"><a-form-item><a-checkbox v-model:checked="createForm.backup_confirmed">已确认完成数据备份</a-checkbox></a-form-item></a-col><a-col v-if="isHighRiskTicket" :span="12"><a-form-item><a-checkbox v-model:checked="createForm.allow_interruption">允许维护窗口内业务中断</a-checkbox></a-form-item></a-col>
            <template v-if="createForm.ticket_type==='create'">
            <a-col :span="12"><a-form-item label="资源类型" required><a-select v-model:value="createForm.resource_type" @change="handleResourceTypeChange"><a-select-option value="cloud">云资源</a-select-option><a-select-option value="physical">物理资源</a-select-option><a-select-option value="network">网络策略</a-select-option></a-select></a-form-item></a-col>
            <a-col v-if="createForm.resource_type==='cloud'" :span="12"><a-form-item label="所属云平台" required><a-select v-model:value="createForm.cloud_platform_id" allow-clear :options="platforms.map(item=>({value:item.id,label:item.platform_name}))" @change="handlePlatformChange" /></a-form-item></a-col>
            <a-col v-if="createForm.resource_type==='cloud'" :span="12"><a-form-item label="物理机房"><a-select v-model:value="createForm.machine_room_id" allow-clear placeholder="选择云平台下的物理机房" :options="rooms.filter(item=>!createForm.cloud_platform_id||item.platform_id===createForm.cloud_platform_id).map(item=>({value:item.id,label:item.room_name}))" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="申请人（不选默认当前登录人）"><a-select v-model:value="createForm.applicant_user_id" :options="users.map(u=>({value:u.id,label:u.nickname||u.username}))" placeholder="请选择申请人" show-search option-filter-prop="label" allow-clear @update:value="changeApplicant" /></a-form-item></a-col><a-col :span="8"><a-form-item label="客户（用户）" required><a-select v-model:value="createForm.customer_user_id" :options="users.map(u=>({value:u.id,label:u.nickname||u.username}))" placeholder="请选择客户用户" show-search option-filter-prop="label" @update:value="changeCustomer" /></a-form-item></a-col><a-col :span="8"><a-form-item label="所属业务项目"><a-input v-model:value="createForm.project_name" placeholder="如 惠企通平台 / 不动产系统" /></a-form-item></a-col><a-col :span="8"><a-form-item label="应用系统"><a-input v-model:value="createForm.application_name" /></a-form-item></a-col><a-col :span="8"><a-form-item label="合同编号（云框架合同）"><a-input v-model:value="createForm.contract_name" placeholder="如 HT-2026-政务云框架" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="资源数量"><a-input-number v-model:value="createForm.resource_count" :min="1" style="width:100%" /></a-form-item></a-col><a-col :span="8"><a-form-item label="CPU 核数"><a-input-number v-model:value="createForm.cpu_cores" :min="1" style="width:100%" /></a-form-item></a-col><a-col :span="8"><a-form-item label="内存 GB"><a-input-number v-model:value="createForm.memory_gb" :min="1" style="width:100%" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="实例规格"><a-input v-model:value="createForm.ecs_type" /></a-form-item></a-col><a-col :span="8"><a-form-item label="操作系统"><a-select v-model:value="createForm.ecs_os"><a-select-option value="Linux">Linux</a-select-option><a-select-option value="Windows">Windows</a-select-option></a-select></a-form-item></a-col><a-col :span="8"><a-form-item label="云区域（随平台自动带出）"><a-input v-model:value="createForm.cloud_region" disabled placeholder="选择云平台后自动带出其资源区" /></a-form-item></a-col>
            <a-col :span="8"><a-form-item label="系统盘类型"><a-input v-model:value="createForm.system_disk" /></a-form-item></a-col><a-col :span="8"><a-form-item label="系统盘 GB"><a-input-number v-model:value="createForm.system_disk_size_gb" :min="20" style="width:100%" /></a-form-item></a-col><a-col :span="8"><a-form-item label="数据盘"><a-input v-model:value="createForm.data_disk" placeholder="例如：500GB SSD" /></a-form-item></a-col>
            <a-col :span="12"><a-form-item label="预分配 IP"><a-input v-model:value="createForm.ip_address" /></a-form-item></a-col>
            <a-col :span="24"><a-form-item><a-checkbox v-model:checked="createForm.has_security_product">需要安全产品</a-checkbox></a-form-item></a-col>
            <a-col v-if="createForm.has_security_product" :span="24"><a-form-item label="安全产品"><a-select v-model:value="createForm.security_products" mode="multiple" :options="securityProducts.map(item=>({value:item.id,label:item.name}))" /></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="合同附件（支持 Word / PDF / Excel / 压缩包，单个 ≤50MB）">
              <a-upload :file-list="contractFiles" accept=".doc,.docx,.pdf,.xls,.xlsx,.zip,.rar,.7z" :before-upload="uploadContractFile" :remove="removeContractAttachment">
                <a-button :loading="uploading">选择文件上传</a-button>
              </a-upload>
            </a-form-item></a-col>
            </template>
            <a-col v-if="['create','renew'].includes(createForm.ticket_type)" :span="8"><a-form-item :label="createForm.ticket_type==='renew'?'续期至（新有效期，必填）':'有效期至（临时资源到期时间）'" :required="createForm.ticket_type==='renew'"><a-input v-model:value="createForm.expire_at" type="datetime-local" /><span style="color:var(--ant-color-text-secondary);font-size:12px">到期前 3 天、1 天会提醒申请人与运维；到期未续期将自动进入回收流程</span></a-form-item></a-col>
            <a-col :span="24"><a-form-item label="申请说明"><a-textarea v-model:value="createForm.remarks" :rows="3" /></a-form-item></a-col>
          </a-row>
        </a-form>
      </a-modal>
      <a-modal v-model:open="deliverVisible" title="确认交付" @ok="confirmDeliver">
        <a-form layout="vertical">
          <a-form-item label="交付备注"><a-textarea v-model:value="deliverComment" :rows="2" placeholder="交付说明、验收结论等" /></a-form-item>
          <a-form-item label="交付材料（验收报告、配置清单等；支持 Word / PDF / Excel / 压缩包，单个 ≤50MB）">
            <a-upload :file-list="deliverFiles" accept=".doc,.docx,.pdf,.xls,.xlsx,.zip,.rar,.7z" :before-upload="uploadDeliverFile" :remove="removeDeliverAttachment">
              <a-button :loading="uploading">选择文件上传</a-button>
            </a-upload>
          </a-form-item>
        </a-form>
      </a-modal>
      <a-modal v-model:open="provisionVisible" title="登记资源配置" :confirm-loading="provisioning" @ok="handleProvision" width="560px">
        <a-alert message="当前阶段记录人工或外部平台配置结果，不会直接调用云厂商创建实例。" type="info" show-icon style="margin-bottom:16px" />
        <a-form layout="vertical">
          <a-form-item label="云厂商对接配置" required><a-select v-model:value="provisionForm.config_id" :options="providerConfigs.filter(item=>item.platform_id===provisionTicketRecord?.cloud_platform_id&&['active','enabled'].includes(item.status)).map(item=>({value:item.id,label:`${item.account_name} / ${item.region_name}`}))" placeholder="选择实际用于开通的凭据" /></a-form-item>
          <a-form-item label="镜像 ID" required><a-input v-model:value="provisionForm.image_id" placeholder="云平台中的镜像 ID" /></a-form-item>
          <a-form-item label="实例规格"><a-input v-model:value="provisionForm.flavor" placeholder="例如 ecs.c6.large" /></a-form-item>
          <a-form-item label="可用区" required><a-input v-model:value="provisionForm.availability_zone" placeholder="例如 cn-hangzhou-h" /></a-form-item>
          <a-form-item label="子网 / 交换机 ID" required><a-input v-model:value="provisionForm.subnet_id" /></a-form-item>
          <a-form-item label="VPC ID（腾讯云必填）"><a-input v-model:value="provisionForm.vpc_id" /></a-form-item>
          <a-form-item label="安全组 ID" required><a-select v-model:value="provisionForm.security_groups" mode="tags" :token-separators="[',']" placeholder="输入安全组 ID，回车添加" /></a-form-item>
          <a-form-item label="配置说明"><a-textarea v-model:value="provisionForm.details" :rows="3" /></a-form-item>
        </a-form>
      </a-modal>
    </div>
  </Page>
</template>
