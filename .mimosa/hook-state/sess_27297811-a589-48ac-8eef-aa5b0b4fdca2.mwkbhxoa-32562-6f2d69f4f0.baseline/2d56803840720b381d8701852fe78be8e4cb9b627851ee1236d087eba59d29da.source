use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::Json,
    http::{Request, StatusCode, header},
    response::Response,
    routing::post,
};
use http_body_util::BodyExt;
use rustset_framework_database::{DatabaseConfig, connect, migrate};
use rustset_framework_security::{
    CurrentUser, DataScope, Permission, PermissionSet, SecurityConfig, TokenService,
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn model(Json(body): Json<Value>) -> Response<Body> {
    if body["stream"].as_bool() == Some(true) {
        Response::builder().header(header::CONTENT_TYPE,"text/event-stream")
            .body(Body::from("data: {\"choices\":[{\"delta\":{\"content\":\"流式\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"回复\"}}]}\n\ndata: [DONE]\n\n")).unwrap()
    } else {
        Response::new(Body::from(
            json!({"choices":[{"message":{"content":"普通回复"}}],"usage":{"total_tokens":3}})
                .to_string(),
        ))
    }
}
async fn mj_imagine() -> Json<Value> {
    Json(json!({"result":"mj-task-1"}))
}
async fn mj_action() -> Json<Value> {
    Json(json!({"result":"mj-task-2"}))
}
async fn mj_fetch() -> Json<Value> {
    Json(
        json!({"status":"SUCCESS","imageUrl":"https://example.test/result.png","buttons":[{"customId":"U1","label":"放大 1"}]}),
    )
}
async fn request(app: Router, token: &str, uri: &str, value: Value) -> (StatusCode, String) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(value.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

#[tokio::test]
#[ignore = "run with script/test-ai-e2e.sh"]
async fn chat_json_and_sse_persist_complete_messages() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::var("TEST_DATABASE_URL")?;
    let pool = connect(&DatabaseConfig::new(url, 1, 5, Duration::from_secs(10))?).await?;
    migrate(&pool).await?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let model_url = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/chat/completions", post(model))
                .route("/mj/submit/imagine", post(mj_imagine))
                .route("/mj/submit/action", post(mj_action))
                .route("/mj/task/{id}/fetch", axum::routing::get(mj_fetch)),
        )
        .await
        .ok();
    });
    let now = chrono::Utc::now().timestamp_millis();
    sqlx::query("INSERT INTO ai.model_configs(id,name,key,platform,type,model,url,status,create_time,update_time)VALUES(91001,'E2E','e2e-chat','OpenAICompatible','chat','mock',$1,0,$2,$2)")
        .bind(&model_url).bind(now).execute(&pool).await?;
    let security = SecurityConfig::new(
        "integration-test-secret-at-least-32-characters",
        "test",
        "test",
        Duration::from_secs(300),
    )?;
    let tokens = TokenService::new(security);
    let token = tokens
        .issue_access_token(CurrentUser {
            user_id: "e2e-user".into(),
            username: "e2e".into(),
            tenant_id: None,
            role_codes: vec!["admin".into()],
            permissions: PermissionSet::new([Permission::new("*")?]),
            data_scope: DataScope::All,
        })
        .expect("issue E2E access token");
    let app = rustset_ai_server::routes(rustset_ai_server::AiState::new(pool.clone(), tokens))
        .finish_api(&mut aide::openapi::OpenApi::default());
    let (status, body) = request(
        app.clone(),
        &token,
        "/ai/chat/conversation/create-my",
        json!({"modelId":91001,"title":"E2E"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let conversation_id: i64 = serde_json::from_str::<Value>(&body)?["data"]
        .as_i64()
        .unwrap();
    let (status, body) = request(
        app.clone(),
        &token,
        "/ai/chat/message/send",
        json!({"conversationId":conversation_id,"content":"普通问题"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_str::<Value>(&body)?["data"]["receive"]["content"],
        "普通回复"
    );
    let (status, sse) = request(
        app.clone(),
        &token,
        "/ai/chat/message/send-stream",
        json!({"conversationId":conversation_id,"content":"流式问题"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(sse.contains("流式"));
    assert!(sse.contains("回复"));
    let stored:String=sqlx::query_scalar("SELECT content FROM ai.chat_messages WHERE conversation_id=$1 AND type='assistant' ORDER BY id DESC LIMIT 1").bind(conversation_id).fetch_one(&pool).await?;
    assert_eq!(stored, "流式回复");

    sqlx::query("INSERT INTO ai.model_configs(id,name,key,platform,type,model,url,status,create_time,update_time)VALUES(91002,'MJ E2E','e2e-mj','Midjourney','image','midjourney',$1,0,$2,$2)")
        .bind(&model_url).bind(now).execute(&pool).await?;
    let (status,body)=request(app.clone(),&token,"/ai/image/midjourney/imagine",json!({"modelId":91002,"prompt":"测试图片","base64Array":[],"width":"1024","height":"1024","version":"6.0"})).await;
    assert_eq!(status, StatusCode::OK);
    let image_id = serde_json::from_str::<Value>(&body)?["data"]
        .as_i64()
        .unwrap();
    let (status, _) = request(
        app.clone(),
        &token,
        "/ai/image/midjourney/poll",
        json!({"ids":[image_id]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let image = sqlx::query("SELECT status,pic_url,buttons FROM ai.images WHERE id=$1")
        .bind(image_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(sqlx::Row::get::<i32, _>(&image, "status"), 20);
    assert_eq!(
        sqlx::Row::get::<Value, _>(&image, "buttons")[0]["customId"],
        "U1"
    );
    let (status, body) = request(
        app.clone(),
        &token,
        "/ai/image/midjourney/action",
        json!({"id":image_id,"customId":"U1"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let action_id = serde_json::from_str::<Value>(&body)?["data"]
        .as_i64()
        .unwrap();
    let (status, _) = request(
        app,
        &token,
        "/ai/image/midjourney/poll",
        json!({"ids":[action_id]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let action = sqlx::query("SELECT status,parent_id,action_custom_id FROM ai.images WHERE id=$1")
        .bind(action_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(sqlx::Row::get::<i32, _>(&action, "status"), 20);
    assert_eq!(sqlx::Row::get::<i64, _>(&action, "parent_id"), image_id);
    assert_eq!(
        sqlx::Row::get::<String, _>(&action, "action_custom_id"),
        "U1"
    );
    Ok(())
}
