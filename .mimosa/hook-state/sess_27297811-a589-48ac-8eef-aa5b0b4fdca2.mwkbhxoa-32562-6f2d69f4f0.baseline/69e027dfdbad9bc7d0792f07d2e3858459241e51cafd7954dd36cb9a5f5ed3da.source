//! 对外 MCP 工具注册表：全部复用业务层查询函数。
//! 每个工具按绑定用户的权限码校验；数据按租户隔离。

use futures_util::future::BoxFuture;
use rustset_ai_server::AiState;
use rustset_framework_database::DbPool;
use rustset_framework_security::{CurrentUser, Permission};
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use rustset_infra_server::asset::repository as asset_repo;
use rustset_infra_server::asset::ASSET;
use rustset_infra_server::{QueryParams, risk, task, ticket};
use serde_json::{Value, json};

fn require(user: &CurrentUser, code: &str) -> Result<(), AppError> {
    if user.role_codes.iter().any(|role| role == "super_admin") {
        return Ok(());
    }
    let permission = Permission::new(code).map_err(|_| AppError::internal("invalid policy"))?;
    if user.can(&permission) {
        Ok(())
    } else {
        Err(AppError::forbidden(format!("permission denied: {code}")))
    }
}

fn page_args(args: &Value) -> Result<QueryParams, AppError> {
    let page_no = args.get("pageNo").and_then(Value::as_i64);
    let page_size = args.get("pageSize").and_then(Value::as_i64);
    serde_json::from_value(json!({
        "pageNo": page_no.unwrap_or(1),
        "pageSize": page_size.unwrap_or(10),
    }))
    .map_err(|_| AppError::bad_request("invalid pagination params"))
}

fn serialize(value: impl serde::Serialize) -> Result<Value, AppError> {
    serde_json::to_value(value).map_err(|_| AppError::internal("serialize failed"))
}

macro_rules! json_schema {
    ($($json:tt)+) => {
        || json!($($json)+)
    };
}

/// 工具执行上下文：复用网关已构建的状态。
pub struct ToolCtx<'a> {
    pub pool: &'a DbPool,
    pub ai: &'a AiState,
    pub user: &'a CurrentUser,
}

type Handler = for<'a> fn(&'a ToolCtx<'_>, &'a Value) -> BoxFuture<'a, Result<Value, AppError>>;

pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub permission: &'static str,
    schema: fn() -> Value,
    handler: Handler,
}

/// 注册表顺序即 tools/list 顺序。
pub static TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "asset_page",
        description: "分页查询资产台账（IP、主机名、业务归属等）。",
        permission: "infra:asset:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "pageNo": { "type": "integer", "description": "页码，默认 1" },
                "pageSize": { "type": "integer", "description": "每页条数 1-200，默认 10" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(
                    asset_repo::page_data(ctx.pool, ASSET, &tenant, page_args(args)?).await?,
                )
            })
        },
    },
    ToolDef {
        name: "asset_get",
        description: "按 id 获取资产台账详情。",
        permission: "infra:asset:query",
        schema: json_schema!({
            "type": "object",
            "properties": { "id": { "type": "integer", "description": "资产 id" } },
            "required": ["id"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let id = arg_id(args)?;
                let tenant = TenantContext::from_user(ctx.user)?;
                asset_repo::get(ctx.pool, ASSET, &tenant, id).await
            })
        },
    },
    ToolDef {
        name: "network_policy_page",
        description: "分页查询网络策略（含各策略未处置的高风险数）。",
        permission: "infra:network-policy:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(
                    rustset_infra_server::asset::policy_risk::page_data(
                        ctx.pool,
                        &tenant,
                        page_args(args)?,
                    )
                    .await?,
                )
            })
        },
    },
    ToolDef {
        name: "risk_page",
        description: "分页查询风险项（按发现时间倒序）。",
        permission: "infra:risk:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(risk::page_data(ctx.pool, &tenant, page_args(args)?).await?)
            })
        },
    },
    ToolDef {
        name: "scan_task_page",
        description: "分页查询扫描/巡检任务及其状态与进度。",
        permission: "infra:task:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let tenant = TenantContext::from_user(ctx.user)?;
                let size = args
                    .get("pageSize")
                    .and_then(Value::as_i64)
                    .unwrap_or(10)
                    .clamp(1, 200);
                let no = args.get("pageNo").and_then(Value::as_i64).unwrap_or(1).max(1);
                let offset = (no - 1).saturating_mul(size);
                let total = task::repository::count(ctx.pool, tenant.id())
                    .await
                    .map_err(|_| AppError::internal("failed to count tasks"))?;
                let list = task::repository::page(ctx.pool, tenant.id(), size, offset)
                    .await
                    .map_err(|_| AppError::internal("failed to list tasks"))?;
                Ok(json!({ "list": list, "total": total }))
            })
        },
    },
    ToolDef {
        name: "scan_task_get",
        description: "按 id 获取扫描任务详情。",
        permission: "infra:task:query",
        schema: json_schema!({
            "type": "object",
            "properties": { "id": { "type": "string", "description": "任务 id" } },
            "required": ["id"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let id = args
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| AppError::bad_request("id is required"))?;
                let tenant = TenantContext::from_user(ctx.user)?;
                task::repository::get(ctx.pool, tenant.id(), &id)
                    .await
                    .map_err(|_| AppError::internal("failed to read task"))?
                    .map(|found| json!({ "task": found }))
                    .ok_or_else(|| AppError::not_found("task not found"))
            })
        },
    },
    ToolDef {
        name: "ticket_page",
        description: "分页查询资源交付工单。",
        permission: "infra:resource-ticket:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(ticket::repository::page_data(ctx.pool, &tenant, page_args(args)?).await?)
            })
        },
    },
    ToolDef {
        name: "ticket_get",
        description: "按 id 获取资源工单详情。",
        permission: "infra:resource-ticket:query",
        schema: json_schema!({
            "type": "object",
            "properties": { "id": { "type": "integer", "description": "工单 id" } },
            "required": ["id"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let id = arg_id(args)?;
                let tenant = TenantContext::from_user(ctx.user)?;
                ticket::repository::get(ctx.pool, &tenant, id).await
            })
        },
    },
    ToolDef {
        name: "cmdb_model_page",
        description: "分页查询 CMDB 模型（支持名称/编码关键词），返回模型及其实例数、属性数。",
        permission: "cmdb:model:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "keyword": { "type": "string", "description": "模型名称/编码关键词" },
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                rustset_cmdb_server::page_models(
                    ctx.pool,
                    args.get("keyword").and_then(Value::as_str),
                    args.get("pageNo").and_then(Value::as_i64),
                    args.get("pageSize").and_then(Value::as_i64),
                )
                .await
            })
        },
    },
    ToolDef {
        name: "cmdb_instance_search",
        description: "在指定 CMDB 模型的实例中按关键词检索（匹配实例属性 JSON 文本）。",
        permission: "cmdb:instance:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "modelId": { "type": "integer", "description": "CMDB 模型 id" },
                "keyword": { "type": "string", "description": "检索关键词" },
                "pageNo": { "type": "integer" },
                "pageSize": { "type": "integer" }
            },
            "required": ["modelId"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let model_id = args
                    .get("modelId")
                    .and_then(Value::as_i64)
                    .ok_or_else(|| AppError::bad_request("modelId is required"))?;
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(
                    rustset_cmdb_server::instance_service::page_instances(
                        ctx.pool,
                        &tenant,
                        model_id,
                        args.get("keyword").and_then(Value::as_str),
                        args.get("pageNo").and_then(Value::as_i64),
                        args.get("pageSize").and_then(Value::as_i64),
                    )
                    .await?,
                )
            })
        },
    },
    ToolDef {
        name: "cmdb_instance_get",
        description: "按 id 获取 CMDB 实例详情（含动态属性）。",
        permission: "cmdb:instance:query",
        schema: json_schema!({
            "type": "object",
            "properties": { "id": { "type": "integer", "description": "实例 id" } },
            "required": ["id"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let id = arg_id(args)?;
                let tenant = TenantContext::from_user(ctx.user)?;
                serialize(
                    rustset_cmdb_server::instance_service::get_instance(
                        ctx.pool, &tenant, id,
                    )
                    .await?,
                )
            })
        },
    },
    ToolDef {
        name: "ai_model_list",
        description: "列出平台已启用的 AI 模型（不返回密钥等敏感字段）。",
        permission: "ai:model:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "type": {
                    "type": "string",
                    "description": "可选类型过滤：chat/image/speech/video/embedding/rerank/transcription/music"
                }
            }
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let models = rustset_ai_server::enabled_models(
                    ctx.pool,
                    args.get("type").and_then(Value::as_str),
                )
                .await?;
                // 只输出非敏感字段，绝不含 api_key/url/config。
                let list: Vec<Value> = models
                    .iter()
                    .map(|model| {
                        json!({
                            "id": model.id,
                            "name": model.name,
                            "platform": model.platform,
                            "type": model.type_,
                            "model": model.model,
                            "status": model.status,
                        })
                    })
                    .collect();
                Ok(json!({ "list": list }))
            })
        },
    },
    ToolDef {
        name: "ai_chat_send",
        description: "向平台 AI 助手发送消息并等待回复（非流式），使用平台已配置的模型与知识库；conversationId 省略或为 0 时自动创建新对话。",
        permission: "ai:chat:query",
        schema: json_schema!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "用户消息内容" },
                "conversationId": { "type": "integer", "description": "对话 id；0/省略则新建对话" },
                "useContext": { "type": "boolean", "description": "是否携带对话上下文，默认 true" }
            },
            "required": ["content"]
        }),
        handler: |ctx, args| {
            Box::pin(async move {
                let content = args
                    .get("content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|content| !content.trim().is_empty())
                    .ok_or_else(|| AppError::bad_request("content is required"))?;
                let conversation_id = args
                    .get("conversationId")
                    .and_then(Value::as_i64)
                    .filter(|id| *id > 0);
                let use_context = args.get("useContext").and_then(Value::as_bool);
                let request = match conversation_id {
                    Some(id) => rustset_ai_server::chat::SendRequest::with_conversation(
                        id, content, use_context,
                    ),
                    None => rustset_ai_server::chat::SendRequest::auto(content, use_context),
                };
                rustset_ai_server::chat::dispatch_send(ctx.ai, ctx.user, &request).await
            })
        },
    },
];

fn arg_id(args: &Value) -> Result<i64, AppError> {
    args.get("id")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::bad_request("id is required"))
}

pub fn find(name: &str) -> Option<&'static ToolDef> {
    TOOLS.iter().find(|tool| tool.name == name)
}

/// tools/list 输出（不含 handler 与权限码）。
pub fn descriptors() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": (tool.schema)(),
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// 权限校验 + 执行。
pub async fn dispatch(ctx: &ToolCtx<'_>, name: &str, args: &Value) -> Result<Value, AppError> {
    let tool =
        find(name).ok_or_else(|| AppError::bad_request(format!("unknown tool: {name}")))?;
    require(ctx.user, tool.permission)?;
    (tool.handler)(ctx, args).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptors_cover_all_tools_with_schema() {
        let list = descriptors();
        let tools = list["tools"].as_array().unwrap();
        assert_eq!(tools.len(), TOOLS.len());
        for (descriptor, def) in tools.iter().zip(TOOLS.iter()) {
            assert_eq!(descriptor["name"], def.name);
            assert_eq!(descriptor["inputSchema"]["type"], "object");
            assert!(!descriptor["description"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn tool_names_unique() {
        let mut names: Vec<&str> = TOOLS.iter().map(|tool| tool.name).collect();
        names.sort_unstable();
        let count = names.len();
        names.dedup();
        assert_eq!(names.len(), count);
    }
}
