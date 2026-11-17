//! 流程定义 CRUD：definition jsonb 校验（engine::validate_definition）、
//! 保存 version+1；删除前检查无 running 实例。

use aide::axum::ApiRouter;
use aide::axum::routing::{get, post, put};
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

use crate::{InfraState, Page, QueryParams};

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/flow-definition/import-bpmn", post(import_bpmn))
        .api_route("/infra/flow-definition/page", get(page))
        .api_route("/infra/flow-definition/get", get(get_one))
        .api_route("/infra/flow-definition/create", post(create))
        .api_route("/infra/flow-definition/update", put(update))
        .api_route("/infra/flow-definition/delete", get(delete))
}

#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct SaveRequest {
    id: Option<i64>,
    name: String,
    description: Option<String>,
    /// 0=启用 1=停用
    status: Option<i16>,
    /// {"steps":[{key,name,type,config,condition}]}
    definition: Value,
}

fn row_value(row: &sqlx::postgres::PgRow) -> Value {
    json!({
        "id": row.get::<i64, _>("id"),
        "name": row.get::<String, _>("name"),
        "description": row.get::<String, _>("description"),
        "status": row.get::<i16, _>("status"),
        "definition": row.get::<Value, _>("definition"),
        "version": row.get::<i32, _>("version"),
        "stepCount": row.get::<Value, _>("definition")
            .get("steps").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
        "creator": row.get::<String, _>("creator"),
        "updater": row.get::<String, _>("updater"),
        "createTime": row.get::<chrono::NaiveDateTime, _>("create_time").to_string(),
        "updateTime": row.get::<chrono::NaiveDateTime, _>("update_time").to_string(),
    })
}

async fn page(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<QueryParams>,
) -> Result<Json<ApiResponse<Page<Value>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let pn = params.page_no.unwrap_or(1).max(1);
    let ps = params.page_size.unwrap_or(10).clamp(1, 200);
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_flow_definition WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to count flow definitions"))?;
    let rows = sqlx::query(
        "SELECT id, name, description, status, definition, version, creator, updater, create_time, update_time
         FROM infra_flow_definition
         WHERE tenant_id=$1 AND deleted=0
         ORDER BY id DESC LIMIT $2 OFFSET $3",
    )
    .bind(tenant.id())
    .bind(ps)
    .bind((pn - 1) * ps)
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to list flow definitions"))?;
    let list: Vec<Value> = rows.iter().map(row_value).collect();
    Ok(Json(ApiResponse::new(Page { list, total })))
}

async fn get_one(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let row = sqlx::query(
        "SELECT id, name, description, status, definition, version, creator, updater, create_time, update_time
         FROM infra_flow_definition WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to read flow definition"))?
    .ok_or_else(|| AppError::not_found("flow definition not found"))?;
    Ok(Json(ApiResponse::new(row_value(&row))))
}

async fn create(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("名称必填且不超过 128 字符"));
    }
    super::engine::validate_definition(&request.definition)
        .map_err(AppError::bad_request)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO infra_flow_definition (name, description, status, definition, version, creator, updater, tenant_id)
         VALUES ($1, $2, $3, $4, 1, $5, $5, $6) RETURNING id",
    )
    .bind(name)
    .bind(request.description.unwrap_or_default())
    .bind(request.status.unwrap_or(0))
    .bind(&request.definition)
    .bind(&user.username)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::bad_request(format!("创建流程定义失败: {e}")))?;
    Ok(Json(ApiResponse::new(id)))
}

async fn update(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<SaveRequest>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let id = request.id.ok_or_else(|| AppError::bad_request("id is required"))?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("名称必填且不超过 128 字符"));
    }
    super::engine::validate_definition(&request.definition)
        .map_err(AppError::bad_request)?;
    let result = sqlx::query(
        "UPDATE infra_flow_definition
         SET name=$2, description=$3, status=$4, definition=$5,
             version=version+1, updater=$6, update_time=now()
         WHERE id=$1 AND tenant_id=$7 AND deleted=0",
    )
    .bind(id)
    .bind(name)
    .bind(request.description.unwrap_or_default())
    .bind(request.status.unwrap_or(0))
    .bind(&request.definition)
    .bind(&user.username)
    .bind(tenant.id())
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to update flow definition"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("flow definition not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

async fn delete(
    user: CurrentUser,
    State(state): State<InfraState>,
    Query(params): Query<Id>,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let running: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_flow_instance
         WHERE definition_id=$1 AND tenant_id=$2 AND status='running' AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to check running instances"))?;
    if running > 0 {
        return Err(AppError::bad_request("该定义存在运行中的实例，不能删除"));
    }
    let result = sqlx::query(
        "UPDATE infra_flow_definition SET deleted=1, updater=$3, update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(params.id)
    .bind(tenant.id())
    .bind(&user.username)
    .execute(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to delete flow definition"))?;
    if result.rows_affected() == 0 {
        return Err(AppError::not_found("flow definition not found"));
    }
    Ok(Json(ApiResponse::new(())))
}

// ==================== V3: BPMN 导入 ====================

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ImportBpmnRequest {
    /// BPMN 2.0 XML 字符串
    xml: String,
    /// 导入后的定义名称（默认取 BPMN process name）
    name: Option<String>,
}

/// BPMN 元素中间表示
#[derive(Debug, Clone)]
struct BpmnElement {
    id: String,
    name: String,
    kind: String,
}

#[derive(Debug, Clone)]
struct BpmnFlow {
    source: String,
    target: String,
}

/// 解析 BPMN 2.0 XML → 流程元素 + 连接。
fn parse_bpmn(xml: &str) -> Result<(String, Vec<BpmnElement>, Vec<BpmnFlow>), String> {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut process_name = String::new();
    let mut elements: Vec<BpmnElement> = Vec::new();
    let mut flows: Vec<BpmnFlow> = Vec::new();
    let mut cur_source = String::new();
    let mut cur_target = String::new();
    let mut skipped: Vec<&str> = Vec::new();

    let handle_attrs = |e: &quick_xml::events::BytesStart| -> (String, String) {
        let mut id = String::new();
        let mut name = String::new();
        for attr in e.attributes().filter_map(|a| a.ok()) {
            match attr.key.as_ref() {
                b"id" => id = String::from_utf8_lossy(&attr.value).to_string(),
                b"name" => name = String::from_utf8_lossy(&attr.value).to_string(),
                _ => {}
            }
        }
        (id, name)
    };

    let handle_flow_attrs = |e: &quick_xml::events::BytesStart| -> (String, String) {
        let mut source = String::new();
        let mut target = String::new();
        for attr in e.attributes().filter_map(|a| a.ok()) {
            match attr.key.as_ref() {
                b"sourceRef" => source = String::from_utf8_lossy(&attr.value).to_string(),
                b"targetRef" => target = String::from_utf8_lossy(&attr.value).to_string(),
                _ => {}
            }
        }
        (source, target)
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "process" => {
                        let (_, name) = handle_attrs(&e);
                        process_name = name;
                    }
                    "userTask" | "serviceTask" | "exclusiveGateway" | "endEvent" => {
                        let (id, name) = handle_attrs(&e);
                        let display_name = if name.is_empty() { id.clone() } else { name };
                        elements.push(BpmnElement {
                            id,
                            name: display_name,
                            kind: tag,
                        });
                    }
                    "sequenceFlow" => {
                        let (s, t) = handle_flow_attrs(&e);
                        cur_source = s;
                        cur_target = t;
                    }
                    "subProcess" | "parallelGateway" | "inclusiveGateway" | "callActivity" => {
                        skipped.push(Box::leak(tag.into_boxed_str()));
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"sequenceFlow"
                    && !cur_source.is_empty()
                    && !cur_target.is_empty()
                {
                    flows.push(BpmnFlow { source: cur_source.clone(), target: cur_target.clone() });
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("BPMN XML 解析失败：{e}")),
            _ => {}
        }
        buf.clear();
    }
    if !skipped.is_empty() {
        tracing::warn!(?skipped, "BPMN import skipped unsupported elements");
    }
    Ok((process_name, elements, flows))
}

/// BPMN 元素 → 流程定义步骤 + next 出口。
fn bpmn_to_steps(elements: &[BpmnElement], flows: &[BpmnFlow]) -> Result<Vec<Value>, String> {
    use std::collections::HashMap;
    let mut key_map: HashMap<&str, String> = HashMap::new();
    let mut steps: Vec<Value> = Vec::new();

    for (i, element) in elements.iter().enumerate() {
        let key = format!("s{}", i + 1);
        key_map.insert(element.id.as_str(), key.clone());
        let step = match element.kind.as_str() {
            "userTask" => json!({
                "key": key, "name": element.name, "type": "approval",
                "config": {"assignee": {"role": "super_admin"}}
            }),
            "serviceTask" => json!({
                "key": key, "name": element.name, "type": "auto",
                "config": {"action": "set_context", "params": {}}
            }),
            "exclusiveGateway" => json!({
                "key": key, "name": element.name, "type": "notification",
                "config": {"assignee": {"role": "super_admin"}}
            }),
            "endEvent" => json!({
                "key": key, "name": element.name, "type": "notification",
                "config": {"assignee": {"role": "super_admin"}}
            }),
            _ => continue,
        };
        steps.push(step);
    }

    // sequenceFlow → next 出口
    let mut next_map: HashMap<String, Vec<Value>> = HashMap::new();
    for flow in flows {
        if let (Some(sk), Some(tk)) = (key_map.get(flow.source.as_str()), key_map.get(flow.target.as_str())) {
            next_map.entry(sk.clone()).or_default().push(json!({"to": tk}));
        }
    }
    for step in steps.iter_mut() {
        if let Some(key) = step.get("key").and_then(Value::as_str) {
            if let Some(next) = next_map.get(key).filter(|n| !n.is_empty()) {
                if let Some(obj) = step.as_object_mut() {
                    obj.insert("next".to_string(), json!(next));
                }
            }
        }
    }
    Ok(steps)
}

async fn import_bpmn(
    user: CurrentUser,
    State(state): State<InfraState>,
    Json(request): Json<ImportBpmnRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let (process_name, elements, flows) =
        parse_bpmn(&request.xml).map_err(AppError::bad_request)?;
    let name = request.name.unwrap_or(
        if process_name.is_empty() { "BPMN 导入流程" } else { &process_name }.to_string(),
    );
    let steps = bpmn_to_steps(&elements, &flows).map_err(AppError::bad_request)?;
    if steps.is_empty() {
        return Err(AppError::bad_request("BPMN 中没有可映射的任务节点（userTask/serviceTask/endEvent）"));
    }
    let definition = json!({"steps": steps});
    super::engine::validate_definition(&definition).map_err(AppError::bad_request)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO infra_flow_definition (name, description, status, definition, version, creator, updater, tenant_id)
         VALUES ($1, $2, 0, $3, 1, $4, $4, $5) RETURNING id",
    )
    .bind(&name)
    .bind(format!("BPMN 导入（{} 个步骤）", steps.len()))
    .bind(&definition)
    .bind(&user.username)
    .bind(tenant.id())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::bad_request(format!("BPMN 导入失败: {e}")))?;
    Ok(Json(ApiResponse::new(json!({ "id": id, "name": name, "stepCount": steps.len() }))))
}
