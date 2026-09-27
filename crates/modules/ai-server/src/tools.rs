use crate::{AiState, require};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

pub(crate) async fn load_definitions(
    pool: &sqlx::PgPool,
    ids: &[i64],
) -> Result<Vec<Value>, AppError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }
    let rows = sqlx::query(
        "SELECT name,description,input_schema FROM ai.tools WHERE id=ANY($1) AND status=1",
    )
    .bind(ids)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("读取对话工具失败"))?;
    Ok(rows.into_iter().map(|r|json!({"type":"function","function":{"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"parameters":r.get::<Value,_>("input_schema")}})).collect())
}
pub(crate) async fn execute(
    pool: &sqlx::PgPool,
    user: &CurrentUser,
    name: &str,
    args: &Value,
) -> Result<String, String> {
    let allowed: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ai.tools WHERE name=$1 AND status=1)")
            .bind(name)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
    if !allowed {
        return Err(format!("工具 {name} 未启用"));
    }
    match name {
        "current_time" => {
            let offset = args
                .get("utcOffset")
                .and_then(Value::as_str)
                .ok_or("utcOffset 不能为空")?;
            let seconds = parse_offset(offset)?;
            let zone = chrono::FixedOffset::east_opt(seconds).ok_or("UTC 偏移无效")?;
            Ok(chrono::Utc::now().with_timezone(&zone).to_rfc3339())
        }
        "weather_query" => {
            let location = args
                .get("location")
                .and_then(Value::as_str)
                .ok_or("location 不能为空")?;
            let response = reqwest::Client::new()
                .get("https://wttr.in/")
                .query(&[("format", "j1"), ("q", location)])
                .send()
                .await
                .map_err(|e| e.to_string())?;
            if !response.status().is_success() {
                return Err(format!("天气服务返回 {}", response.status()));
            }
            response.text().await.map_err(|e| e.to_string())
        }
        // ---- 运维 Agent 工具集（docs/cmdb-roadmap.md）----
        "cmdb_model_list" => {
            require(user, "cmdb:model:query").map_err(|error| error.message().to_owned())?;
            let tenant =
                TenantContext::from_user(user).map_err(|error| error.message().to_owned())?;
            let rows = sqlx::query(
                "SELECT m.id, m.name, m.code, m.description,
                        (SELECT count(*) FROM cmdb_attribute a WHERE a.model_id=m.id AND a.deleted=0) AS attrs,
                        (SELECT count(*) FROM cmdb_instance i WHERE i.model_id=m.id AND i.tenant_id=$1 AND i.deleted=0) AS instances
                 FROM cmdb_model m WHERE m.deleted=0 AND m.status=0 ORDER BY m.sort, m.id",
            )
            .bind(tenant.id())
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            let list: Vec<Value> = rows
                .iter()
                .map(|r| json!({"name": r.get::<String,_>("name"), "code": r.get::<String,_>("code"),
                                "description": r.get::<Option<String>,_>("description"),
                                "attributes": r.get::<i64,_>("attrs"), "instances": r.get::<i64,_>("instances")}))
                .collect();
            Ok(serde_json::to_string(&json!({"models": list})).unwrap())
        }
        "cmdb_instance_query" => {
            require(user, "cmdb:instance:query").map_err(|error| error.message().to_owned())?;
            let tenant =
                TenantContext::from_user(user).map_err(|error| error.message().to_owned())?;
            let model_code = args
                .get("model_code")
                .and_then(Value::as_str)
                .ok_or("model_code 不能为空")?;
            let keyword = args.get("keyword").and_then(Value::as_str).unwrap_or("");
            let limit = args
                .get("limit")
                .and_then(Value::as_i64)
                .unwrap_or(10)
                .clamp(1, 20) as i64;
            let model_id: Option<i64> =
                sqlx::query_scalar("SELECT id FROM cmdb_model WHERE code=$1 AND deleted=0")
                    .bind(model_code)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| e.to_string())?;
            let Some(model_id) = model_id else {
                return Ok(format!("模型 {model_code:?} 不存在"));
            };
            let rows = sqlx::query(
                "SELECT attributes, update_time FROM cmdb_instance
                 WHERE model_id=$1 AND tenant_id=$4 AND deleted=0
                   AND ($2::text = '' OR attributes::text ILIKE '%'||$2||'%')
                 ORDER BY id DESC LIMIT $3",
            )
            .bind(model_id)
            .bind(keyword)
            .bind(limit)
            .bind(tenant.id())
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            let list: Vec<Value> = rows
                .iter()
                .map(|r| json!({"attributes": r.get::<Value,_>("attributes"), "updateTime": r.get::<chrono::NaiveDateTime,_>("update_time").to_string()}))
                .collect();
            Ok(serde_json::to_string(&json!({"instances": list})).unwrap())
        }
        "asset_query" => {
            require(user, "infra:asset:query").map_err(|error| error.message().to_owned())?;
            let tenant =
                TenantContext::from_user(user).map_err(|error| error.message().to_owned())?;
            let keyword = args.get("keyword").and_then(Value::as_str).unwrap_or("");
            let limit = args
                .get("limit")
                .and_then(Value::as_i64)
                .unwrap_or(10)
                .clamp(1, 20) as i64;
            let rows = sqlx::query(
                "SELECT name, ip, zone, device_type, os, owner, organization_name,
                        application_name, classified_protection_level, status
                 FROM infra_asset
                 WHERE tenant_id=$3 AND deleted=0 AND ($1::text = '' OR name ILIKE '%'||$1||'%' OR ip ILIKE '%'||$1||'%'
                                      OR coalesce(organization_name,'') ILIKE '%'||$1||'%'
                                      OR coalesce(application_name,'') ILIKE '%'||$1||'%')
                 ORDER BY id DESC LIMIT $2",
            )
            .bind(keyword)
            .bind(limit)
            .bind(tenant.id())
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            let list: Vec<Value> = rows
                .iter()
                .map(|r| json!({"name": r.get::<String,_>("name"), "ip": r.get::<String,_>("ip"),
                                "zone": r.get::<String,_>("zone"),
                                "deviceType": r.get::<Option<String>,_>("device_type"),
                                "os": r.get::<Option<String>,_>("os"),
                                "owner": r.get::<Option<String>,_>("owner"),
                                "organization": r.get::<Option<String>,_>("organization_name"),
                                "application": r.get::<Option<String>,_>("application_name"),
                                "mlpsLevel": r.get::<Option<String>,_>("classified_protection_level"),
                                "status": r.get::<i16,_>("status")}))
                .collect();
            Ok(serde_json::to_string(&json!({"assets": list})).unwrap())
        }
        "ticket_query" => {
            require(user, "infra:resource-ticket:query")
                .map_err(|error| error.message().to_owned())?;
            let tenant =
                TenantContext::from_user(user).map_err(|error| error.message().to_owned())?;
            let status = args.get("status").and_then(Value::as_str).unwrap_or("");
            let keyword = args.get("keyword").and_then(Value::as_str).unwrap_or("");
            let limit = args
                .get("limit")
                .and_then(Value::as_i64)
                .unwrap_or(10)
                .clamp(1, 20) as i64;
            let rows = sqlx::query(
                "SELECT id, resource_type, ecs_name, ecs_type, cpu_cores, memory_gb, resource_count,
                        cloud_platform_name, cloud_region, ticket_status, apply_status,
                        applicant_name, create_time
                 FROM infra_resource_ticket
                 WHERE tenant_id=$4 AND deleted=0
                   AND ($1::text = '' OR ticket_status=$1)
                   AND ($2::text = '' OR coalesce(ecs_name,'') ILIKE '%'||$2||'%'
                        OR coalesce(application_name,'') ILIKE '%'||$2||'%')
                 ORDER BY id DESC LIMIT $3",
            )
            .bind(status)
            .bind(keyword)
            .bind(limit)
            .bind(tenant.id())
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;
            let list: Vec<Value> = rows
                .iter()
                .map(|r| json!({"id": r.get::<i64,_>("id"), "resourceType": r.get::<String,_>("resource_type"),
                                "ecsName": r.get::<String,_>("ecs_name"),
                                "ecsType": r.get::<Option<String>,_>("ecs_type"),
                                "cpuCores": r.get::<Option<i32>,_>("cpu_cores"),
                                "memoryGb": r.get::<Option<i32>,_>("memory_gb"),
                                "count": r.get::<Option<i32>,_>("resource_count"),
                                "cloudPlatform": r.get::<Option<String>,_>("cloud_platform_name"),
                                "region": r.get::<Option<String>,_>("cloud_region"),
                                "ticketStatus": r.get::<String,_>("ticket_status"),
                                "applyStatus": r.get::<Option<String>,_>("apply_status"),
                                "applicant": r.get::<Option<String>,_>("applicant_name"),
                                "createTime": r.get::<Option<chrono::NaiveDateTime>,_>("create_time").map(|t| t.to_string())}))
                .collect();
            Ok(serde_json::to_string(&json!({"tickets": list})).unwrap())
        }
        "ticket_create" => {
            require(user, "infra:resource-ticket:create")
                .map_err(|error| error.message().to_owned())?;
            let tenant =
                TenantContext::from_user(user).map_err(|error| error.message().to_owned())?;
            let ecs_name = args
                .get("ecs_name")
                .and_then(Value::as_str)
                .ok_or("ecs_name 不能为空")?;
            let cpu = args.get("cpu_cores").and_then(Value::as_i64).unwrap_or(4);
            let memory = args.get("memory_gb").and_then(Value::as_i64).unwrap_or(8);
            let count = args
                .get("resource_count")
                .and_then(Value::as_i64)
                .unwrap_or(1);
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO infra_resource_ticket
                     (resource_type, ecs_name, ecs_type, ecs_os, cloud_category, cloud_region,
                      resource_count, cpu_cores, memory_gb, application_name,
                      ticket_status, ticket_type, approval_stage, approval_total,
                      current_approval_role, delivery_status, created_by, applicant_name,
                      create_time, update_time, tenant_id)
                 VALUES ('ecs', $1, $2, $3, $4, $5, $6, $7, $8, $9,
                         'pending_approval', 'create', 1, 1, '资源管理员', '未交付', $10, $10,
                         date_trunc('second', now()), date_trunc('second', now()), $11)
                 RETURNING id",
            )
            .bind(ecs_name)
            .bind(args.get("ecs_type").and_then(Value::as_str))
            .bind(args.get("ecs_os").and_then(Value::as_str))
            .bind(args.get("cloud_category").and_then(Value::as_str))
            .bind(args.get("cloud_region").and_then(Value::as_str))
            .bind(count as i32)
            .bind(cpu as i32)
            .bind(memory as i32)
            .bind(args.get("application_name").and_then(Value::as_str))
            .bind(&user.username)
            .bind(tenant.id())
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
            let rule: Option<(String, bool)> = sqlx::query_as(
                "SELECT name, auto_provision FROM infra_approval_rule
                 WHERE deleted=0 AND status=0
                   AND (resource_type='' OR resource_type='ecs')
                   AND (max_cpu_cores IS NULL OR max_cpu_cores >= $1)
                   AND (max_memory_gb IS NULL OR max_memory_gb >= $2)
                   AND (max_resource_count IS NULL OR max_resource_count >= $3)
                 ORDER BY id LIMIT 1",
            )
            .bind(cpu as i32)
            .bind(memory as i32)
            .bind(count as i32)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            match rule {
                Some((rule_name, _auto)) => {
                    sqlx::query(
                        "UPDATE infra_resource_ticket SET ticket_status='pending_provision',
                                approver=$2, approve_comment=$3, update_time=now()
                         WHERE id=$1 AND tenant_id=$4",
                    )
                    .bind(id)
                    .bind(format!("auto:{rule_name}"))
                    .bind(format!("Agent 建单命中自动审批规则 {rule_name}"))
                    .bind(tenant.id())
                    .execute(pool)
                    .await
                    .map_err(|e| e.to_string())?;
                    Ok(format!(
                        "{{\"ticketId\": {id}, \"status\": \"pending_provision\", \"autoApprovedBy\": \"{rule_name}\"}}"
                    ))
                }
                None => Ok(format!(
                    "{{\"ticketId\": {id}, \"status\": \"pending_approval\"}}"
                )),
            }
        }
        _ => Err(format!("工具 {name} 没有已注册的 Rust 执行器")),
    }
}
fn parse_offset(value: &str) -> Result<i32, String> {
    let sign = if value.starts_with('-') { -1 } else { 1 };
    let clean = value.trim_start_matches(['+', '-']);
    let (h, m) = clean.split_once(':').ok_or("UTC 偏移格式应为 +08:00")?;
    let hours: i32 = h.parse().map_err(|_| "UTC 小时无效")?;
    let minutes: i32 = m.parse().map_err(|_| "UTC 分钟无效")?;
    if hours > 23 || minutes > 59 {
        return Err("UTC 偏移超出范围".into());
    }
    Ok(sign * (hours * 3600 + minutes * 60))
}

#[cfg(test)]
mod tests {
    use super::parse_offset;
    #[test]
    fn parses_utc_offsets() {
        assert_eq!(parse_offset("+08:00").unwrap(), 28_800);
        assert_eq!(parse_offset("-05:30").unwrap(), -19_800);
        assert!(parse_offset("25:00").is_err());
    }
}

pub fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/tool/page", get(page))
        .api_route("/ai/tool/simple-list", get(simple_list))
        .api_route("/ai/tool/get", get(get_one))
        .api_route("/ai/tool/create", post(create))
        .api_route("/ai/tool/update", put(update))
        .api_route("/ai/tool/delete", delete(remove))
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct PageQuery {
    page_no: Option<i64>,
    page_size: Option<i64>,
    name: Option<String>,
    status: Option<i32>,
}
#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Save {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    status: Option<i32>,
    input_schema: Option<Value>,
    executor: Option<Value>,
}
fn value(r: sqlx::postgres::PgRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"name":r.get::<String,_>("name"),"description":r.get::<String,_>("description"),"status":r.get::<i32,_>("status"),"inputSchema":r.get::<Value,_>("input_schema"),"executor":r.get::<Value,_>("executor"),"createTime":r.get::<i64,_>("create_time")})
}
async fn page(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<PageQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&u, "ai:tool:query")?;
    let p = q.page_no.unwrap_or(1).max(1);
    let n = q.page_size.unwrap_or(10).clamp(1, 200);
    let rows=sqlx::query("SELECT * FROM ai.tools WHERE ($1::text IS NULL OR name ILIKE '%'||$1||'%') AND ($2::int IS NULL OR status=$2) ORDER BY id DESC LIMIT $3 OFFSET $4").bind(&q.name).bind(q.status).bind(n).bind((p-1)*n).fetch_all(&s.pool).await.map_err(|_|AppError::internal("读取工具失败"))?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM ai.tools WHERE ($1::text IS NULL OR name ILIKE '%'||$1||'%') AND ($2::int IS NULL OR status=$2)").bind(&q.name).bind(q.status).fetch_one(&s.pool).await.map_err(|_|AppError::internal("读取工具失败"))?;
    Ok(Json(ApiResponse::new(
        json!({"list":rows.into_iter().map(value).collect::<Vec<_>>(),"total":total}),
    )))
}
async fn simple_list(
    u: CurrentUser,
    State(s): State<AiState>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    require(&u, "ai:tool:query")?;
    let rows = sqlx::query("SELECT * FROM ai.tools WHERE status=1 ORDER BY name")
        .fetch_all(&s.pool)
        .await
        .map_err(|_| AppError::internal("读取工具失败"))?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(value).collect(),
    )))
}
async fn get_one(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&u, "ai:tool:query")?;
    let r = sqlx::query("SELECT * FROM ai.tools WHERE id=$1")
        .bind(q.id)
        .fetch_optional(&s.pool)
        .await
        .map_err(|_| AppError::internal("读取工具失败"))?
        .ok_or_else(|| AppError::not_found("工具不存在"))?;
    Ok(Json(ApiResponse::new(value(r))))
}
fn validate(v: &Save) -> Result<(), AppError> {
    if v.name.is_empty()
        || !v
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Err(AppError::bad_request("工具名只能包含字母、数字和下划线"))
    } else {
        Ok(())
    }
}
async fn create(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Save>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&u, "ai:tool:create")?;
    validate(&v)?;
    let id = chrono::Utc::now().timestamp_micros();
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query("INSERT INTO ai.tools(id,name,description,status,input_schema,executor,create_time,update_time)VALUES($1,$2,$3,$4,$5,$6,$7,$7)").bind(id).bind(v.name).bind(v.description.unwrap_or_default()).bind(v.status.unwrap_or(1)).bind(v.input_schema.unwrap_or_else(||json!({"type":"object","properties":{}}))).bind(v.executor.unwrap_or_else(||json!({"kind":"builtin"}))).bind(now).execute(&s.pool).await.map_err(|e|AppError::bad_request(format!("创建工具失败: {e}")))?;
    Ok(Json(ApiResponse::new(id)))
}
async fn update(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Save>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:tool:update")?;
    validate(&v)?;
    let id =
        v.id.ok_or_else(|| AppError::bad_request("id is required"))?;
    let r=sqlx::query("UPDATE ai.tools SET name=$2,description=$3,status=$4,input_schema=COALESCE($5,input_schema),executor=COALESCE($6,executor),update_time=$7 WHERE id=$1").bind(id).bind(v.name).bind(v.description.unwrap_or_default()).bind(v.status.unwrap_or(1)).bind(v.input_schema).bind(v.executor).bind(chrono::Utc::now().timestamp_millis()).execute(&s.pool).await.map_err(|_|AppError::internal("更新工具失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
async fn remove(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:tool:delete")?;
    let used: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM ai.chat_conversations WHERE $1=ANY(tool_ids))",
    )
    .bind(q.id)
    .fetch_one(&s.pool)
    .await
    .map_err(|_| AppError::internal("检查工具引用失败"))?;
    if used {
        return Err(AppError::bad_request("工具正在被对话使用"));
    }
    let r = sqlx::query("DELETE FROM ai.tools WHERE id=$1")
        .bind(q.id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("删除工具失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}

#[cfg(test)]
mod tenant_tests;
