<script lang="ts" setup>
import type { VxeTableGridOptions } from '#/adapter/vxe-table';
import type { SystemTenantApi } from '#/api/system/tenant';

import { ref } from 'vue';

import { confirm, DocAlert, Page, useVbenModal } from '@vben/common-ui';
import { downloadFileFromBlobPart, isEmpty } from '@vben/utils';

import { Button, message, Space, Upload } from 'ant-design-vue';

import { ACTION_ICON, TableAction, useVbenVxeGrid } from '#/adapter/vxe-table';
import {
  deleteTenant,
  deleteTenantList,
  exportTenant,
  getTenantPage,
  importTenantCsv,
  tenantImportTemplate,
} from '#/api/system/tenant';
import { $t } from '#/locales';

import { useGridColumns, useGridFormSchema } from './data';
import Form from './modules/form.vue';
import NetZone from './modules/net-zone.vue';

const [FormModal, formModalApi] = useVbenModal({
  connectedComponent: Form,
  destroyOnClose: true,
});

const [NetZoneModal, netZoneModalApi] = useVbenModal({
  connectedComponent: NetZone,
  destroyOnClose: true,
});

/** 刷新表格 */
function handleRefresh() {
  gridApi.query();
}

/** 导出表格 */
async function handleExport() {
  const data = await exportTenant(await gridApi.formApi.getValues());
  downloadFileFromBlobPart({ fileName: '租户.csv', source: data });
}

async function handleTemplate() {
  const data = await tenantImportTemplate();
  downloadFileFromBlobPart({ fileName: '租户导入模板.csv', source: data });
}

async function handleImport(file: File) {
  const result: any = await importTenantCsv(file);
  message.success(
    `导入完成：成功 ${result.created || 0} 条，失败 ${result.failed || 0} 条`,
  );
  handleRefresh();
  return false;
}

/** 创建租户 */
function handleCreate() {
  formModalApi.setData(null).open();
}

/** 编辑租户 */
function handleEdit(row: SystemTenantApi.Tenant) {
  formModalApi.setData(row).open();
}

function handleNetZones(row: SystemTenantApi.Tenant) {
  netZoneModalApi.setData(row).open();
}

/** 删除租户 */
async function handleDelete(row: SystemTenantApi.Tenant) {
  const hideLoading = message.loading({
    content: $t('ui.actionMessage.deleting', [row.name]),
    duration: 0,
  });
  try {
    await deleteTenant(row.id!);
    message.success($t('ui.actionMessage.deleteSuccess', [row.name]));
    handleRefresh();
  } finally {
    hideLoading();
  }
}

/** 批量删除租户 */
async function handleDeleteBatch() {
  await confirm($t('ui.actionMessage.deleteBatchConfirm'));
  const hideLoading = message.loading({
    content: $t('ui.actionMessage.deletingBatch'),
    duration: 0,
  });
  try {
    await deleteTenantList(checkedIds.value);
    checkedIds.value = [];
    message.success($t('ui.actionMessage.deleteSuccess'));
    handleRefresh();
  } finally {
    hideLoading();
  }
}

const checkedIds = ref<number[]>([]);
function handleRowCheckboxChange({
  records,
}: {
  records: SystemTenantApi.Tenant[];
}) {
  checkedIds.value = records.map((item) => item.id!);
}

const [Grid, gridApi] = useVbenVxeGrid({
  formOptions: {
    schema: useGridFormSchema(),
  },
  gridOptions: {
    columns: useGridColumns(),
    height: 'auto',
    keepSource: true,
    proxyConfig: {
      ajax: {
        query: async ({ page }, formValues) => {
          return await getTenantPage({
            pageNo: page.currentPage,
            pageSize: page.pageSize,
            ...formValues,
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
      search: true,
    },
  } as VxeTableGridOptions<SystemTenantApi.Tenant>,
  gridEvents: {
    checkboxAll: handleRowCheckboxChange,
    checkboxChange: handleRowCheckboxChange,
  },
});
</script>
<template>
  <Page auto-content-height>
    <template #doc>
      <DocAlert title="SaaS 多租户" url="https://doc.iocoder.cn/saas-tenant/" />
    </template>

    <FormModal @success="handleRefresh" />
    <NetZoneModal />
    <Grid table-title="租户列表">
      <template #toolbar-tools>
        <Space>
          <TableAction
            :actions="[
            {
              label: $t('ui.actionTitle.create', ['租户']),
              type: 'primary',
              icon: ACTION_ICON.ADD,
              auth: ['system:tenant:create'],
              onClick: handleCreate,
            },
            {
              label: $t('ui.actionTitle.export'),
              type: 'primary',
              icon: ACTION_ICON.DOWNLOAD,
              auth: ['system:tenant:export'],
              onClick: handleExport,
            },
            {
              label: $t('ui.actionTitle.deleteBatch'),
              type: 'primary',
              danger: true,
              icon: ACTION_ICON.DELETE,
              disabled: isEmpty(checkedIds),
              auth: ['system:tenant:delete'],
              onClick: handleDeleteBatch,
            },
            ]"
          />
          <Button v-access:code="['system:tenant:create']" @click="handleTemplate">
            下载 CSV 模板
          </Button>
          <Upload
            :show-upload-list="false"
            accept=".csv,text/csv"
            :before-upload="handleImport"
          >
            <Button v-access:code="['system:tenant:create']">导入 CSV</Button>
          </Upload>
        </Space>
      </template>
      <template #actions="{ row }">
        <TableAction
          :actions="[
            {
              label: '网段',
              type: 'link',
              icon: 'lucide:network',
              auth: ['cmdb:net-zone:query'],
              onClick: handleNetZones.bind(null, row),
            },
            {
              label: $t('common.edit'),
              type: 'link',
              icon: ACTION_ICON.EDIT,
              auth: ['system:tenant:update'],
              onClick: handleEdit.bind(null, row),
            },
            {
              label: $t('common.delete'),
              type: 'link',
              danger: true,
              icon: ACTION_ICON.DELETE,
              auth: ['system:tenant:delete'],
              popConfirm: {
                title: $t('ui.actionMessage.deleteConfirm', [row.name]),
                confirm: handleDelete.bind(null, row),
              },
            },
          ]"
        />
      </template>
    </Grid>
  </Page>
</template>
