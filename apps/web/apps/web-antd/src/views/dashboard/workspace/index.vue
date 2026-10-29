<script lang="ts" setup>
import type {
  WorkbenchProjectItem,
  WorkbenchQuickNavItem,
  WorkbenchTodoItem,
} from '@vben/common-ui';

import { ref } from 'vue';
import { useRouter } from 'vue-router';

import {
  WorkbenchHeader,
  WorkbenchProject,
  WorkbenchQuickNav,
  WorkbenchTodo,
} from '@vben/common-ui';
import { preferences } from '@vben/preferences';
import { useUserStore } from '@vben/stores';

const userStore = useUserStore();
const router = useRouter();

const projectItems: WorkbenchProjectItem[] = [
  {
    color: '#1677ff',
    content: '统一维护资产采集表字段、归属和运行状态',
    date: '持续维护',
    group: '资产中心',
    icon: 'lucide:database',
    title: '资产台账',
    url: '/asset-center/assets',
  },
  {
    color: '#722ed1',
    content: '定义配置项结构、属性、实例及实例关系',
    date: '动态建模',
    group: '配置管理',
    icon: 'lucide:boxes',
    title: 'CMDB',
    url: '/cmdb/model',
  },
  {
    color: '#13c2c2',
    content: '管理云平台、区域和云厂商接入凭据',
    date: '多云接入',
    group: '云管理',
    icon: 'lucide:cloud-cog',
    title: '云平台',
    url: '/infra-center/cloud-platform',
  },
  {
    color: '#52c41a',
    content: '维护业务应用与其关联的计算、网络资源',
    date: '关系清晰',
    group: '业务中心',
    icon: 'lucide:layout-grid',
    title: '业务应用',
    url: '/biz-center/business-application',
  },
  {
    color: '#fa8c16',
    content: '通过审批规则和 OpenTofu 管理资源交付',
    date: '流程留痕',
    group: '运维中心',
    icon: 'lucide:clipboard-check',
    title: '资源工单',
    url: '/ops-center/resource-ticket',
  },
  {
    color: '#eb2f96',
    content: '复用统一模型配置开展对话、知识库和智能运维',
    date: '统一模型',
    group: 'AI 大模型',
    icon: 'lucide:brain-circuit',
    title: 'AI 助手',
    url: '/ai/chat',
  },
];

const quickNavItems: WorkbenchQuickNavItem[] = [
  {
    color: '#1677ff',
    icon: 'lucide:server',
    title: '资产台账',
    url: '/asset-center/assets',
  },
  {
    color: '#722ed1',
    icon: 'lucide:network',
    title: '网络策略',
    url: '/asset-center/network-policy',
  },
  {
    color: '#13c2c2',
    icon: 'lucide:scan-line',
    title: '扫描任务',
    url: '/asset-center/task',
  },
  {
    color: '#fa541c',
    icon: 'lucide:shield-alert',
    title: '风险管理',
    url: '/asset-center/risk',
  },
  {
    color: '#52c41a',
    icon: 'lucide:ticket-check',
    title: '资源工单',
    url: '/ops-center/resource-ticket',
  },
  {
    color: '#faad14',
    icon: 'lucide:building-2',
    title: '租户与网段',
    url: '/system/tenant',
  },
];

const capabilityItems = ref<WorkbenchTodoItem[]>([
  {
    completed: true,
    content: '资产台账按资产采集表字段统一维护，并可匹配网络策略。',
    date: '资产中心',
    title: '资产数据标准化',
  },
  {
    completed: true,
    content: '租户网段在租户页面集中维护，不再提供独立网段菜单。',
    date: '系统管理',
    title: '租户网络边界',
  },
  {
    completed: true,
    content: 'AI 对话、知识库与智能运维共用 AI 大模型配置。',
    date: 'AI 大模型',
    title: '统一模型管理',
  },
  {
    completed: false,
    content: '云厂商连接和扫描执行能力需要按实际环境持续接入验证。',
    date: '持续建设',
    title: '外部系统接入',
  },
]);

function navTo(nav: WorkbenchProjectItem | WorkbenchQuickNavItem) {
  if (!nav.url?.startsWith('/')) return;
  void router.push(nav.url);
}
</script>

<template>
  <div class="p-5">
    <WorkbenchHeader
      :avatar="userStore.userInfo?.avatar || preferences.app.defaultAvatar"
    >
      <template #title>
        您好，{{ userStore.userInfo?.nickname || userStore.userInfo?.username }}
      </template>
      <template #description>
        在 RustSet 统一管理资产、CMDB、云资源与运维流程。
      </template>
    </WorkbenchHeader>

    <div class="flex flex-col lg:flex-row">
      <div class="mr-4 w-full lg:w-3/5">
        <WorkbenchProject
          :items="projectItems"
          title="业务模块"
          @click="navTo"
        />
      </div>
      <div class="w-full lg:w-2/5">
        <WorkbenchQuickNav
          :items="quickNavItems"
          class="lg:mt-0"
          title="快捷导航"
          @click="navTo"
        />
        <WorkbenchTodo :items="capabilityItems" class="mt-5" title="平台能力" />
      </div>
    </div>
  </div>
</template>
