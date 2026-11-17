# RustSet AI 应用平台 — 融合 Windmill + Dify 设计方案

## 核心理念

从"工单自动化工具"演进为"AI 应用开发平台"。抄 Windmill 的**数据模型**（步骤=函数、schema=表单、变量系统）和 Dify 的**AI 原生节点**（LLM/知识/Agent 一等公民），用 Rust + Vue 实现。

## 一、数据模型（Windmill 风格）

### 1.1 步骤 = 函数签名

每个节点声明明确的 input/output，引擎自动追踪数据流：

```json
{
  "key": "risk_analysis",
  "name": "风险分析",
  "type": "llm",
  "config": {
    "modelId": 1,
    "systemPrompt": "你是安全分析师",
    "userPrompt": "分析 {{payload.ecsName}}",
    "outputFormat": "json"
  },
  "outputSchema": {
    "type": "object",
    "properties": {
      "level": { "type": "string" },
      "reason": { "type": "string" }
    }
  }
}
```

### 1.2 变量系统（`steps.{key}.output` 全局引用）

```
上游输出自动挂载：steps.risk_analysis.output.level
实例输入：       payload.resourceType
系统变量：       flow.id, flow.startTime, tenant.id
```

### 1.3 节点注册表（后端返回 schema，前端自动渲染）

```json
GET /infra/flow-node-types
[
  {
    "type": "llm",
    "name": "AI 处理",
    "icon": "sparkles",
    "color": "#6a5acd",
    "fields": [
      { "key": "modelId", "label": "模型", "widget": "model_select" },
      { "key": "systemPrompt", "label": "系统提示词", "widget": "textarea" },
      { "key": "userPrompt", "label": "用户提示词", "widget": "textarea_with_vars" },
      { "key": "temperature", "label": "温度", "widget": "slider", "min": 0, "max": 2, "step": 0.1, "default": 0.7 },
      { "key": "outputFormat", "label": "输出格式", "widget": "select", "options": ["text","json"] },
      { "key": "outputKey", "label": "输出变量名", "widget": "text", "default": "llm_output" }
    ]
  },
  {
    "type": "knowledge",
    "name": "知识检索",
    "icon": "book",
    "color": "#20b2aa",
    "fields": [
      { "key": "knowledgeIds", "label": "知识库", "widget": "knowledge_multiselect" },
      { "key": "query", "label": "检索内容", "widget": "textarea_with_vars" },
      { "key": "topK", "label": "Top K", "widget": "number", "default": 5 },
      { "key": "outputKey", "label": "输出变量名", "widget": "text", "default": "kb_context" }
    ]
  },
  {
    "type": "agent",
    "name": "AI Agent",
    "icon": "bot",
    "color": "#ff1493",
    "fields": [
      { "key": "modelId", "label": "模型", "widget": "model_select" },
      { "key": "systemPrompt", "label": "系统提示词", "widget": "textarea" },
      { "key": "userPrompt", "label": "用户提示词", "widget": "textarea_with_vars" },
      { "key": "toolIds", "label": "可用工具", "widget": "tool_multiselect" },
      { "key": "maxIterations", "label": "最大迭代", "widget": "number", "default": 5 },
      { "key": "outputKey", "label": "输出变量名", "widget": "text", "default": "agent_result" }
    ]
  }
]
```

## 二、三种 AI 节点

### 2.1 LLM 节点（`type: "llm"`）

```
输入：系统提示词 + 用户提示词（支持 {{变量}}）
执行：AiModelFactory.chat(model_id, messages, temperature, max_tokens)
输出：AI 回复文本 → JSON 提取（可选）→ context.{outputKey}
```

### 2.2 知识检索节点（`type: "knowledge"`）

```
输入：知识库 ID 列表 + 检索 query（支持 {{变量}}）
执行：ai-server knowledge::retrieve(knowledge_ids, interpolated_query)
输出：检索结果格式化文本 → context.{outputKey}
```

### 2.3 Agent 节点（`type: "agent"`）

```
输入：模型 + 提示词 + 绑定的工具列表
执行：agentic loop（chat_tools → tools::execute → 回填，最多 N 轮）
输出：AI 最终回复 → context.{outputKey}
```

## 三、变量插值引擎

```rust
/// {{payload.field}} → instance.payload.field
/// {{context.field}} → instance.context.field（含上游节点输出）
/// {{steps.key.output.field}} → 上游步骤输出
/// 未命中保留原文，不报错
pub fn interpolate(template: &str, payload: &Value, context: &Value) -> String
```

支持嵌套路径：`{{steps.risk_analysis.output.level}}`、`{{payload.config.cpuCores}}`

## 四、实现文件清单

### 后端

| 文件 | 改动 |
|---|---|
| `ai-server/src/lib.rs` | `pub mod knowledge/tools` |
| `ai-server/src/knowledge.rs` | `retrieve` → `pub` |
| `ai-server/src/tools.rs` | `load_definitions`/`execute` → `pub` |
| `infra-server/Cargo.toml` | 加 `rustset-ai-server` 依赖 |
| `infra-server/src/workflow/variables.rs` | 新建：interpolate + extract_json + 单测 |
| `infra-server/src/workflow/ai_nodes.rs` | 新建：execute_llm / execute_knowledge / execute_agent |
| `infra-server/src/workflow/engine.rs` | valid_types 加 llm/knowledge/agent；advance dispatch |
| `infra-server/src/workflow/actions.rs` | execute_step_inner 加三个分支 |
| `infra-server/src/workflow/definition.rs` | 新增 GET /infra/flow-node-types 端点 |

### 前端

| 文件 | 改动 |
|---|---|
| `views/asset-ops/flow-definition/data.ts` | STEP_TYPES 加 llm/knowledge/agent + 颜色 |
| `views/asset-ops/flow-definition/modules/canvas-editor.vue` | 属性面板按 widget 自动渲染 |
| `api/asset-ops/flow.ts` | 加 getFlowNodeTypes / getAIModels / getKnowledgeBases / getAITools |

## 五、验证场景

```
工单创建 → [知识检索] 查安全策略 → [LLM] 风险分析（输出 level）→ 条件路由
  ├─ level=high → [审批] 人工审批
  ├─ level=medium → [Agent] AI 自动处理
  └─ level=low → [auto] ticket_approve + ticket_deliver（全自动）
```
