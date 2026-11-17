# 流程引擎 V3 — 画布编辑器 + HTTP 节点 + BPMN 导入 + 跨实例调用 + 动态加签

## Context

V2（`18341ab`）完成后推迟的五项特性。探索确认：`@vue-flow/core` 未安装（需新装）；reqwest 已在 infra-server（HTTP 节点零新依赖）；无 XML 解析依赖（BPMN 需加 `quick-xml`）；`start_instance` 是 `pub(crate)` 可从 actions 直接调用。

## A. 画布编辑器（前端）

**安装**：`@vue-flow/core` + `@vue-flow/background` + `@vue-flow/controls` + `@dagrejs/dagre`（自动布局）

**新文件** `views/asset-ops/flow-definition/modules/canvas-editor.vue`：
- Vue Flow 画布，步骤 = 节点（按 type 着色：approval=橙、auto=蓝、manual=绿、notification=灰、ticket_wait=紫）
- `next` 出口 = 边（带条件标签），连线创建出口、删线删出口
- dagre 自动布局（LR 方向）
- 双击节点打开右侧属性面板（key/name/type/assignee/condition/timeout/multi/params）
- 保存时序列化为现有 `{steps:[{key,name,type,config,condition,next}]}` 格式——引擎零改动
- 入口：定义编辑弹窗加"画布模式"切换 tab（表格式 ↔ 画布）

## B. HTTP 通用节点（后端）

**新自动动作** `http_request`（actions.rs）：
- params: `{method, url, headers?, body?, timeoutSeconds?}`
- **SSRF 防护**（安全约束）：
  - scheme 仅 http/https
  - 解析 host → 拒绝 loopback/private/link-local/reserved IP
  - redirect policy: none（禁跟随防绕过）
  - timeout 默认 30s，上限 120s
- 响应：status + 截断的 body（512 字符）存入 step.result
- 失败进现有退避重试（5/10/20s，3 次）

## C. BPMN 导入（后端 + 前端）

**依赖**：workspace 加 `quick-xml = "0.36"`

**新端点** `POST /infra/flow-definition/import-bpmn`（definition.rs）：
- 接受 BPMN 2.0 XML 字符串
- 解析映射：
  - `<process>` → 流程定义
  - `<userTask>` → approval 步骤
  - `<serviceTask>` → auto 步骤（默认 set_context，可后改）
  - `<exclusiveGateway>` + conditional `<sequenceFlow>` → DAG next 出口
  - `<endEvent>` → notification 终点
  - `<sequenceFlow>` 无条件 → 线性 next
- 跳过不支持的元素（子流程/并行网关等），日志告警
- 前端：定义页加"导入 BPMN"按钮 + 文件选择器

## D. 跨实例调用（后端）

**新自动动作** `start_flow`（actions.rs）：
- params: `{definitionId 或 definitionName, payload?, wait?}`
- `wait=false`（默认）：启动后即 done（fire-and-forget）
- `wait=true`：启动后步骤转 ticket_wait 模式轮询子实例状态（runner 已有轮询机制，扩展 `handle_ticket_waits` 检查 `biz_type='flow_child'` 的实例终态）
- 复用 `instance::start_instance`（pub(crate) 同 crate 直调）

## E. 动态加签（后端）

**新端点** `POST /infra/flow-instance/{id}/step/{stepKey}/add-approver`（instance.rs）：
- body: `{usernames: [...]}` 或 `{userIds: [...]}`
- 仅 active + multi=all 的 approval 步骤可加签
- 合并到 assignee（不覆盖已有），已有 approvers 保留
- 加签后重新检查是否已达全员通过（新加的人可能已 approve 过）

## 实现步骤

### 1. 后端：HTTP 节点 + SSRF（actions.rs）
### 2. 后端：跨实例调用 + 子实例等待（actions.rs + runner.rs）
### 3. 后端：动态加签（instance.rs）
### 4. 后端：BPMN 导入（definition.rs + Cargo.toml 加 quick-xml）
### 5. 前端：安装 @vue-flow + 画布编辑器
### 6. 前端：BPMN 导入按钮 + HTTP/跨实例动作的编辑器支持
### 7. 文档更新

## 验证

1. cargo test + 前端 type check + build
2. E2E：HTTP 节点（对本地 mock 端点发起请求，SSRF 拒绝 localhost）→ 跨实例（主流程 auto 步骤启动子流程并等待完成）→ 动态加签（中途追加审批人后全员通过）→ BPMN 导入（标准 XML → 定义创建成功）
3. 浏览器：画布编辑器渲染、节点拖拽、连线、属性面板、保存

## 明确不做
子流程嵌套（BPMN `<subProcess>`）、并行网关（`<parallelGateway>`）、定时器事件、补偿/回滚
