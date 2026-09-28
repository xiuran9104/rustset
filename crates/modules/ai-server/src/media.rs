use crate::{AiModelFactory, AiState, require};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_ai_api::ImageRequest;
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
pub fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/image/draw", post(draw))
        .api_route("/ai/image/my-page", get(image_my_page))
        .api_route("/ai/image/get-my", get(image_my))
        .api_route("/ai/image/my-list-by-ids", get(image_ids))
        .api_route("/ai/image/delete-my", delete(image_delete_my))
        .api_route("/ai/image/page", get(image_page))
        .api_route("/ai/image/update", put(image_update))
        .api_route("/ai/image/delete", delete(image_delete))
        .api_route("/ai/music/generate", post(music_generate))
        .api_route("/ai/music/poll", post(music_poll))
        .api_route("/ai/music/page", get(music_page))
        .api_route("/ai/music/update", put(music_update))
        .api_route("/ai/music/delete", delete(music_delete))
}
pub(crate) fn spawn_music_sync(pool: sqlx::PgPool, factory: AiModelFactory) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    handle.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let _ = sync_pending_music(&pool, &factory, None).await;
        }
    });
}
fn text_at<'a>(v: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths
        .iter()
        .find_map(|p| v.pointer(p).and_then(Value::as_str))
}
async fn sync_one_music(
    pool: &sqlx::PgPool,
    factory: &AiModelFactory,
    id: i64,
    model_id: i64,
    task_id: &str,
    poll_count: i32,
) -> Result<(), AppError> {
    let response = factory.poll_music(model_id, task_id).await;
    let time = now();
    match response {
        Ok(result) => {
            let raw = &result.raw;
            let state = text_at(raw, &["/status", "/state", "/data/status", "/data/state"])
                .unwrap_or("pending")
                .to_ascii_lowercase();
            let audio = if result.url.is_empty() {
                text_at(
                    raw,
                    &[
                        "/audio_url",
                        "/audioUrl",
                        "/data/audio_url",
                        "/data/audioUrl",
                    ],
                )
                .unwrap_or("")
                .to_string()
            } else {
                result.url
            };
            let video = text_at(
                raw,
                &[
                    "/video_url",
                    "/videoUrl",
                    "/data/video_url",
                    "/data/videoUrl",
                ],
            )
            .unwrap_or("");
            let image = text_at(
                raw,
                &[
                    "/image_url",
                    "/imageUrl",
                    "/data/image_url",
                    "/data/imageUrl",
                ],
            )
            .unwrap_or("");
            let duration = raw
                .get("duration")
                .or_else(|| raw.pointer("/data/duration"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let failed = matches!(
                state.as_str(),
                "failed" | "error" | "cancelled" | "canceled"
            );
            let success = !audio.is_empty()
                || matches!(
                    state.as_str(),
                    "success" | "completed" | "complete" | "finished"
                );
            let status = if failed {
                30
            } else if success {
                20
            } else {
                10
            };
            let error = if failed {
                text_at(
                    raw,
                    &["/error", "/error_message", "/message", "/data/error"],
                )
                .unwrap_or("音乐生成失败")
            } else {
                ""
            };
            sqlx::query("UPDATE ai.music SET status=$2,audio_url=COALESCE(NULLIF($3,''),audio_url),video_url=COALESCE(NULLIF($4,''),video_url),image_url=COALESCE(NULLIF($5,''),image_url),duration=CASE WHEN $6>0 THEN $6 ELSE duration END,error_message=NULLIF($7,''),poll_count=poll_count+1,last_poll_time=$8,finish_time=CASE WHEN $2 IN(20,30) THEN $8 ELSE finish_time END WHERE id=$1").bind(id).bind(status).bind(audio).bind(video).bind(image).bind(duration).bind(error).bind(time).execute(pool).await.map_err(|_|AppError::internal("同步音乐任务失败"))?;
        }
        Err(error) => {
            let attempts = poll_count + 1;
            let terminal = attempts >= 720;
            sqlx::query("UPDATE ai.music SET poll_count=$2,last_poll_time=$3,status=CASE WHEN $4 THEN 30 ELSE status END,error_message=$5,finish_time=CASE WHEN $4 THEN $3 ELSE finish_time END WHERE id=$1").bind(id).bind(attempts).bind(time).bind(terminal).bind(format!("{error:?}")).execute(pool).await.map_err(|_|AppError::internal("同步音乐任务失败"))?;
        }
    }
    Ok(())
}
async fn sync_pending_music(
    pool: &sqlx::PgPool,
    factory: &AiModelFactory,
    ids: Option<&[i64]>,
) -> Result<Vec<i64>, AppError> {
    let rows=sqlx::query("SELECT id,model_id,task_id,poll_count FROM ai.music WHERE status=10 AND task_id IS NOT NULL AND ($1::bigint[] IS NULL OR id=ANY($1)) ORDER BY coalesce(last_poll_time,0) LIMIT 50").bind(ids).fetch_all(pool).await.map_err(|_|AppError::internal("读取待同步音乐失败"))?;
    let mut synced = Vec::new();
    for row in rows {
        let id = row.get("id");
        sync_one_music(
            pool,
            factory,
            id,
            row.get("model_id"),
            row.get("task_id"),
            row.get("poll_count"),
        )
        .await?;
        synced.push(id)
    }
    Ok(synced)
}
#[derive(Deserialize, JsonSchema)]
struct PollRequest {
    #[serde(default)]
    ids: Vec<i64>,
}
async fn music_poll(
    _u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<PollRequest>,
) -> Result<Json<ApiResponse<Vec<i64>>>, AppError> {
    let ids = if v.ids.is_empty() {
        None
    } else {
        Some(v.ids.as_slice())
    };
    Ok(Json(ApiResponse::new(
        sync_pending_music(&s.pool, &s.factory, ids).await?,
    )))
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn id() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
#[derive(Deserialize, JsonSchema)]
struct Id {
    id: i64,
}
#[derive(Deserialize, JsonSchema)]
struct Ids {
    ids: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Page {
    page_no: Option<i64>,
    page_size: Option<i64>,
    user_id: Option<String>,
    status: Option<i32>,
    public_status: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Draw {
    prompt: String,
    model_id: i64,
    width: i32,
    height: i32,
    style: Option<String>,
    #[serde(default)]
    options: Value,
}
fn image(r: sqlx::postgres::PgRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"userId":r.get::<String,_>("user_id"),"platform":r.get::<String,_>("platform"),"model":r.get::<String,_>("model"),"prompt":r.get::<String,_>("prompt"),"width":r.get::<i32,_>("width"),"height":r.get::<i32,_>("height"),"status":r.get::<i32,_>("status"),"publicStatus":r.get::<bool,_>("public_status"),"picUrl":r.get::<Option<String>,_>("pic_url"),"errorMessage":r.get::<Option<String>,_>("error_message"),"options":r.get::<Value,_>("options"),"taskId":r.get::<Option<String>,_>("task_id"),"buttons":r.get::<Value,_>("buttons"),"createTime":r.get::<i64,_>("create_time"),"finishTime":r.get::<Option<i64>,_>("finish_time")})
}
async fn draw(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Draw>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    if v.prompt.trim().is_empty() {
        return Err(AppError::bad_request("提示词不能为空"));
    }
    let c = s.factory.config(v.model_id).await?;
    let id = id();
    let mut options = v.options;
    if let Some(style) = v.style {
        options["style"] = json!(style)
    }
    sqlx::query("INSERT INTO ai.images(id,user_id,model_id,platform,model,prompt,width,height,status,options,create_time)VALUES($1,$2,$3,$4,$5,$6,$7,$8,10,$9,$10)").bind(id).bind(&u.user_id).bind(v.model_id).bind(&c.platform).bind(&c.model).bind(&v.prompt).bind(v.width).bind(v.height).bind(options).bind(now()).execute(&s.pool).await.map_err(|_|AppError::internal("创建绘图任务失败"))?;
    let result = s
        .factory
        .image(
            v.model_id,
            ImageRequest {
                prompt: v.prompt,
                size: format!("{}x{}", v.width, v.height),
                references: Vec::new(),
            },
        )
        .await;
    match result {
        Ok(x) => {
            sqlx::query(
                "UPDATE ai.images SET status=20,pic_url=$2,task_id=$3,finish_time=$4 WHERE id=$1",
            )
            .bind(id)
            .bind(x.url)
            .bind(x.task_id)
            .bind(now())
            .execute(&s.pool)
            .await
            .ok();
        }
        Err(e) => {
            sqlx::query(
                "UPDATE ai.images SET status=30,error_message=$2,finish_time=$3 WHERE id=$1",
            )
            .bind(id)
            .bind(format!("{e:?}"))
            .bind(now())
            .execute(&s.pool)
            .await
            .ok();
            return Err(e);
        }
    }
    Ok(Json(ApiResponse::new(id)))
}
async fn image_my_page(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    image_page_for(&s, &q, Some(&u.user_id)).await
}
async fn image_page_for(
    s: &AiState,
    q: &Page,
    owner: Option<&str>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let p = q.page_no.unwrap_or(1).max(1);
    let n = q.page_size.unwrap_or(10).clamp(1, 100);
    let user = owner.map(str::to_string).or_else(|| q.user_id.clone());
    let rows=sqlx::query("SELECT * FROM ai.images WHERE ($1::text IS NULL OR user_id=$1) AND ($2::int IS NULL OR status=$2) AND ($3::bool IS NULL OR public_status=$3) ORDER BY id DESC LIMIT $4 OFFSET $5").bind(&user).bind(q.status).bind(q.public_status).bind(n).bind((p-1)*n).fetch_all(&s.pool).await.map_err(|_|AppError::internal("读取绘图失败"))?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM ai.images WHERE ($1::text IS NULL OR user_id=$1) AND ($2::int IS NULL OR status=$2) AND ($3::bool IS NULL OR public_status=$3)").bind(&user).bind(q.status).bind(q.public_status).fetch_one(&s.pool).await.map_err(|_|AppError::internal("读取绘图失败"))?;
    Ok(Json(ApiResponse::new(
        json!({"list":rows.into_iter().map(image).collect::<Vec<_>>(),"total":total}),
    )))
}
async fn image_page(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&u, "ai:image:query")?;
    image_page_for(&s, &q, None).await
}
async fn image_my(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let r = sqlx::query("SELECT * FROM ai.images WHERE id=$1 AND user_id=$2")
        .bind(q.id)
        .bind(&u.user_id)
        .fetch_optional(&s.pool)
        .await
        .map_err(|_| AppError::internal("读取绘图失败"))?
        .ok_or_else(|| AppError::not_found("绘图不存在"))?;
    Ok(Json(ApiResponse::new(image(r))))
}
async fn image_ids(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Ids>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let ids = q
        .ids
        .split(',')
        .filter_map(|x| x.parse().ok())
        .collect::<Vec<i64>>();
    let rows = sqlx::query("SELECT * FROM ai.images WHERE id=ANY($1) AND user_id=$2")
        .bind(ids)
        .bind(&u.user_id)
        .fetch_all(&s.pool)
        .await
        .map_err(|_| AppError::internal("读取绘图失败"))?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(image).collect(),
    )))
}
async fn image_delete_my(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let r = sqlx::query("DELETE FROM ai.images WHERE id=$1 AND user_id=$2")
        .bind(q.id)
        .bind(&u.user_id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("删除绘图失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Update {
    id: i64,
    public_status: bool,
}
async fn image_update(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Update>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:image:update")?;
    let r = sqlx::query("UPDATE ai.images SET public_status=$2 WHERE id=$1")
        .bind(v.id)
        .bind(v.public_status)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("更新绘图失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
async fn image_delete(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:image:delete")?;
    let r = sqlx::query("DELETE FROM ai.images WHERE id=$1")
        .bind(q.id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("删除绘图失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct MusicGenerate {
    model_id: Option<i64>,
    title: Option<String>,
    lyric: Option<String>,
    prompt: Option<String>,
    tags: Option<String>,
    generate_mode: Option<i32>,
}
async fn music_generate(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<MusicGenerate>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    let model_id = match v.model_id {
        Some(id) => id,
        None => sqlx::query_scalar(
            "SELECT id FROM ai.model_configs WHERE type='music' AND status=0 ORDER BY id LIMIT 1",
        )
        .fetch_optional(&s.pool)
        .await
        .map_err(|_| AppError::internal("读取音乐模型失败"))?
        .ok_or_else(|| AppError::bad_request("请先配置启用的音乐模型"))?,
    };
    let c = s.factory.config(model_id).await?;
    let id = id();
    let prompt = v.prompt.unwrap_or_default();
    sqlx::query("INSERT INTO ai.music(id,user_id,model_id,title,lyric,status,prompt,platform,model,generate_mode,tags,create_time)VALUES($1,$2,$3,$4,$5,10,$6,$7,$8,$9,$10,$11)").bind(id).bind(&u.user_id).bind(model_id).bind(v.title.unwrap_or_default()).bind(v.lyric.unwrap_or_default()).bind(&prompt).bind(&c.platform).bind(&c.model).bind(v.generate_mode.unwrap_or(1)).bind(v.tags.unwrap_or_default()).bind(now()).execute(&s.pool).await.map_err(|_|AppError::internal("创建音乐任务失败"))?;
    let result = s.factory.music(model_id, json!({"prompt":prompt})).await;
    match result {
        Ok(x) => {
            let video = text_at(
                &x.raw,
                &[
                    "/video_url",
                    "/videoUrl",
                    "/data/video_url",
                    "/data/videoUrl",
                ],
            )
            .unwrap_or("");
            let image = text_at(
                &x.raw,
                &[
                    "/image_url",
                    "/imageUrl",
                    "/data/image_url",
                    "/data/imageUrl",
                ],
            )
            .unwrap_or("");
            let duration = x
                .raw
                .get("duration")
                .or_else(|| x.raw.pointer("/data/duration"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            sqlx::query("UPDATE ai.music SET status=$2,audio_url=NULLIF($3,''),video_url=NULLIF($4,''),image_url=NULLIF($5,''),duration=$6,task_id=$7,finish_time=CASE WHEN $3='' THEN NULL ELSE $8 END WHERE id=$1").bind(id).bind(if x.url.is_empty(){10}else{20}).bind(x.url).bind(video).bind(image).bind(duration).bind(x.task_id).bind(now()).execute(&s.pool).await.ok();
        }
        Err(e) => {
            sqlx::query(
                "UPDATE ai.music SET status=30,error_message=$2,finish_time=$3 WHERE id=$1",
            )
            .bind(id)
            .bind(format!("{e:?}"))
            .bind(now())
            .execute(&s.pool)
            .await
            .ok();
            return Err(e);
        }
    }
    Ok(Json(ApiResponse::new(id)))
}
fn music(r: sqlx::postgres::PgRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"userId":r.get::<String,_>("user_id"),"title":r.get::<String,_>("title"),"lyric":r.get::<String,_>("lyric"),"imageUrl":r.get::<Option<String>,_>("image_url"),"audioUrl":r.get::<Option<String>,_>("audio_url"),"videoUrl":r.get::<Option<String>,_>("video_url"),"status":r.get::<i32,_>("status"),"prompt":r.get::<String,_>("prompt"),"platform":r.get::<String,_>("platform"),"model":r.get::<String,_>("model"),"generateMode":r.get::<i32,_>("generate_mode"),"tags":r.get::<String,_>("tags"),"duration":r.get::<f64,_>("duration"),"publicStatus":r.get::<bool,_>("public_status"),"taskId":r.get::<Option<String>,_>("task_id"),"errorMessage":r.get::<Option<String>,_>("error_message")})
}
async fn music_page(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Page>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&u, "ai:music:query")?;
    let p = q.page_no.unwrap_or(1).max(1);
    let n = q.page_size.unwrap_or(10).clamp(1, 100);
    let rows=sqlx::query("SELECT * FROM ai.music WHERE ($1::text IS NULL OR user_id=$1) AND ($2::int IS NULL OR status=$2) ORDER BY id DESC LIMIT $3 OFFSET $4").bind(&q.user_id).bind(q.status).bind(n).bind((p-1)*n).fetch_all(&s.pool).await.map_err(|_|AppError::internal("读取音乐失败"))?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM ai.music WHERE ($1::text IS NULL OR user_id=$1) AND ($2::int IS NULL OR status=$2)").bind(&q.user_id).bind(q.status).fetch_one(&s.pool).await.map_err(|_|AppError::internal("读取音乐失败"))?;
    Ok(Json(ApiResponse::new(
        json!({"list":rows.into_iter().map(music).collect::<Vec<_>>(),"total":total}),
    )))
}
async fn music_update(
    u: CurrentUser,
    State(s): State<AiState>,
    Json(v): Json<Update>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:music:update")?;
    let r = sqlx::query("UPDATE ai.music SET public_status=$2 WHERE id=$1")
        .bind(v.id)
        .bind(v.public_status)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("更新音乐失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
async fn music_delete(
    u: CurrentUser,
    State(s): State<AiState>,
    Query(q): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&u, "ai:music:delete")?;
    let r = sqlx::query("DELETE FROM ai.music WHERE id=$1")
        .bind(q.id)
        .execute(&s.pool)
        .await
        .map_err(|_| AppError::internal("删除音乐失败"))?;
    Ok(Json(ApiResponse::new(r.rows_affected() > 0)))
}
