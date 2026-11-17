//! 产品内流程引擎：定义（模板）→ 实例 → 步骤推进。
//!
//! v1 模型：顺序步骤 + 步骤级条件跳过（condition 对 payload/context 求值，
//! false → skipped）。节点类型：approval / manual / auto / notification /
//! ticket_wait。推进由 API 动作（审批/人工完成）与后台 runner（超时/重试/
//! 工单等待轮询）共同驱动；并发以实例级 advisory lock 串行化。
//! 与既有资源工单并存：`create_ticket` 自动节点把工单接入流程。

pub(crate) mod actions;
pub(crate) mod definition;
pub(crate) mod engine;
pub(crate) mod instance;
pub(crate) mod notify;
pub(crate) mod runner;

use aide::axum::ApiRouter;

use crate::InfraState;

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .merge(definition::routes())
        .merge(instance::routes())
}

/// 事件触发入口（pool-only，供任务 worker 等只有 DbPool 的调用方使用）：
/// 查名字匹配的启用定义 → 创建实例 + 展开步骤（不推进，由 runner 兜底）。
/// 幂等：同 biz_type+biz_id 已有实例则跳过。失败只告警。
pub(crate) async fn trigger_by_event(
    pool: &rustset_framework_database::DbPool,
    tenant_id: i64,
    name_pattern: &str,
    biz_type: &str,
    biz_id: &str,
    payload: serde_json::Value,
) {
    use serde_json::Value;
    use sqlx::Row;
    let result: Result<(), String> = async {
        // 幂等：同 biz 已有实例则跳过
        let existing: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM infra_flow_instance
             WHERE biz_type=$1 AND biz_id=$2 AND tenant_id=$3 AND deleted=0",
        )
        .bind(biz_type)
        .bind(biz_id)
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;
        if existing > 0 {
            return Ok(());
        }
        // 名字 LIKE 匹配的启用定义
        let definition = sqlx::query(
            "SELECT id, name, definition, version FROM infra_flow_definition
             WHERE name LIKE $1 AND tenant_id=$2 AND deleted=0 AND status=0
             ORDER BY id LIMIT 1",
        )
        .bind(format!("%{name_pattern}%"))
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        let Some(definition) = definition else { return Ok(()) };
        let def_id: i64 = definition.get("id");
        let def_name: String = definition.get("name");
        let def_version: i32 = definition.get("version");
        let def_body: Value = definition.get("definition");

        // 创建实例 + 展开步骤（同 start_instance 的 SQL，但不调 advance）
        let instance_id: i64 = sqlx::query_scalar(
            "INSERT INTO infra_flow_instance
                 (definition_id, definition_snapshot, title, biz_type, biz_id, payload, starter, tenant_id)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(def_id)
        .bind(serde_json::json!({ "name": def_name, "version": def_version, "steps": def_body.get("steps") }))
        .bind(&format!("事件触发：{biz_type} {biz_id}"))
        .bind(biz_type)
        .bind(biz_id)
        .bind(&payload)
        .bind("system:event-trigger")
        .bind(tenant_id)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?;

        let steps = def_body
            .get("steps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for (index, step) in steps.iter().enumerate() {
            let mut config = step.get("config").cloned().unwrap_or_else(|| serde_json::json!({}));
            if let Some(condition) = step.get("condition").filter(|c| !c.is_null()) {
                if let Some(object) = config.as_object_mut() {
                    object.insert("condition".to_string(), condition.clone());
                }
            }
            sqlx::query(
                "INSERT INTO infra_flow_step
                     (instance_id, step_key, step_index, name, type, status, config, assignee, next, tenant_id)
                 VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8, $9)",
            )
            .bind(instance_id)
            .bind(step.get("key").and_then(Value::as_str).unwrap_or_default())
            .bind(index as i32)
            .bind(step.get("name").and_then(Value::as_str).unwrap_or_default())
            .bind(step.get("type").and_then(Value::as_str).unwrap_or_default())
            .bind(&config)
            .bind(
                step.get("config")
                    .and_then(|c| c.get("assignee"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            )
            .bind(step.get("next").cloned().unwrap_or_else(|| serde_json::json!([])))
            .bind(tenant_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        tracing::warn!(biz_type, biz_id, %error, "event flow trigger failed");
    }
}
