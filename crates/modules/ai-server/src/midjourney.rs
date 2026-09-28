use crate::{AiModelFactory, AiState};
use aide::axum::ApiRouter;
use aide::axum::routing::post;
use axum::{Json, extract::State};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

pub(crate) fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/image/midjourney/imagine", post(imagine))
        .api_route("/ai/image/midjourney/action", post(action))
        .api_route("/ai/image/midjourney/poll", post(poll))
}

pub(crate) fn spawn_sync(pool: sqlx::PgPool, factory: AiModelFactory) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let _ = sync_pending(&pool, &factory, None).await;
        }
    });
}

fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn new_id() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
fn text_at<'a>(value: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_str))
}
fn task_status(value: &Value, has_url: bool) -> i32 {
    let state = text_at(value, &["/status", "/state", "/data/status", "/data/state"])
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(
        state.as_str(),
        "failure" | "failed" | "error" | "cancelled" | "canceled"
    ) {
        30
    } else if has_url
        || matches!(
            state.as_str(),
            "success" | "completed" | "complete" | "finished" | "done"
        )
    {
        20
    } else {
        10
    }
}
fn buttons(value: &Value) -> Value {
    value
        .get("buttons")
        .or_else(|| value.pointer("/data/buttons"))
        .cloned()
        .unwrap_or_else(|| json!([]))
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Imagine {
    prompt: String,
    model_id: i64,
    #[serde(default)]
    base64_array: Vec<String>,
    width: Value,
    height: Value,
    version: Option<String>,
    refer_image_url: Option<String>,
}
fn dimension(value: &Value) -> Result<i32, AppError> {
    value
        .as_i64()
        .map(|v| v as i32)
        .or_else(|| value.as_str().and_then(|v| v.parse().ok()))
        .filter(|v| *v > 0)
        .ok_or_else(|| AppError::bad_request("图片尺寸无效"))
}
async fn imagine(
    user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Imagine>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    if input.prompt.trim().is_empty() {
        return Err(AppError::bad_request("提示词不能为空"));
    }
    let width = dimension(&input.width)?;
    let height = dimension(&input.height)?;
    let config = state.factory.config(input.model_id).await?;
    let id = new_id();
    let options = json!({"model":config.model,"version":input.version,"referImageUrl":input.refer_image_url,"base64Array":input.base64_array});
    sqlx::query("INSERT INTO ai.images(id,user_id,model_id,platform,model,prompt,width,height,status,options,create_time) VALUES($1,$2,$3,$4,$5,$6,$7,$8,10,$9,$10)")
        .bind(id).bind(&user.user_id).bind(input.model_id).bind(&config.platform).bind(&config.model)
        .bind(&input.prompt).bind(width).bind(height).bind(&options).bind(now()).execute(&state.pool).await
        .map_err(|_| AppError::internal("创建 Midjourney 任务失败"))?;
    let result = state.factory.midjourney_imagine(input.model_id, json!({
        "prompt":input.prompt,"base64Array":input.base64_array,"width":width,"height":height,
        "version":input.version,"referImageUrl":input.refer_image_url
    })).await;
    finish_submission(&state.pool, id, result).await?;
    Ok(Json(ApiResponse::new(id)))
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Action {
    id: i64,
    custom_id: String,
}
async fn action(
    user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Action>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    if input.custom_id.trim().is_empty() {
        return Err(AppError::bad_request("Action 按钮标识不能为空"));
    }
    let source = sqlx::query("SELECT * FROM ai.images WHERE id=$1 AND user_id=$2")
        .bind(input.id)
        .bind(&user.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|_| AppError::internal("读取 Midjourney 图片失败"))?
        .ok_or_else(|| AppError::not_found("图片不存在"))?;
    if source.get::<i32, _>("status") != 20 {
        return Err(AppError::bad_request("图片尚未生成完成"));
    }
    let task_id = source
        .get::<Option<String>, _>("task_id")
        .ok_or_else(|| AppError::bad_request("源图片缺少任务 ID"))?;
    let model_id = source.get::<i64, _>("model_id");
    let id = new_id();
    sqlx::query("INSERT INTO ai.images(id,user_id,model_id,platform,model,prompt,width,height,status,options,parent_id,action_custom_id,create_time) VALUES($1,$2,$3,$4,$5,$6,$7,$8,10,$9,$10,$11,$12)")
        .bind(id).bind(&user.user_id).bind(model_id).bind(source.get::<String,_>("platform"))
        .bind(source.get::<String,_>("model")).bind(source.get::<String,_>("prompt"))
        .bind(source.get::<i32,_>("width")).bind(source.get::<i32,_>("height"))
        .bind(source.get::<Value,_>("options")).bind(input.id).bind(&input.custom_id).bind(now())
        .execute(&state.pool).await.map_err(|_| AppError::internal("创建 Midjourney Action 任务失败"))?;
    let result = state
        .factory
        .midjourney_action(
            model_id,
            json!({"taskId":task_id,"customId":input.custom_id}),
        )
        .await;
    finish_submission(&state.pool, id, result).await?;
    Ok(Json(ApiResponse::new(id)))
}

async fn finish_submission(
    pool: &sqlx::PgPool,
    id: i64,
    result: Result<rustset_ai_api::MediaResponse, AppError>,
) -> Result<(), AppError> {
    match result {
        Ok(result) => {
            let status = if result.url.is_empty() { 10 } else { 20 };
            sqlx::query("UPDATE ai.images SET status=$2,pic_url=NULLIF($3,''),task_id=$4,buttons=$5,finish_time=CASE WHEN $2=20 THEN $6 END WHERE id=$1")
                .bind(id).bind(status).bind(result.url).bind(result.task_id).bind(buttons(&result.raw)).bind(now()).execute(pool).await
                .map_err(|_| AppError::internal("保存 Midjourney 任务失败"))?;
            Ok(())
        }
        Err(error) => {
            sqlx::query(
                "UPDATE ai.images SET status=30,error_message=$2,finish_time=$3 WHERE id=$1",
            )
            .bind(id)
            .bind(format!("{error:?}"))
            .bind(now())
            .execute(pool)
            .await
            .ok();
            Err(error)
        }
    }
}

#[derive(Deserialize, JsonSchema)]
struct Poll {
    #[serde(default)]
    ids: Vec<i64>,
}
async fn poll(
    _user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Poll>,
) -> Result<Json<ApiResponse<Vec<i64>>>, AppError> {
    let ids = if input.ids.is_empty() {
        None
    } else {
        Some(input.ids.as_slice())
    };
    Ok(Json(ApiResponse::new(
        sync_pending(&state.pool, &state.factory, ids).await?,
    )))
}
pub(crate) async fn sync_pending(
    pool: &sqlx::PgPool,
    factory: &AiModelFactory,
    ids: Option<&[i64]>,
) -> Result<Vec<i64>, AppError> {
    let rows = sqlx::query("SELECT id,model_id,task_id,poll_count FROM ai.images WHERE status=10 AND task_id IS NOT NULL AND platform='Midjourney' AND ($1::bigint[] IS NULL OR id=ANY($1)) ORDER BY COALESCE(last_poll_time,0) LIMIT 50")
        .bind(ids).fetch_all(pool).await.map_err(|_| AppError::internal("读取 Midjourney 待同步任务失败"))?;
    let mut synced = Vec::new();
    for row in rows {
        let id: i64 = row.get("id");
        let result = factory
            .poll_midjourney(row.get("model_id"), row.get("task_id"))
            .await;
        let time = now();
        match result {
            Ok(result) => {
                let status = task_status(&result.raw, !result.url.is_empty());
                let error = if status == 30 {
                    text_at(
                        &result.raw,
                        &["/error", "/failReason", "/message", "/data/error"],
                    )
                    .unwrap_or("Midjourney 生成失败")
                } else {
                    ""
                };
                sqlx::query("UPDATE ai.images SET status=$2,pic_url=COALESCE(NULLIF($3,''),pic_url),buttons=$4,error_message=NULLIF($5,''),poll_count=poll_count+1,last_poll_time=$6,finish_time=CASE WHEN $2 IN(20,30) THEN $6 ELSE finish_time END WHERE id=$1")
                    .bind(id).bind(status).bind(result.url).bind(buttons(&result.raw)).bind(error).bind(time).execute(pool).await
                    .map_err(|_| AppError::internal("同步 Midjourney 任务失败"))?;
            }
            Err(error) => {
                let attempts = row.get::<i32, _>("poll_count") + 1;
                let terminal = attempts >= 720;
                sqlx::query("UPDATE ai.images SET poll_count=$2,last_poll_time=$3,status=CASE WHEN $4 THEN 30 ELSE status END,error_message=$5,finish_time=CASE WHEN $4 THEN $3 ELSE finish_time END WHERE id=$1")
                    .bind(id).bind(attempts).bind(time).bind(terminal).bind(format!("{error:?}")).execute(pool).await
                    .map_err(|_| AppError::internal("同步 Midjourney 任务失败"))?;
            }
        }
        synced.push(id);
    }
    Ok(synced)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_terminal_states() {
        assert_eq!(task_status(&json!({"status":"SUCCESS"}), false), 20);
        assert_eq!(task_status(&json!({"state":"FAILURE"}), false), 30);
        assert_eq!(task_status(&json!({"status":"IN_PROGRESS"}), false), 10);
        assert_eq!(task_status(&json!({}), true), 20);
    }
    #[test]
    fn reads_nested_buttons() {
        assert_eq!(
            buttons(&json!({"data":{"buttons":[{"customId":"U1"}]}}))[0]["customId"],
            "U1"
        );
    }
}
