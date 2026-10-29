<script lang="ts" setup>
import type { InfraMonitorApi } from '#/api/infra/monitor';
import { onMounted, ref } from 'vue';
import { Page } from '@vben/common-ui';
import { Button, Card, Statistic, Table, Tag } from 'ant-design-vue';
import { useRouter } from 'vue-router';
import { getTraceMonitor } from '#/api/infra/monitor';

const router = useRouter();
const data = ref<InfraMonitorApi.TraceMonitor>();
const loading = ref(false);
async function load() { loading.value = true; try { data.value = await getTraceMonitor(); } finally { loading.value = false; } }
onMounted(load);
</script>

<template>
  <Page auto-content-height>
    <div class="mb-4 flex items-center justify-between">
      <span style="color: var(--ant-color-text-secondary)">
        监控快照：统计最近 24 小时，链路仅显示最近 100 条；完整审计明细请查看访问日志。
      </span>
      <Button type="primary" :loading="loading" @click="load">刷新链路</Button>
    </div>
    <div class="grid grid-cols-2 gap-4 md:grid-cols-4">
      <Card><Statistic title="24 小时请求" :value="data?.total || 0" /></Card>
      <Card><Statistic title="失败请求" :value="data?.failed || 0" /></Card>
      <Card><Statistic title="平均耗时" :precision="2" :value="data?.averageDurationMs || 0" suffix="ms" /></Card>
      <Card><Statistic title="最大耗时" :value="data?.maximumDurationMs || 0" suffix="ms" /></Card>
    </div>
    <Card class="mt-4" title="最近请求链路">
      <template #extra><a @click="router.push('/infra/log/api-access-log')">查看完整访问日志</a></template>
      <Table :data-source="data?.traces || []" :loading="loading" :pagination="{ pageSize: 20 }" row-key="traceId" size="small">
        <Table.Column data-index="traceId" title="Trace ID" width="260" /><Table.Column data-index="service" title="服务" width="150" />
        <Table.Column data-index="method" title="方法" width="90" /><Table.Column data-index="url" title="请求地址" />
        <Table.Column data-index="durationMs" title="耗时(ms)" width="110" />
        <Table.Column key="status" title="状态" width="90"><template #default="{ record }"><Tag :color="record.status >= 400 ? 'red' : 'green'">{{ record.status }}</Tag></template></Table.Column>
        <Table.Column data-index="startedAt" title="开始时间" width="190" />
      </Table>
    </Card>
  </Page>
</template>
