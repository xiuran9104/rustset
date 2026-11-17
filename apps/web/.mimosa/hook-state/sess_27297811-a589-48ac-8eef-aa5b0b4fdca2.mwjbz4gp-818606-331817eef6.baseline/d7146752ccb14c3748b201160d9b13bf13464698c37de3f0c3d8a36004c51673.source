import type { VbenFormSchema } from '#/adapter/form';
import type { VxeTableGridOptions } from '#/adapter/vxe-table';

import { z } from '#/adapter/form';

/** 创建密钥的表单（仅创建；密钥不可编辑） */
export function useFormSchema(): VbenFormSchema[] {
  return [
    {
      fieldName: 'name',
      label: '密钥名称',
      component: 'Input',
      componentProps: {
        placeholder: '例如：研发部 Claude Code',
      },
      rules: z.string().min(1, { message: '请输入密钥名称' }).max(64),
    },
    {
      fieldName: 'userId',
      label: '绑定用户',
      component: 'ApiSelect',
      componentProps: {
        api: async () => {
          const { getSimpleUserList } = await import('#/api/system/user');
          const users = await getSimpleUserList();
          return users.map((user: any) => ({
            label: `${user.nickname}（${user.username}）`,
            value: user.id,
          }));
        },
        placeholder: '权限与租户隔离继承该用户',
      },
      rules: z.number({ message: '请选择绑定用户' }),
    },
    {
      fieldName: 'expiresTime',
      label: '过期时间',
      component: 'DatePicker',
      componentProps: {
        showTime: true,
        valueFormat: 'YYYY-MM-DD HH:mm:ss',
        placeholder: '留空表示永不过期',
      },
    },
  ];
}

/** 表格列 */
export function useGridColumns(): VxeTableGridOptions['columns'] {
  return [
    { type: 'seq', title: '序号', width: 50 },
    {
      field: 'name',
      title: '密钥名称',
      minWidth: 160,
    },
    {
      field: 'keyPrefix',
      title: '密钥前缀',
      minWidth: 130,
      formatter: ({ cellValue }) => `${cellValue}…`,
    },
    {
      field: 'nickname',
      title: '绑定用户',
      minWidth: 130,
      formatter: ({ row }) =>
        `${row.nickname ?? ''}（${row.username ?? row.userId}）`,
    },
    {
      field: 'enabled',
      title: '状态',
      width: 100,
      slots: { default: 'enabled' },
    },
    {
      field: 'lastUsedTime',
      title: '最后使用',
      minWidth: 160,
      formatter: ({ cellValue }) => cellValue ?? '从未使用',
    },
    {
      field: 'expiresTime',
      title: '过期时间',
      minWidth: 160,
      formatter: ({ cellValue }) => cellValue ?? '永不过期',
    },
    {
      field: 'createTime',
      title: '创建时间',
      minWidth: 160,
    },
    {
      field: 'action',
      title: '操作',
      width: 140,
      fixed: 'right',
      slots: { default: 'action' },
    },
  ];
}
