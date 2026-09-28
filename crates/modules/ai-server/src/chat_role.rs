use crate::{AiState, require};
use aide::axum::ApiRouter;
use aide::axum::routing::{delete, get, post, put};
use axum::{
    Json,
    extract::{Query, State},
};
use rustset_framework_common::ApiResponse;
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

pub(crate) fn routes() -> ApiRouter<AiState> {
    ApiRouter::new()
        .api_route("/ai/chat-role/page", get(page))
        .api_route("/ai/chat-role/my-page", get(my_page))
        .api_route("/ai/chat-role/get", get(get_one))
        .api_route("/ai/chat-role/category-list", get(categories))
        .api_route("/ai/chat-role/create", post(create_admin))
        .api_route("/ai/chat-role/create-my", post(create_my))
        .api_route("/ai/chat-role/update", put(update))
        .api_route("/ai/chat-role/delete", delete(remove_admin))
        .api_route("/ai/chat-role/delete-my", delete(remove_my))
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
#[serde(rename_all = "camelCase")]
struct Page {
    page_no: Option<i64>,
    page_size: Option<i64>,
    name: Option<String>,
    category: Option<String>,
    public_status: Option<bool>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Save {
    id: Option<i64>,
    model_id: i64,
    name: String,
    avatar: Option<String>,
    category: Option<String>,
    sort: Option<i32>,
    description: Option<String>,
    system_message: Option<String>,
    welcome_message: Option<String>,
    public_status: Option<bool>,
    status: Option<i32>,
    #[serde(default)]
    knowledge_ids: Vec<i64>,
    #[serde(default)]
    tool_ids: Vec<i64>,
}
fn value(row: sqlx::postgres::PgRow) -> Value {
    json!({"id":row.get::<i64,_>("id"),"userId":row.get::<Option<String>,_>("user_id"),"modelId":row.get::<i64,_>("model_id"),
        "name":row.get::<String,_>("name"),"avatar":row.get::<String,_>("avatar"),"category":row.get::<String,_>("category"),
        "sort":row.get::<i32,_>("sort"),"description":row.get::<String,_>("description"),"systemMessage":row.get::<String,_>("system_message"),
        "welcomeMessage":row.get::<String,_>("welcome_message"),"publicStatus":row.get::<bool,_>("public_status"),"status":row.get::<i32,_>("status"),
        "knowledgeIds":row.get::<Vec<i64>,_>("knowledge_ids"),"toolIds":row.get::<Vec<i64>,_>("tool_ids"),"createTime":row.get::<i64,_>("create_time")})
}
async fn list(
    state: &AiState,
    query: &Page,
    owner: Option<&str>,
    public_only: bool,
) -> Result<Value, AppError> {
    let page = query.page_no.unwrap_or(1).max(1);
    let size = query.page_size.unwrap_or(10).clamp(1, 100);
    let rows=sqlx::query("SELECT * FROM ai.chat_roles WHERE ($1::text IS NULL OR user_id=$1) AND (NOT $2 OR public_status=true) AND ($3::text IS NULL OR name ILIKE '%'||$3||'%') AND ($4::text IS NULL OR category=$4) AND ($5::bool IS NULL OR public_status=$5) ORDER BY sort,id DESC LIMIT $6 OFFSET $7")
        .bind(owner).bind(public_only).bind(&query.name).bind(&query.category).bind(query.public_status).bind(size).bind((page-1)*size).fetch_all(&state.pool).await.map_err(|_|AppError::internal("读取聊天角色失败"))?;
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM ai.chat_roles WHERE ($1::text IS NULL OR user_id=$1) AND (NOT $2 OR public_status=true) AND ($3::text IS NULL OR name ILIKE '%'||$3||'%') AND ($4::text IS NULL OR category=$4) AND ($5::bool IS NULL OR public_status=$5)")
        .bind(owner).bind(public_only).bind(&query.name).bind(&query.category).bind(query.public_status).fetch_one(&state.pool).await.map_err(|_|AppError::internal("读取聊天角色失败"))?;
    Ok(json!({"list":rows.into_iter().map(value).collect::<Vec<_>>(),"total":total}))
}
async fn page(
    user: CurrentUser,
    State(state): State<AiState>,
    Query(query): Query<Page>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    require(&user, "ai:chat-role:query")?;
    Ok(Json(ApiResponse::new(
        list(&state, &query, None, false).await?,
    )))
}
async fn my_page(
    user: CurrentUser,
    State(state): State<AiState>,
    Query(query): Query<Page>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let owner = if query.public_status == Some(true) {
        None
    } else {
        Some(user.user_id.as_str())
    };
    Ok(Json(ApiResponse::new(
        list(&state, &query, owner, query.public_status == Some(true)).await?,
    )))
}
async fn get_one(
    user: CurrentUser,
    State(state): State<AiState>,
    Query(query): Query<Id>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let row = sqlx::query(
        "SELECT * FROM ai.chat_roles WHERE id=$1 AND (public_status=true OR user_id=$2)",
    )
    .bind(query.id)
    .bind(&user.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| AppError::internal("读取聊天角色失败"))?
    .ok_or_else(|| AppError::not_found("聊天角色不存在"))?;
    Ok(Json(ApiResponse::new(value(row))))
}
async fn categories(
    _user: CurrentUser,
    State(state): State<AiState>,
) -> Result<Json<ApiResponse<Vec<String>>>, AppError> {
    let values = sqlx::query_scalar(
        "SELECT DISTINCT category FROM ai.chat_roles WHERE status=1 ORDER BY category",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("读取角色分类失败"))?;
    Ok(Json(ApiResponse::new(values)))
}
async fn insert(state: &AiState, user_id: Option<&str>, input: Save) -> Result<i64, AppError> {
    state.factory.config(input.model_id).await?;
    let role_id = id();
    sqlx::query("INSERT INTO ai.chat_roles(id,user_id,model_id,name,avatar,category,sort,description,system_message,welcome_message,public_status,status,knowledge_ids,tool_ids,create_time,update_time)VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$15)").bind(role_id).bind(user_id).bind(input.model_id).bind(input.name).bind(input.avatar.unwrap_or_default()).bind(input.category.unwrap_or_else(||"通用".into())).bind(input.sort.unwrap_or(0)).bind(input.description.unwrap_or_default()).bind(input.system_message.unwrap_or_default()).bind(input.welcome_message.unwrap_or_default()).bind(input.public_status.unwrap_or(false)).bind(input.status.unwrap_or(1)).bind(input.knowledge_ids).bind(input.tool_ids).bind(now()).execute(&state.pool).await.map_err(|_|AppError::internal("创建聊天角色失败"))?;
    Ok(role_id)
}
async fn create_admin(
    user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Save>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    require(&user, "ai:chat-role:create")?;
    Ok(Json(ApiResponse::new(insert(&state, None, input).await?)))
}
async fn create_my(
    user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Save>,
) -> Result<Json<ApiResponse<i64>>, AppError> {
    Ok(Json(ApiResponse::new(
        insert(&state, Some(&user.user_id), input).await?,
    )))
}
async fn update(
    user: CurrentUser,
    State(state): State<AiState>,
    Json(input): Json<Save>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let role_id = input
        .id
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let admin = user.can(
        &rustset_framework_security::Permission::new("ai:chat-role:update")
            .map_err(|_| AppError::internal("invalid policy"))?,
    );
    let result=sqlx::query("UPDATE ai.chat_roles SET model_id=$3,name=$4,avatar=$5,category=$6,sort=$7,description=$8,system_message=$9,welcome_message=$10,public_status=$11,status=$12,knowledge_ids=$13,tool_ids=$14,update_time=$15 WHERE id=$1 AND ($2 OR user_id=$16)").bind(role_id).bind(admin).bind(input.model_id).bind(input.name).bind(input.avatar.unwrap_or_default()).bind(input.category.unwrap_or_else(||"通用".into())).bind(input.sort.unwrap_or(0)).bind(input.description.unwrap_or_default()).bind(input.system_message.unwrap_or_default()).bind(input.welcome_message.unwrap_or_default()).bind(input.public_status.unwrap_or(false)).bind(input.status.unwrap_or(1)).bind(input.knowledge_ids).bind(input.tool_ids).bind(now()).bind(&user.user_id).execute(&state.pool).await.map_err(|_|AppError::internal("更新聊天角色失败"))?;
    Ok(Json(ApiResponse::new(result.rows_affected() > 0)))
}
async fn remove_admin(
    user: CurrentUser,
    State(state): State<AiState>,
    Query(query): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    require(&user, "ai:chat-role:delete")?;
    let result = sqlx::query("DELETE FROM ai.chat_roles WHERE id=$1")
        .bind(query.id)
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("删除聊天角色失败"))?;
    Ok(Json(ApiResponse::new(result.rows_affected() > 0)))
}
async fn remove_my(
    user: CurrentUser,
    State(state): State<AiState>,
    Query(query): Query<Id>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let result = sqlx::query("DELETE FROM ai.chat_roles WHERE id=$1 AND user_id=$2")
        .bind(query.id)
        .bind(&user.user_id)
        .execute(&state.pool)
        .await
        .map_err(|_| AppError::internal("删除聊天角色失败"))?;
    Ok(Json(ApiResponse::new(result.rows_affected() > 0)))
}
