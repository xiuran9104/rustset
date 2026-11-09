<script lang="ts" setup>
import type { VxeTableGridOptions } from '#/adapter/vxe-table';
import type { AiMcpServerApi } from '#/api/ai/model/mcpServer';

import { ref } from 'vue';

import { Page, useVbenModal } from '@vben/common-ui';

import { Switch, message } from 'ant-design-vue';

import {
  ACTION_ICON,
  TableAction,
  useVbenVxeGrid,
} from '#/adapter/vxe-table';
import {
  deleteMcpServer,
  getMcpServerPage,
  syncMcpServer,
  updateMcpServer,
} from '#/api/ai/model/mcpServer';

import { useGridColumns } from './data';
import Form from './modules/form.vue';

const syncingIds = ref<number[]>([]);
const [FormModal, formModalApi] = useVbenModal({
  connectedComponent: Form,
  destroyOnClose: true,
});

function handleCreate() {
  formModalApi.setData(null).open();
}

function handleEdit(row: AiMcpServerApi.McpServer) {
  formModalApi.setData(row).open();
}

async function handleToggle(row: AiMcpServerApi.McpServer, enabled: boolean) {
  try {
    await updateMcpServer({ ...row, id: row.id, status: enabled ? 1 : 0 });
    row.status = enabled ? 1 : 0;
  } catch {
    // 失败保持原值
  }
}

async function handleSync(row: AiMcpServerApi.McpServer) {
  syncingIds.value = [...syncingIds.value, row.id];
  try {
    const result = await syncMcpServer(row.id);
    message.success(
      `同步完成：新增 ${result.added}、更新 ${result.updated}、下架 ${result.disabled}（共 ${result.toolCount} 个工具）`,
      5,
    );
    gridApi.query();
  } finally {
    syncingIds.value = syncingIds.value.filter((id) => id !== row.id);
  }
}

async function handleDelete(row: AiMcpServerApi.McpServer) {
  await deleteMcpServer(row.id);
  message.success(`MCP 服务器 ${row.name} 已删除`);
  gridApi.query();
}

const [Grid, gridApi] = useVbenVxeGrid({
  gridOptions: {
    columns: useGridColumns(),
    height: 'auto',
    keepSource: true,
    proxyConfig: {
      ajax: {
        query: async ({ page }) => {
          return await getMcpServerPage({
            pageNo: page.currentPage,
            pageSize: page.pageSize,
          });
        },
      },
    },
    rowConfig: {
      keyField: 'id',
      isHover: true,
    },
    toolbarConfig: {
      refresh: true,
      search: false,
    },
  } as VxeTableGridOptions<AiMcpServerApi.McpServer>,
  gridEvents: {},
});
</script>

<template>
  <Page auto-content-height>
    <FormModal @success="gridApi.query()" />
    <Grid table-title="MCP 服务器（外部工具源）">
      <template #toolbar-tools>
        <TableAction
          :actions="[
            {
              label: '登记服务器',
              type: 'primary',
              icon: ACTION_ICON.ADD,
              auth: ['ai:mcp-server:create'],
              onClick: handleCreate,
            },
          ]"
        />
      </template>
      <template #status="{ row }">
        <Switch
          :checked="row.status === 1"
          checked-children="启用"
          un-checked-children="停用"
          @change="(val: any) => handleToggle(row, !!val)"
        />
      </template>
      <template #action="{ row }">
        <TableAction
          :actions="[
            {
              label: syncingIds.includes(row.id) ? '同步中…' : '同步工具',
              type: 'link',
              icon: ACTION_ICON.REFRESH,
              auth: ['ai:mcp-server:sync'],
              onClick: handleSync.bind(null, row),
            },
            {
              label: $t('common.edit'),
              type: 'link',
              icon: ACTION_ICON.EDIT,
              auth: ['ai:mcp-server:update'],
              onClick: handleEdit.bind(null, row),
            },
            {
              label: $t('common.delete'),
              type: 'link',
              danger: true,
              icon: ACTION_ICON.DELETE,
              auth: ['ai:mcp-server:delete'],
              popConfirm: {
                title: `确认删除「${row.name}」？其同步工具将一并删除。`,
                confirm: handleDelete.bind(null, row),
              },
            },
          ]"
        />
      </template>
    </Grid>
  </Page>
</template>
