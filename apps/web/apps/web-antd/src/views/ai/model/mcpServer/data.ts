import type { VbenFormSchema } from '#/adapter/form';
import type { VxeTableGridOptions } from '#/adapter/vxe-table';

import { z } from '#/adapter/form';

/** 新增/修改的表单 */
export function useFormSchema(): VbenFormSchema[] {
  return [
    {
      component: 'Input',
      fieldName: 'id',
      dependencies: {
        triggerFields: [''],
        show: () => false,
      },
    },
    {
      component: 'Input',
      fieldName: 'name',
      label: '服务器名称',
      rules: z.string().min(1, { message: '请输入服务器名称' }).max(64),
      componentProps: {
        placeholder: '例如：rustset、github',
      },
    },
    {
      component: 'Input',
      fieldName: 'url',
      label: '端点 URL',
      rules: z
        .string()
        .min(1, { message: '请输入端点 URL' })
        .refine((v) => v.startsWith('http://') || v.startsWith('https://'), {
          message: 'URL 必须以 http:// 或 https:// 开头',
        }),
      componentProps: {
        placeholder: 'http://host:port/mcp（Streamable HTTP）',
      },
    },
    {
      component: 'Input',
      fieldName: 'authHeader',
      label: '鉴权头名',
      defaultValue: 'Authorization',
      componentProps: {
        placeholder: '默认 Authorization，可填 X-Api-Key 等',
      },
    },
    {
      component: 'InputPassword',
      fieldName: 'authToken',
      label: '鉴权头值',
      componentProps: {
        placeholder: '如 Bearer mcp_xxx；编辑时留空表示保持不变',
      },
      formItemClass: 'col-span-2',
      help: '值将封存存储（SM4），保存后不再回显。',
    },
    {
      component: 'RadioGroup',
      fieldName: 'status',
      label: '状态',
      componentProps: {
        options: [
          { label: '启用', value: 1 },
          { label: '停用', value: 0 },
        ],
        buttonStyle: 'solid',
        optionType: 'button',
      },
      defaultValue: 1,
    },
    {
      component: 'Textarea',
      fieldName: 'remark',
      label: '备注',
      componentProps: {
        placeholder: '可选',
      },
    },
  ];
}

/** 表格列 */
export function useGridColumns(): VxeTableGridOptions['columns'] {
  return [
    { type: 'seq', title: '序号', width: 50 },
    { field: 'name', title: '名称', minWidth: 120 },
    { field: 'url', title: '端点 URL', minWidth: 220 },
    {
      field: 'hasToken',
      title: '鉴权',
      width: 70,
      formatter: ({ cellValue }) => (cellValue ? '已配置' : '无'),
    },
    {
      field: 'toolCount',
      title: '工具数',
      width: 80,
      formatter: ({ cellValue }) => String(cellValue ?? 0),
    },
    {
      field: 'lastSyncTime',
      title: '最后同步',
      width: 160,
      formatter: ({ cellValue }) =>
        cellValue
          ? new Date(Number(cellValue)).toLocaleString('zh-CN', { hour12: false })
          : '从未同步',
    },
    {
      field: 'status',
      title: '状态',
      width: 90,
      slots: { default: 'status' },
    },
    {
      field: 'lastError',
      title: '最近错误',
      minWidth: 160,
      showOverflow: 'tooltip',
      formatter: ({ cellValue }) =>
        cellValue ? String(cellValue).slice(0, 120) : '',
    },
    {
      field: 'action',
      title: '操作',
      width: 220,
      fixed: 'right',
      slots: { default: 'action' },
    },
  ];
}
