//! 流程站内信通知：收件人解析 + 模板渲染直写 system_notify_message。
//! 沿用工单到期提醒的直写模式（expiry.rs），跨模块共享序列。

use rustset_framework_database::{BindIn, DbPool};
use serde_json::{Value, json};

/// assignee → 用户 id 集合。支持 {"role":"super_admin"} / {"userIds":[..]} /
/// {"usernames":["admin"]}，三种可并存（取并集）。
pub(crate) async fn resolve_recipients(
    pool: &DbPool,
    tenant_id: i64,
    assignee: &Value,
) -> Vec<i64> {
    let mut recipients: Vec<i64> = Vec::new();
    if let Some(role) = assignee.get("role").and_then(Value::as_str) {
        let admins: Vec<i64> = sqlx::query_scalar(
            "SELECT DISTINCT ur.user_id FROM system_user_role ur
             JOIN system_role r ON r.id=ur.role_id AND r.deleted=0 AND r.status=0 AND r.code=$1
             JOIN system_users u ON u.id=ur.user_id AND u.deleted=0 AND u.status=0
             WHERE ur.deleted=0 AND (u.tenant_id=$2 OR $2 IS NULL)",
        )
        .bind(role)
        .bind(tenant_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
        recipients.extend(admins);
    }
    if let Some(ids) = assignee.get("userIds").and_then(Value::as_array) {
        recipients.extend(ids.iter().filter_map(Value::as_i64));
    }
    if let Some(names) = assignee.get("usernames").and_then(Value::as_array) {
        let names: Vec<String> = names
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        if !names.is_empty() {
            // IN 展开遵循信创空列表规则；这里 names 非空。
            let placeholders =
                rustset_framework_database::expand_in_placeholders(&names, 1);
            let ids: Vec<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT id FROM system_users WHERE username IN ({placeholders}) AND deleted=0"
            )))
            .bind_in(&names)
            .fetch_all(pool)
            .await
            .unwrap_or_default();
            recipients.extend(ids);
        }
    }
    recipients.sort_unstable();
    recipients.dedup();
    recipients
}

/// 当前登录用户的数字 id（system_users.id）；assignee.userIds 匹配用。
pub(crate) async fn current_user_numeric_id(
    pool: &DbPool,
    identity_uuid: &str,
) -> Option<i64> {
    sqlx::query_scalar("SELECT id FROM system_users WHERE identity_uuid=$1 AND deleted=0")
        .bind(identity_uuid)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

/// assignee 是否包含该用户（角色 / 数字 id / 用户名任一命中）。
pub(crate) fn assignee_matches(
    user: &rustset_framework_security::CurrentUser,
    numeric_id: Option<i64>,
    assignee: &Value,
) -> bool {
    if let Some(role) = assignee.get("role").and_then(Value::as_str) {
        if user.role_codes.iter().any(|code| code == role) {
            return true;
        }
    }
    if let Some(id) = numeric_id {
        if assignee
            .get("userIds")
            .and_then(Value::as_array)
            .is_some_and(|ids| ids.iter().any(|v| v.as_i64() == Some(id)))
        {
            return true;
        }
    }
    assignee
        .get("usernames")
        .and_then(Value::as_array)
        .is_some_and(|names| {
            names
                .iter()
                .any(|name| name.as_str() == Some(user.username.as_str()))
        })
}

/// 渲染模板并发送。占位符：flowName / stepName / title，外加 params 里的
/// 自定义键（{ticketName} 等）——params 为步骤 config.params 插值后的对象，
/// 同名自定义参数覆盖内置占位符（merge 语义：自定义优先）。
pub(crate) async fn send_step_notice(
    pool: &DbPool,
    tenant_id: i64,
    template_code: &str,
    flow_name: &str,
    step_name: &str,
    title: &str,
    recipients: &[i64],
    params: &Value,
) {
    let template = sqlx::query_as::<_, (i64, String, String, String, i16)>(
        "SELECT id, code, nickname, content, type FROM system_notify_template
         WHERE code=$1 AND status=0 AND deleted=0 ORDER BY id LIMIT 1",
    )
    .bind(template_code)
    .fetch_optional(pool)
    .await;
    let Ok(Some((template_id, code, nickname, content, template_type))) = template else {
        tracing::warn!(template_code, "notify template missing; flow notice skipped");
        return;
    };
    // 模板参数对象：内置三键 + 自定义 merge（值统一字符串化供占位符替换）。
    let mut rendered = json!({
        "flowName": flow_name,
        "stepName": step_name,
        "title": title,
    });
    let mut content = content
        .replace("{flowName}", flow_name)
        .replace("{stepName}", step_name)
        .replace("{title}", title);
    if let Some(map) = params.as_object() {
        for (key, value) in map {
            let text = match value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            content = content.replace(&format!("{{{key}}}"), &text);
            if let Some(obj) = rendered.as_object_mut() {
                obj.insert(key.clone(), Value::String(text));
            }
        }
    }
    let rendered_params = rendered.to_string();
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
        .bind(&rendered_params)
        .bind(tenant_id)
        .execute(pool)
        .await
        {
            tracing::warn!(user_id, %error, "failed to insert flow notice message");
        }
    }
}
