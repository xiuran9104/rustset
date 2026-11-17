use axum::{Json, extract::State};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use serde_json::{Value, json};

use crate::SystemState;

use super::shared::require;

pub async fn notice_push(
    user: CurrentUser,
    State(state): State<SystemState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&user, "system:notice:update")?;
    let tenant_id = current_tenant_id(&user)?;
    let notice_id = opt_i64_field(&payload, "id")
        .or_else(|| opt_i64_field(&payload, "noticeId"))
        .ok_or_else(|| AppError::bad_request("noticeId is required"))?;
    let notice = sqlx::query_as::<_, NoticePushRow>(
        "SELECT id, title, content, type AS type_
         FROM system_notice
         WHERE id=$1 AND deleted=0 AND status=0 AND tenant_id=$2",
    )
    .bind(notice_id)
    .bind(tenant_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to get notice"))?
    .ok_or_else(|| AppError::not_found("notice not found"))?;

    let inserted = sqlx::query_scalar::<_, i64>(
        "WITH target_users AS (
             SELECT id FROM system_users
             WHERE deleted=0 AND status=0 AND tenant_id=$1
         ),
         inserted AS (
             INSERT INTO system_notify_message
             (id, user_id, user_type, template_id, template_code, template_nickname,
              template_content, template_type, template_params, read_status, creator,
              create_time, updater, update_time, deleted, tenant_id)
             SELECT nextval('system_notify_message_seq'), id, 2, $2, $3, $4,
                    $5, $6, '{}', false, $7, now(), $7, now(), 0, $1
             FROM target_users
             RETURNING id
         )
         SELECT count(*) FROM inserted",
    )
    .bind(tenant_id)
    .bind(notice.id)
    .bind(format!("notice-{}", notice.id))
    .bind(&notice.title)
    .bind(&notice.content)
    .bind(notice.type_)
    .bind(&user.username)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to push notice"))?;

    Ok(Json(ApiResponse::new(inserted)))
}

pub async fn mail_template_send(
    user: CurrentUser,
    State(state): State<SystemState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "system:mail-template:send-mail")?;
    let template_code = str_field_required(&payload, "templateCode")?;
    let to_mail = opt_str_field(&payload, "mail")
        .or_else(|| opt_str_field(&payload, "toMail"))
        .or_else(|| opt_str_field(&payload, "toMails"))
        .ok_or_else(|| AppError::bad_request("mail is required"))?;
    let params = payload
        .get("templateParams")
        .or_else(|| payload.get("params"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let template = sqlx::query_as::<_, MailTemplateSendRow>(
        "SELECT t.id, t.code, t.account_id, t.nickname, t.title, t.content,
                a.mail AS from_mail
         FROM system_mail_template t
         JOIN system_mail_account a ON a.id=t.account_id AND a.deleted=0
         WHERE t.code=$1 AND t.status=0 AND t.deleted=0
         ORDER BY t.id
         LIMIT 1",
    )
    .bind(template_code)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to get mail template"))?
    .ok_or_else(|| AppError::not_found("mail template not found"))?;
    let content = render_template(&template.content, &params);
    let title = render_template(&template.title, &params);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO system_mail_log
         (id, user_id, user_type, to_mails, account_id, from_mail,
          template_id, template_code, template_nickname, template_title,
          template_content, template_params, send_status, send_time,
          send_exception, creator, create_time, updater, update_time, deleted)
         VALUES (nextval('system_mail_log_seq'), $1, $2, $3, $4, $5,
                 $6, $7, $8, $9, $10, $11, 30, now(),
                 'mail provider is not configured', $12, now(), $12, now(), 0)
         RETURNING id",
    )
    .bind(opt_i64_field(&payload, "userId"))
    .bind(opt_i16_field(&payload, "userType").unwrap_or(2))
    .bind(to_mail)
    .bind(template.account_id)
    .bind(template.from_mail)
    .bind(template.id)
    .bind(template.code)
    .bind(template.nickname)
    .bind(title)
    .bind(content)
    .bind(params.to_string())
    .bind(&user.username)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to create mail log"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

pub async fn sms_template_send(
    user: CurrentUser,
    State(state): State<SystemState>,
    Json(payload): Json<Value>,
) -> Result<Json<ApiResponse<String>>, AppError> {
    require(&user, "system:sms-template:send-sms")?;
    let template_code = str_field_required(&payload, "templateCode")?;
    let mobile = str_field_required(&payload, "mobile")?;
    let params = payload
        .get("templateParams")
        .or_else(|| payload.get("params"))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let template = sqlx::query_as::<_, SmsTemplateSendRow>(
        "SELECT id, type AS type_, code, content, api_template_id, channel_id, channel_code
         FROM system_sms_template
         WHERE code=$1 AND status=0 AND deleted=0
         ORDER BY id
         LIMIT 1",
    )
    .bind(template_code)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to get sms template"))?
    .ok_or_else(|| AppError::not_found("sms template not found"))?;
    let content = render_template(&template.content, &params);
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO system_sms_log
         (id, channel_id, channel_code, template_id, template_code, template_type,
          template_content, template_params, api_template_id, mobile, user_id,
          user_type, send_status, send_time, api_send_code, api_send_msg,
          receive_status, creator, create_time, updater, update_time, deleted)
         VALUES (nextval('system_sms_log_seq'), $1, $2, $3, $4, $5,
                 $6, $7, $8, $9, $10, $11, 30, now(), 'NOT_CONFIGURED',
                 'sms provider is not configured', 0, $12, now(), $12, now(), 0)
         RETURNING id",
    )
    .bind(template.channel_id)
    .bind(template.channel_code)
    .bind(template.id)
    .bind(template.code)
    .bind(template.type_)
    .bind(content)
    .bind(params.to_string())
    .bind(template.api_template_id)
    .bind(mobile)
    .bind(opt_i64_field(&payload, "userId"))
    .bind(opt_i16_field(&payload, "userType").unwrap_or(2))
    .bind(&user.username)
    .fetch_one(&state.pool)
    .await
    .map_err(|_| AppError::internal("failed to create sms log"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

fn current_tenant_id(user: &CurrentUser) -> Result<i64, AppError> {
    user.tenant_id
        .as_deref()
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| AppError::forbidden("tenant is required"))
}

fn str_field_required(value: &Value, key: &str) -> Result<String, AppError> {
    opt_str_field(value, key).ok_or_else(|| AppError::bad_request(format!("{key} is required")))
}

fn opt_str_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn opt_i64_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
    })
}

fn opt_i16_field(value: &Value, key: &str) -> Option<i16> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .and_then(|value| i16::try_from(value).ok())
}

fn render_template(template: &str, params: &Value) -> String {
    let Some(object) = params.as_object() else {
        return template.to_owned();
    };
    object
        .iter()
        .fold(template.to_owned(), |content, (key, value)| {
            let rendered = render_value(value);
            content
                .replace(&format!("{{{key}}}"), &rendered)
                .replace(&format!("${{{key}}}"), &rendered)
        })
}

fn render_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

#[derive(sqlx::FromRow)]
struct NoticePushRow {
    id: i64,
    title: String,
    content: String,
    type_: i32,
}

#[derive(sqlx::FromRow)]
struct MailTemplateSendRow {
    id: i64,
    code: String,
    account_id: i64,
    nickname: Option<String>,
    title: String,
    content: String,
    from_mail: String,
}

#[derive(sqlx::FromRow)]
struct SmsTemplateSendRow {
    id: i64,
    type_: i16,
    code: String,
    content: String,
    api_template_id: String,
    channel_id: i64,
    channel_code: String,
}
