//! 流程后台推进器：轮询三类待处理——
//! 1) 审批/人工步骤超时（escalate_notify 升级通知 / auto_reject 拒绝）
//! 2) auto 步骤重试到期（调 engine::advance 重执行）
//! 3) ticket_wait 轮询关联工单状态（终态 → done）
//! 模式沿用 task/worker.rs：DB 轮询为准、advisory lock 串行化由 engine 保证。

use rustset_framework_database::DbPool;
use sqlx::Row;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_secs(5);
const TICKET_WAIT_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) fn start(state: crate::InfraState) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(POLL_INTERVAL);
        loop {
            tick.tick().await;
            if let Err(error) = run_once(&state).await {
                tracing::warn!(%error, "flow runner pass failed");
            }
        }
    });
}

async fn run_once(state: &crate::InfraState) -> Result<(), String> {
    let pool = &state.pool;
    handle_pending_instances(state).await?;
    handle_timeouts(pool).await?;
    handle_auto_retries(state).await?;
    handle_ticket_waits(state).await?;
    handle_flow_waits(state).await?;
    Ok(())
}

/// start_flow wait 模式：轮询子实例终态。
/// active auto 步骤 result 含 mode=wait + childInstanceId，30s 间隔轮询。
async fn handle_flow_waits(state: &crate::InfraState) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT s.id, s.instance_id, s.tenant_id, s.result->>'childInstanceId' AS child_id,
                s.retry_at, i.status AS parent_status
         FROM infra_flow_step s
         JOIN infra_flow_instance i ON i.id=s.instance_id AND i.tenant_id=s.tenant_id
              AND i.status='running' AND i.deleted=0
         WHERE s.status='active' AND s.type='auto'
           AND s.result->>'mode'='wait'
           AND s.result->>'childInstanceId' IS NOT NULL
           AND s.retry_at IS NOT NULL AND s.retry_at <= now()
         LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        let step_id: i64 = row.get("id");
        let instance_id: i64 = row.get("instance_id");
        let tenant_id: i64 = row.get("tenant_id");
        let child_id: String = row.get("child_id");
        let Ok(child_id) = child_id.parse::<i64>() else { continue };

        let child_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM infra_flow_instance WHERE id=$1 AND deleted=0",
        )
        .bind(child_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(None);

        match child_status.as_deref() {
            Some("completed") => {
                sqlx::query(
                    "UPDATE infra_flow_step
                     SET status='done', finished_at=now(), operator='system',
                         result=result || $2
                     WHERE id=$1",
                )
                .bind(step_id)
                .bind(serde_json::json!({ "childStatus": "completed" }))
                .execute(&state.pool)
                .await
                .map_err(|e| e.to_string())?;
                if let Ok(tenant) = rustset_framework_tenant::TenantContext::from_persisted_id(Some(tenant_id)) {
                    super::engine::advance(state, &tenant, instance_id).await;
                }
            }
            Some("failed") | Some("cancelled") => {
                sqlx::query(
                    "UPDATE infra_flow_step
                     SET status='failed', finished_at=now(), operator='system',
                         result=result || $2
                     WHERE id=$1",
                )
                .bind(step_id)
                .bind(serde_json::json!({ "childStatus": child_status }))
                .execute(&state.pool)
                .await
                .map_err(|e| e.to_string())?;
                if let Ok(tenant) = rustset_framework_tenant::TenantContext::from_persisted_id(Some(tenant_id)) {
                    super::engine::advance(state, &tenant, instance_id).await;
                }
            }
            _ => {
                // 未终态：顺延 30s
                sqlx::query("UPDATE infra_flow_step SET retry_at=$2 WHERE id=$1")
                    .bind(step_id)
                    .bind(
                        chrono::Utc::now()
                            .checked_add_signed(chrono::Duration::seconds(30))
                            .map(|t| t.naive_utc()),
                    )
                    .execute(&state.pool)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// 兜底：扫描 running 实例中仍有 pending 步骤的（事件触发创建后未推进），
/// 调 advance 激活首个步骤。5 秒轮询内自动补推。
async fn handle_pending_instances(state: &crate::InfraState) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT DISTINCT i.id, i.tenant_id
         FROM infra_flow_instance i
         JOIN infra_flow_step s ON s.instance_id=i.id AND s.tenant_id=i.tenant_id
              AND s.status='pending'
         WHERE i.status='running' AND i.deleted=0
         LIMIT 10",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        let instance_id: i64 = row.get("id");
        let tenant_id: i64 = row.get("tenant_id");
        if let Ok(tenant) = rustset_framework_tenant::TenantContext::from_persisted_id(Some(tenant_id)) {
            super::engine::advance(state, &tenant, instance_id).await;
        }
    }
    Ok(())
}

/// 审批/人工步骤超时：按 onTimeout 处理（escalate_notify 通知一次后清空
/// timeout_at 防重复；auto_reject 置 failed 并失败收尾实例）。
async fn handle_timeouts(pool: &DbPool) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT s.id, s.instance_id, s.name, s.config, s.tenant_id, i.title,
                i.definition_snapshot->>'name' AS flow_name
         FROM infra_flow_step s
         JOIN infra_flow_instance i ON i.id=s.instance_id AND i.tenant_id=s.tenant_id
              AND i.status='running' AND i.deleted=0
         WHERE s.status='active' AND s.type IN ('approval','manual')
           AND s.timeout_at IS NOT NULL AND s.timeout_at <= now()
         LIMIT 50",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        let step_id: i64 = row.get("id");
        let instance_id: i64 = row.get("instance_id");
        let tenant_id: Option<i64> = row.get("tenant_id");
        let step_name: String = row.get("name");
        let title: String = row.get("title");
        let flow_name: String = row.get::<Option<String>, _>("flow_name").unwrap_or_default();
        let config: serde_json::Value = row.get("config");
        let on_timeout = config
            .get("onTimeout")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("escalate_notify");
        let assignee = config
            .get("assignee")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        match on_timeout {
            "auto_reject" => {
                sqlx::query(
                    "UPDATE infra_flow_step
                     SET status='failed', finished_at=now(), operator='system:timeout',
                         comment='审批超时自动拒绝'
                     WHERE id=$1 AND status='active'",
                )
                .bind(step_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                sqlx::query(
                    "UPDATE infra_flow_step SET status='cancelled', finished_at=now()
                     WHERE instance_id=$1 AND tenant_id=$2 AND status IN ('pending','active') AND id<>$3",
                )
                .bind(instance_id)
                .bind(tenant_id)
                .bind(step_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                sqlx::query(
                    "UPDATE infra_flow_instance
                     SET status='failed', finish_time=now(), update_time=now()
                     WHERE id=$1 AND tenant_id=$2 AND status='running'",
                )
                .bind(instance_id)
                .bind(tenant_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            _ => {
                // escalate_notify：给办理人+管理员发一次超时通知，清空 timeout_at 防重发。
                let mut recipients = super::notify::resolve_recipients(
                    pool,
                    tenant_id.unwrap_or_default(),
                    &assignee,
                )
                .await;
                let admins: Vec<i64> = sqlx::query_scalar(
                    "SELECT DISTINCT ur.user_id FROM system_user_role ur
                     JOIN system_role r ON r.id=ur.role_id AND r.deleted=0 AND r.status=0 AND r.code='super_admin'
                     JOIN system_users u ON u.id=ur.user_id AND u.deleted=0 AND u.status=0
                     WHERE ur.deleted=0",
                )
                .fetch_all(pool)
                .await
                .unwrap_or_default();
                recipients.extend(admins);
                recipients.sort_unstable();
                recipients.dedup();
                super::notify::send_step_notice(
                    pool,
                    tenant_id.unwrap_or_default(),
                    "flow_step_timeout",
                    &flow_name,
                    &step_name,
                    &title,
                    &recipients,
                    &serde_json::json!({}),
                )
                .await;
                sqlx::query(
                    "UPDATE infra_flow_step SET timeout_at=NULL WHERE id=$1",
                )
                .bind(step_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// auto 步骤重试到期：直接调 engine::advance（它会执行动作并推进）。
async fn handle_auto_retries(state: &crate::InfraState) -> Result<(), String> {
    let rows = sqlx::query_scalar::<_, (i64, Option<i64>)>(
        "SELECT s.instance_id, s.tenant_id FROM infra_flow_step s
         JOIN infra_flow_instance i ON i.id=s.instance_id AND i.tenant_id=s.tenant_id
              AND i.status='running' AND i.deleted=0
         WHERE s.status='active' AND s.type='auto' AND s.retry_at IS NOT NULL
           AND s.retry_at <= now() AND s.attempt_count < 3
         LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for (instance_id, tenant_id) in rows {
        let Some(tenant_id) = tenant_id else { continue };
        let Ok(tenant) = rustset_framework_tenant::TenantContext::from_persisted_id(Some(tenant_id)) else {
            continue;
        };
        super::engine::advance(state, &tenant, instance_id).await;
    }
    Ok(())
}

/// ticket_wait：轮询关联工单状态；终态（delivered/rejected/expired/cancelled）
/// → 步骤 done 带结果；否则顺延 retry_at。
async fn handle_ticket_waits(state: &crate::InfraState) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT s.id, s.instance_id, s.tenant_id, s.config, s.retry_at
         FROM infra_flow_step s
         JOIN infra_flow_instance i ON i.id=s.instance_id AND i.tenant_id=s.tenant_id
              AND i.status='running' AND i.deleted=0
         WHERE s.status='active' AND s.type='ticket_wait'
           AND s.retry_at IS NOT NULL AND s.retry_at <= now()
         LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| e.to_string())?;
    for row in rows {
        let step_id: i64 = row.get("id");
        let instance_id: i64 = row.get("instance_id");
        let tenant_id: Option<i64> = row.get("tenant_id");
        let config: serde_json::Value = row.get("config");
        // 工单 id：config.ticketId 直接给，或 ticketIdFrom 从实例 context 取。
        let ticket_id = config
            .get("ticketId")
            .and_then(serde_json::Value::as_i64)
            .or_else(|| {
                config
                    .get("ticketIdFrom")
                    .and_then(serde_json::Value::as_str)
                    .and_then(|field| {
                        // context 字段路径由 actions::create_ticket 写入 context.ticketId
                        let _ = field;
                        None
                    })
            });
        let ticket_id = match ticket_id {
            Some(id) => Some(id),
            None => {
                // 从实例 context.ticket_id / 实例 biz_id 兜底。
                sqlx::query_scalar::<_, Option<String>>(
                    "SELECT coalesce(context->>'ticket_id', biz_id)
                     FROM infra_flow_instance WHERE id=$1",
                )
                .bind(instance_id)
                .fetch_one(&state.pool)
                .await
                .map_err(|e| e.to_string())?
                .and_then(|v| v.parse().ok())
            }
        };
        let Some(ticket_id) = ticket_id else {
            // 无法解析工单：置失败，避免死轮询。
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status='failed', finished_at=now(), operator='system',
                     comment='ticket_wait 无法解析工单 id'
                 WHERE id=$1",
            )
            .bind(step_id)
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
            continue;
        };
        let ticket_status: Option<String> = sqlx::query_scalar(
            "SELECT ticket_status FROM infra_resource_ticket
             WHERE id=$1 AND tenant_id=$2 AND deleted=0",
        )
        .bind(ticket_id)
        .bind(tenant_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| e.to_string())?
        .flatten();
        // ticket_status 可能为空字符串（varchar 默认值兜不住时），flatten 已处理 NULL。
        let Some(status) = ticket_status.filter(|s| !s.is_empty()) else {
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status='failed', finished_at=now(), comment='ticket_wait 关联工单不存在'
                 WHERE id=$1",
            )
            .bind(step_id)
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
            continue;
        };
        if ["delivered", "rejected", "expired"].contains(&status.as_str()) {
            sqlx::query(
                "UPDATE infra_flow_step
                 SET status='done', finished_at=now(), operator='system',
                     result=result || $2
                 WHERE id=$1",
            )
            .bind(step_id)
            .bind(serde_json::json!({ "ticketStatus": status }))
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
            if let Some(tenant_id) = tenant_id {
                if let Ok(tenant) = rustset_framework_tenant::TenantContext::from_persisted_id(Some(tenant_id)) {
                    super::engine::advance(state, &tenant, instance_id).await;
                }
            }
        } else {
            // 未到终态：顺延下一轮。
            sqlx::query(
                "UPDATE infra_flow_step
                 SET retry_at=$2 WHERE id=$1",
            )
            .bind(step_id)
            .bind(
                chrono::Utc::now()
                    .checked_add_signed(
                    chrono::Duration::from_std(TICKET_WAIT_INTERVAL)
                        .unwrap_or_else(|_| chrono::Duration::seconds(30)),
                )
                    .map(|t| t.naive_utc()),
            )
            .execute(&state.pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
