<script lang="ts" setup>
import type { VxeTableGridOptions } from '#/adapter/vxe-table';
import type { AssetOpsFlowApi } from '#/api/asset-ops/flow';

import { ref } from 'vue';

import { Page } from '@vben/common-ui';

import { Switch, message } from 'ant-design-vue';

import {
  ACTION_ICON,
  TableAction,
  useVbenVxeGrid,
} from '#/adapter/vxe-table';
import {
  deleteFlowDefinition,
  getFlowDefinitionPage,
  updateFlowDefinition,
} from '#/api/asset-ops/flow';

import { useGridColumns } from './data';
import Editor from './modules/editor.vue';

const editorRef = ref();
const [Grid, gridApi] = useVbenVxeGrid({
  gridOptions: {
    columns: useGridColumns(),
    height: 'auto',
    keepSource: true,
    proxyConfig: {
      ajax: {
        query: async ({ page }) => {
          return await getFlowDefinitionPage({
            pageNo: page.currentPage,
            pageSize: page.pageSize,
          });
        },
      },
    },
    rowConfig: { keyField: 'id', isHover: true },
    toolbarConfig: { refresh: true, search: false },
  } as VxeTableGridOptions<AssetOpsFlowApi.FlowDefinition>,
  gridEvents: {},
});

function handleCreate() {
  editorRef.value?.open();
}
function handleEdit(row: AssetOpsFlowApi.FlowDefinition) {
  editorRef.value?.open(row);
}
async function handleToggle(
  row: AssetOpsFlowApi.FlowDefinition,
  enabled: boolean,
) {
  try {
    await updateFlowDefinition({
      ...(row as any),
      status: enabled ? 0 : 1,
      definition: row.definition ?? { steps: [] },
    });
    row.status = enabled ? 0 : 1;
  } catch {
    // 失败保持原值
  }
}
async function handleDelete(row: AssetOpsFlowApi.FlowDefinition) {
  await deleteFlowDefinition(row.id);
  message.success(`流程定义 ${row.name} 已删除`);
  gridApi.query();
}
</script>

<template>
  <Page auto-content-height>
    <Editor ref="editorRef" @success="gridApi.query()" />
    <Grid table-title="流程定义">
      <template #toolbar-tools>
        <TableAction
          :actions="[
            {
              label: '新建流程',
              type: 'primary',
              icon: ACTION_ICON.ADD,
              auth: ['infra:flow-definition:create'],
              onClick: handleCreate,
            },
          ]"
        />
      </template>
      <template #status="{ row }">
        <Switch
          :checked="row.status === 0"
          checked-children="启用"
          un-checked-children="停用"
          @change="(val: any) => handleToggle(row, !!val)"
        />
      </template>
      <template #action="{ row }">
        <TableAction
          :actions="[
            {
              label: '编辑',
              type: 'link',
              icon: ACTION_ICON.EDIT,
              auth: ['infra:flow-definition:update'],
              onClick: handleEdit.bind(null, row),
            },
            {
              label: '删除',
              type: 'link',
              danger: true,
              icon: ACTION_ICON.DELETE,
              auth: ['infra:flow-definition:delete'],
              popConfirm: {
                title: `确认删除「${row.name}」？（运行中实例会阻止删除）`,
                confirm: handleDelete.bind(null, row),
              },
            },
          ]"
        />
      </template>
    </Grid>
  </Page>
</template>
