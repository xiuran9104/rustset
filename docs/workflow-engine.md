# 产品内流程引擎

自研轻量编排：管理员定义流程（顺序/DAG 步骤 + 条件 + 会签），实例化后由引擎推进，
解决"多级审批 / 条件分支 / 自动动作 / 通知 / 等待工单"的端到端编排。
**流程即审批权威**（0068 起资源工单链原生化）：申请中心与工单创建 API 均原生
启动流程实例，工单表退化为资源台账 + 供应/交付/到期操作记录。

## 资源工单链原生化（迁移 0068）

- 影子机制退役：`FLOW_TICKET_MODE` 与「资源交付流程（标准）」（0063）停用。
- **前端路径**：资源工单页创建弹窗 → 原生流程发起（create 类→「云资源申请
  流程」，变更类→「资源变更申请流程」）；网络策略工单页 →「网络策略申请
  流程」（0067）。审批在 流程实例 / 我的待办 处理。
- **API/MCP 路径**：工单创建 API 未命中自动审批规则时，自动启动「资源工单
  审批流程（台账触发）」（payload=工单 JSON，biz 绑定，幂等；无 create_ticket/
  交付步骤——供应交付仍在工单页操作）。
- 工单分页附带 `flowInstanceId`/`flowStatus`；运行中的工单在前端显示
  「流程审批中」标签（存量无流程绑定的工单保留旧审批按钮）。

## V2 新增（迁移 0063）

- **DAG 分支跳转**：步骤可声明 `next: [{"when":{...},"to":"key"},...]` 多出口；
  完成后按顺序评估第一个命中选路，非命中分支的步骤自动置 skipped。
- **会签**：`assignee.multi="all"` 时 N 人全部通过才 done（默认 any=首人通过）。
- 新自动动作：`ticket_approve`/`ticket_provision`/`ticket_deliver`
  复用工单内核（approve_core/deliver_core）。
- **事件触发**：任务完成→名含「扫描完成」的启用定义→自动启动实例；
  风险新增→名含「风险新增」→自动启动（inspection_key 幂等）。
- **前端**：实例页支持"从工单发起"（选工单自动填 payload）、
  步骤编辑器支持出口/会签、实例详情显示会签进度。

## 变量插值与动态审批（0066/0067 特性）

- **Value 级插值**（`interpolate_value`）：auto 节点 params 与通知节点 params
  的所有 string 叶做 `{{payload.x}}`/`{{context.x}}`/`{{steps.key.output.x}}`
  替换；整串单变量引用保留原始 JSON 类型（数字/布尔/对象）。
- **动态审批人**：审批/人工节点 `config.fromContext` 指向 context 内用户名
  （点路径），激活时并入步骤 assignee（空/未命中回退 assignee 角色）。
- **通知模板参数**：通知节点 `config.params` 插值后按 `{key}` 替换模板占位
  符（自定义键与 flowName/stepName/title 并存）。

## 模型

- **流程定义**（`infra_flow_definition`）：`{"steps":[...]}`，保存时
  `version+1`；实例化时快照进 `infra_flow_instance.definition_snapshot`。
- **顺序步骤 + 条件跳过**：按 index 依次执行；每个步骤可带
  `condition: {field, op, value}`（对 payload 求值，先 payload 后 context，
  支持 eq/ne/gt/gte/lt/lte/in/exists/not_exists，数字/字符串宽容比较），
  为假 → `skipped`。表结构已预留 `next_steps`，v2 可升级 DAG。
- **并发控制**：实例级 advisory lock（`rustset:flow:{id}`），API 动作与
  runner 推进互斥。

## 步骤类型

| 类型 | 语义 | config |
| --- | --- | --- |
| `approval` | 审批（多级=多个审批步骤连排） | `assignee`（role/userIds/usernames 任一或并用）、`timeoutHours`、`onTimeout`（escalate_notify=升级通知后不再重复 / auto_reject=超时自动拒绝并失败收尾） |
| `manual` | 人工执行确认 | 同 approval（无 onTimeout 语义则仅提醒） |
| `auto` | 自动动作 | `action` + `params`；失败退避重试（5/10/20s，3 次后 failed），可人工重试 |
| `notification` | 站内信 | `assignee`、`templateCode`（默认 flow_step_todo） |
| `ticket_wait` | 等待工单终态 | `ticketId`（或运行时从 context/biz_id 解析，create_ticket 会自动落关联）；30s 轮询 |

**自动动作 v1**：`firewall_push`（payload 带 fw* 字段；走自动化开关/熔断/
审计，action=flow_auto_push；开关关闭=跳过不失败，熔断=重试）、`trigger_scan`
（params: name/target/ports/taskKind/policy，复用任务队列幂等键 flow:{id}）、
`create_ticket`（params=工单字段，自动关联实例 biz）、`set_context`
（params: key + value/fromPayloadField，写实例 context 供后续条件引用）。
**`check_risk_port`**（高危端口检测前置，params: port/sourcePort/protocol——
支持插值与 `443`/`8000-8010`/`80,443` 端口描述；与
`infra_high_risk_port_rule` 启用规则求区间交集，协议 any 通配，结果写
context：`risk_port_hit`/`risk_level`/`risk_port_rules`/`risk_ports`；
网络策略申请流程用作主管审批前的检测与会签触发条件）。

## 推进

- **API 动作**：审批通过/拒绝、人工完成 → `engine::advance` 就地推进
  （跳过条件、执行 auto/notification，直到等待外部输入）。
- **后台 runner**（5s 轮询）：审批/人工步骤超时处理、auto 重试到期重执行、
  ticket_wait 轮询工单状态。
- 通知：激活审批/人工步骤与超时升级直写 `system_notify_message`
  （模板 `flow_step_todo` / `flow_step_timeout`，迁移 0062 种子）。

## REST（权限码经 authorization.rs 强制）

- 定义：`/infra/flow-definition/page|get|create|update|delete`
  （`infra:flow-definition:query|create|update|delete`；删除被运行中实例阻止）
- 实例：`/infra/flow-instance/page|get|todo|create|cancel`
  （query/start/operate）与
  `/infra/flow-instance/{id}/step/{stepKey}/approve|reject|complete|retry`
  （operate；操作人必须命中 assignee，拒绝必填原因）
- `todo` 返回当前用户（角色/用户 id/用户名）命中的 active 审批与人工步骤。

## 菜单与示例

迁移 0062：运营流程 → **流程定义 / 流程实例**（30362/30363 + 按钮权限，
超管默认可见）；种子模板与停用的示例定义"网络开通标准流程（示例）"
（安全审批 → 高风险才走的主管复审 → 防火墙下发 → 通知 → 人工确认交付）。

## 验证要点（E2E 已跑通）

定义创建 → 启动实例（payload: riskLevel=high）→ 待办命中一级审批 → 通过 →
条件步骤按 riskLevel 激活/跳过 → auto（firewall dry_run 落审计）→ 通知 →
人工确认 → completed；负例：拒绝→failed、取消、超时 auto_reject、重试。

## v1 不做

画布编辑器、DAG 跳转、会签（多人均需通过）、HTTP 通用节点、BPMN 导入、
现有工单审批链迁移（v2 吸收）。
