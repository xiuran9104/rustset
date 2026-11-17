
## V3 新增

- **画布编辑器**：@vue-flow 可视化编排（节点拖拽/连线/属性面板/dagre 自动布局），
  与表格编辑器 tab 切换，同一数据格式。
- **HTTP 通用节点**（`http_request`）：SSRF 防护（scheme 校验 + IP 黑名单 +
  DNS 解析检查 + 禁 redirect）；响应截断 512 字符存入 step.result。
- **BPMN 导入**（`POST /infra/flow-definition/import-bpmn`）：BPMN 2.0 XML →
  流程定义（userTask→approval、serviceTask→auto、exclusiveGateway→路由、
  sequenceFlow→next 出口）；不支持子流程/并行网关，跳过并告警。
- **跨实例调用**（`start_flow`）：启动子流程实例，wait=true 时轮询等待终态
  （runner 30s 轮询）；wait=false fire-and-forget。
- **动态加签**（`POST .../add-approver`）：向 active 会签步骤追加审批人，
  合并 assignee 不覆盖已有 approvers。
