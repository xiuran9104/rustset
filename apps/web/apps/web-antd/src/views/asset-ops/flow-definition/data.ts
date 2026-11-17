import type { VxeTableGridOptions } from '#/adapter/vxe-table';

/** 步骤类型元数据（编辑器与列表共用） */
export const STEP_TYPES = [
  { value: 'approval', label: '审批' },
  { value: 'manual', label: '人工执行' },
  { value: 'auto', label: '自动动作' },
  { value: 'notification', label: '通知' },
  { value: 'ticket_wait', label: '等待工单' },
] as const;

export const AUTO_ACTIONS = [
  { value: 'http_request', label: 'HTTP 请求（SSRF 防护）' },
  { value: 'start_flow', label: '启动子流程' },
  { value: 'firewall_push', label: '防火墙策略下发' },
  { value: 'trigger_scan', label: '触发扫描任务' },
  { value: 'create_ticket', label: '创建资源工单' },
  { value: 'set_context', label: '写流程上下文' },
  { value: 'ticket_approve', label: '工单审批' },
  { value: 'ticket_provision', label: '工单供应（Tofu）' },
  { value: 'ticket_deliver', label: '工单交付' },
] as const;

export const CONDITION_OPS = [
  { value: 'eq', label: '等于' },
  { value: 'ne', label: '不等于' },
  { value: 'gt', label: '大于' },
  { value: 'gte', label: '大于等于' },
  { value: 'lt', label: '小于' },
  { value: 'lte', label: '小于等于' },
  { value: 'in', label: '属于' },
  { value: 'exists', label: '存在' },
] as const;

export function stepTypeLabel(type: string): string {
  return STEP_TYPES.find((t) => t.value === type)?.label ?? type;
}

/** 表格列 */
export function useGridColumns(): VxeTableGridOptions['columns'] {
  return [
    { type: 'seq', title: '序号', width: 50 },
    { field: 'name', title: '流程名称', minWidth: 180 },
    { field: 'description', title: '说明', minWidth: 200, showOverflow: 'tooltip' },
    { field: 'stepCount', title: '步骤数', width: 80 },
    {
      field: 'status',
      title: '状态',
      width: 90,
      slots: { default: 'status' },
    },
    { field: 'version', title: '版本', width: 70 },
    { field: 'updateTime', title: '更新时间', width: 170 },
    {
      field: 'action',
      title: '操作',
      width: 160,
      fixed: 'right',
      slots: { default: 'action' },
    },
  ];
}
