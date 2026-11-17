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

/* BPMN 导入 */
const bpmnFileInput = ref<HTMLInputElement>();
function handleImportBpmn() {
  bpmnFileInput.value?.click();
}
async function onBpmnFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (!file) return;
  const xml = await file.text();
  const { requestClient } = await import('#/api/request');
  try {
    const result = await requestClient.post(
      '/infra/flow-definition/import-bpmn',
      { xml },
    );
    message.success(`BPMN 导入成功：${result.name}（${result.stepCount} 步）`, 4);
    gridApi.query();
  } catch (error: any) {
    message.error(`BPMN 导入失败：${error?.message ?? error}`, 5);
  }
  input.value = ''; // 清空以便重复选择同一文件
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
    <input
      ref="bpmnFileInput"
      type="file"
      accept=".bpmn,.xml"
      class="hidden"
      @change="onBpmnFileSelected"
    />
    <Grid table-title="流程定义">
      <template #toolbar-tools>
        <div class="flex gap-2">
          <TableAction
            :actions="[
              {
                label: '新建流程',
                type: 'primary',
                icon: ACTION_ICON.ADD,
                auth: ['infra:flow-definition:create'],
                onClick: handleCreate,
              },
              {
                label: '导入 BPMN',
                icon: ACTION_ICON.UPLOAD,
                auth: ['infra:flow-definition:create'],
                onClick: handleImportBpmn,
              },
            ]"
          />
        </div>
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
