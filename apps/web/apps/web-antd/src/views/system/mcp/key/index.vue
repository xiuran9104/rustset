<script lang="ts" setup>
import type { VxeTableGridOptions } from '#/adapter/vxe-table';
import type { SystemMcpKeyApi } from '#/api/system/mcp-key';

import { ref } from 'vue';

import { Page, useVbenModal } from '@vben/common-ui';

import { message } from 'ant-design-vue';

import { ACTION_ICON, TableAction, useVbenVxeGrid } from '#/adapter/vxe-table';
import {
  deleteMcpKey,
  getMcpKeyPage,
  updateMcpKey,
} from '#/api/system/mcp-key';

import { useGridColumns } from './data';
import KeyForm from './modules/form.vue';

/** 一次性密钥展示弹窗状态 */
const createdKey = ref<null | { apiKey: string; name: string }>(null);

const [FormModal, formModalApi] = useVbenModal({
  connectedComponent: KeyForm,
  destroyOnClose: true,
});

function handleCreate() {
  formModalApi.setData(null).open();
}

async function handleToggle(row: SystemMcpKeyApi.McpApiKey, enabled: boolean) {
  try {
    await updateMcpKey({ id: row.id, enabled: enabled ? 1 : 0 });
    row.enabled = enabled ? 1 : 0;
  } catch {
    // 失败时保持原值（vxe 行数据由插槽重渲染）
  }
}

async function handleDelete(row: SystemMcpKeyApi.McpApiKey) {
  await deleteMcpKey(row.id);
  message.success(`密钥 ${row.name} 已删除`);
  gridApi.query();
}

async function copyKey(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    message.success('已复制到剪贴板');
  } catch {
    message.error('复制失败，请手动选择复制');
  }
}

const [Grid, gridApi] = useVbenVxeGrid({
  gridOptions: {
    columns: useGridColumns(),
    height: 'auto',
    keepSource: true,
    proxyConfig: {
      ajax: {
        query: async ({ page }) => {
          return await getMcpKeyPage({
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
  } as VxeTableGridOptions<SystemMcpKeyApi.McpApiKey>,
  gridEvents: {},
});

function handleFormSuccess(result: null | { apiKey: string; name: string }) {
  gridApi.query();
  if (result) {
    createdKey.value = result;
  }
}
</script>

<template>
  <Page auto-content-height>
    <FormModal @success="handleFormSuccess" />
    <Grid table-title="MCP API 密钥">
      <template #toolbar-tools>
        <TableAction
          :actions="[
            {
              label: '创建密钥',
              type: 'primary',
              icon: ACTION_ICON.ADD,
              auth: ['system:mcp-key:create'],
              onClick: handleCreate,
            },
          ]"
        />
      </template>
      <template #enabled="{ row }">
        <a-switch
          :checked="row.enabled === 1"
          checked-children="启用"
          un-checked-children="停用"
          @change="(val: any) => handleToggle(row, !!val)"
        />
      </template>
      <template #actions="{ row }">
        <TableAction
          :actions="[
            {
              label: '删除',
              type: 'link',
              danger: true,
              icon: ACTION_ICON.DELETE,
              auth: ['system:mcp-key:delete'],
              popConfirm: {
                title: `确认删除密钥「${row.name}」？删除后客户端立即失效。`,
                confirm: handleDelete.bind(null, row),
              },
            },
          ]"
        />
      </template>
    </Grid>

    <!-- 创建成功：完整密钥仅此一次展示 -->
    <a-modal
      :open="createdKey !== null"
      title="密钥已创建"
      :footer="null"
      :mask-closable="false"
      @cancel="createdKey = null"
    >
      <a-alert
        type="warning"
        show-icon
        message="请立即复制保存：完整密钥仅展示这一次，系统只保留哈希，之后无法找回。"
        class="mb-4"
      />
      <div class="mb-2 text-sm text-gray-500">{{ createdKey?.name }}</div>
      <div class="flex items-center gap-2">
        <span class="flex-1 break-all rounded bg-gray-100 p-2 font-mono text-sm">
          {{ createdKey?.apiKey }}
        </span>
        <a-button type="primary" @click="copyKey(createdKey?.apiKey ?? '')">
          复制
        </a-button>
      </div>
    </a-modal>
  </Page>
</template>
