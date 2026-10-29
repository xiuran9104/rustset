//! 工单到期生命周期：提醒锚定工单（审批许可的来源），资源侧的
//! `expire_at` 仅作同步展示与辅助校验，不产生第二套独立提醒。
//!
//! - 到期前 3 天、1 天：向申请人与运维（超级管理员）发送站内提醒
//! - 到期仍未续期：工单转为 `expired`，并自动生成回收工单进入审批流
//! - 续期工单审批通过：新有效期回写台账与原工单（见 provisioning 模块）

use chrono::{NaiveDate, NaiveDateTime, Utc};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time;

const TEMPLATE_CODE: &str = "ticket_expire_reminder";
/// 仍处于生命周期内的工单状态（已审批、尚未回收）。
const ACTIVE_STATUSES: &str =
    "'approved','pending_provision','provisioning','pending_delivery','delivered'";

pub(crate) fn start(pool: PgPool) {
    tokio::spawn(async move {
        let mut tick = time::interval(Duration::from_secs(30 * 60));
        loop {
            tick.tick().await;
            if let Err(error) = scan(&pool).await {
                tracing::warn!(%error, "ticket expiry scan failed");
            }
        }
    });
}

struct ExpiringTicket {
    id: i64,
    tenant_id: i64,
    ecs_name: String,
    expire_at: String,
    applicant_user_id: Option<i64>,
    resource_type: String,
}

async fn scan(pool: &PgPool) -> Result<(), sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, i64, String, String, i16, Option<i64>, String)>(&format!(
        "SELECT id, tenant_id, ecs_name, expire_at, expiry_remind_stage, applicant_user_id, resource_type
         FROM infra_resource_ticket
         WHERE deleted=0 AND expire_at IS NOT NULL AND expire_at <> ''
           AND ticket_status IN ({ACTIVE_STATUSES})
           AND expiry_remind_stage < 3"
    ))
    .fetch_all(pool)
    .await?;
    for (id, tenant_id, ecs_name, expire_at, remind_stage, applicant_user_id, resource_type) in rows
    {
        let ticket = ExpiringTicket {
            id,
            tenant_id,
            ecs_name,
            expire_at,
            applicant_user_id,
            resource_type,
        };
        let Some(days_left) = days_left(&ticket.expire_at) else {
            continue;
        };
        if days_left <= 0 {
            if let Err(error) = expire_ticket(pool, &ticket).await {
                tracing::warn!(ticket = ticket.id, %error, "failed to expire ticket");
            }
        } else if remind_stage < 1 && days_left <= 3 {
            remind(pool, &ticket, days_left, 1).await;
        } else if remind_stage < 2 && days_left <= 1 {
            remind(pool, &ticket, days_left, 2).await;
        }
    }
    Ok(())
}

/// 兼容 `YYYY-MM-DD`、`YYYY-MM-DD HH:MM` 和 datetime-local 的 `T` 分隔。
fn days_left(expire_at: &str) -> Option<i64> {
    let normalized = expire_at.trim().replace('T', " ");
    let parsed = NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M")
        .ok()
        .or_else(|| {
            let date = NaiveDate::parse_from_str(
                normalized.split_whitespace().next()?,
                "%Y-%m-%d",
            )
            .ok()?;
            date.and_hms_opt(0, 0, 0)
        })?;
    let now = Utc::now().naive_utc();
    Some((parsed.date() - now.date()).num_days())
}

async fn remind(pool: &PgPool, ticket: &ExpiringTicket, days_left: i64, stage: i16) {
    if let Err(error) = send_reminders(pool, ticket, days_left).await {
        tracing::warn!(ticket = ticket.id, %error, "failed to send expiry reminders");
        return;
    }
    if let Err(error) = sqlx::query(
        "UPDATE infra_resource_ticket SET expiry_remind_stage=$2, update_time=now()
         WHERE id=$1 AND expiry_remind_stage < $2",
    )
    .bind(ticket.id)
    .bind(stage)
    .execute(pool)
    .await
    {
        tracing::warn!(ticket = ticket.id, %error, "failed to advance reminder stage");
    }
}

async fn send_reminders(
    pool: &PgPool,
    ticket: &ExpiringTicket,
    days_left: i64,
) -> Result<(), sqlx::Error> {
    let template = sqlx::query_as::<_, (i64, String, String, String, i16)>(
        "SELECT id, code, nickname, content, type FROM system_notify_template
         WHERE code=$1 AND status=0 AND deleted=0 ORDER BY id LIMIT 1",
    )
    .bind(TEMPLATE_CODE)
    .fetch_optional(pool)
    .await?;
    let Some((template_id, code, nickname, content, template_type)) = template else {
        tracing::warn!("notify template {TEMPLATE_CODE} missing; reminder skipped");
        return Ok(());
    };
    let content = content
        .replace("{ticketName}", &ticket.ecs_name)
        .replace("{expireAt}", &ticket.expire_at)
        .replace("{daysLeft}", &days_left.to_string());
    let params = format!(
        "{{\"ticketName\":\"{}\",\"expireAt\":\"{}\",\"daysLeft\":{}}}",
        ticket.ecs_name, ticket.expire_at, days_left
    );

    // 收件人：申请人 + 运维（启用状态的超级管理员）。
    let mut recipients: Vec<i64> = ticket.applicant_user_id.into_iter().collect();
    let admins: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT ur.user_id FROM system_user_role ur
         JOIN system_role r ON r.id=ur.role_id AND r.deleted=0 AND r.status=0 AND r.code='super_admin'
         JOIN system_users u ON u.id=ur.user_id AND u.deleted=0 AND u.status=0
         WHERE ur.deleted=0",
    )
    .fetch_all(pool)
    .await?;
    recipients.extend(admins);
    recipients.sort_unstable();
    recipients.dedup();

    for user_id in recipients {
        if let Err(error) = sqlx::query(
            "INSERT INTO system_notify_message
             (id, user_id, user_type, template_id, template_code, template_nickname,
              template_content, template_type, template_params, read_status, creator,
              create_time, updater, update_time, deleted, tenant_id)
             VALUES (nextval('system_notify_message_seq'), $1, 2, $2, $3, $4,
                     $5, $6, $7, false, 'system', now(), 'system', now(), 0, $8)",
        )
        .bind(user_id)
        .bind(template_id)
        .bind(&code)
        .bind(&nickname)
        .bind(&content)
        .bind(i32::from(template_type))
        .bind(&params)
        .bind(ticket.tenant_id)
        .execute(pool)
        .await
        {
            tracing::warn!(user_id, %error, "failed to insert expiry reminder message");
        }
    }
    Ok(())
}

/// 到期未续期：工单转 `expired`，并自动生成回收工单进入审批流。
/// 云资源的交付台账以 resource_id 记录原工单号，可链接回收目标；
/// 物理资源无此链接，回收工单由运维人工指定范围。
async fn expire_ticket(pool: &PgPool, ticket: &ExpiringTicket) -> Result<(), sqlx::Error> {
    let updated = sqlx::query(
        "UPDATE infra_resource_ticket
         SET ticket_status='expired', expiry_remind_stage=3, update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND deleted=0 AND expiry_remind_stage < 3",
    )
    .bind(ticket.id)
    .bind(ticket.tenant_id)
    .execute(pool)
    .await?;
    if updated.rows_affected() == 0 {
        return Ok(());
    }

    let target: Option<i64> = if ticket.resource_type == "physical" {
        None
    } else {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM infra_cloud_resource
             WHERE resource_id=$1 AND tenant_id=$2 AND deleted=0
             ORDER BY id DESC LIMIT 1",
        )
        .bind(ticket.id.to_string())
        .bind(ticket.tenant_id)
        .fetch_optional(pool)
        .await?
    };
    let reclaim_name = format!("回收-{}", ticket.ecs_name);
    let remarks = format!(
        "工单 #{}（{}）于 {} 到期未续期，系统自动生成回收工单，审批通过后将销毁对应资源。",
        ticket.id, ticket.ecs_name, ticket.expire_at
    );
    let reclaim_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO infra_resource_ticket
         (id, resource_type, ecs_name, ticket_type, ticket_status, risk_level,
          target_resource_id, target_resource_type, remarks, applicant_name,
          created_by, creator, create_time, updater, update_time, deleted, tenant_id)
         VALUES (nextval('infra_resource_ticket_seq'), $1, $2, 'reclaim', 'pending_approval',
                 'high', $3, $4, $5, '系统', 'system', 'system', now(), 'system', now(), 0, $6)
         RETURNING id",
    )
    .bind(&ticket.resource_type)
    .bind(&reclaim_name)
    .bind(target)
    .bind(if target.is_some() {
        Some("cloud".to_string())
    } else {
        None
    })
    .bind(&remarks)
    .bind(ticket.tenant_id)
    .fetch_one(pool)
    .await;
    match reclaim_id {
        Ok(id) => tracing::info!(expired_ticket = ticket.id, reclaim_ticket = id,
            "ticket expired; reclaim ticket created"),
        Err(error) => tracing::warn!(expired_ticket = ticket.id, %error,
            "failed to create reclaim ticket; manual reclaim required"),
    }
    Ok(())
}

/// 供 provisioning 模块在续期审批通过后调用：新有效期回写台账，
/// 并把由交付生成的原工单（云资源 instance_id 记录了原工单号）恢复授权。
pub(super) async fn apply_renewal(
    pool: &PgPool,
    tenant_id: i64,
    target: i64,
    physical: bool,
    new_expire_at: &str,
) {
    let ledger_table = if physical {
        "infra_physical_resource"
    } else {
        "infra_cloud_resource"
    };
    let updated = sqlx::query(&format!(
        "UPDATE {ledger_table} SET expire_at=$2, update_time=now()
         WHERE id=$1 AND tenant_id=$3 AND deleted=0"
    ))
    .bind(target)
    .bind(new_expire_at)
    .bind(tenant_id)
    .execute(pool)
    .await;
    match updated {
        Ok(result) if result.rows_affected() > 0 => {
            tracing::info!(target, new_expire_at, "renewal applied to resource ledger");
        }
        _ => tracing::warn!(target, "renewal target resource not found"),
    }
    if !physical {
        // 云资源交付行以 resource_id 记录原工单号；续期即恢复该工单授权。
        let source: Option<i64> = sqlx::query_scalar(&format!(
            "SELECT NULLIF(regexp_replace(resource_id, '\\D', '', 'g'), '')::bigint
             FROM {ledger_table} WHERE id=$1 AND tenant_id=$2 AND deleted=0"
        ))
        .bind(target)
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if let Some(source_id) = source {
            let result = sqlx::query(
                "UPDATE infra_resource_ticket
                 SET expire_at=$2, expiry_remind_stage=0,
                     ticket_status=CASE WHEN ticket_status='expired' THEN 'delivered' ELSE ticket_status END,
                     update_time=now()
                 WHERE id=$1 AND tenant_id=$3 AND deleted=0",
            )
            .bind(source_id)
            .bind(new_expire_at)
            .bind(tenant_id)
            .execute(pool)
            .await;
            if let Ok(result) = result
                && result.rows_affected() > 0
            {
                tracing::info!(source_id, "source ticket re-authorized by renewal");
            }
        }
    }
}
