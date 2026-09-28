use super::*;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use rustset_framework_security::{DataScope, PermissionSet};
use tower::ServiceExt;

fn user(tenant: Option<i64>) -> CurrentUser {
    CurrentUser {
        user_id: "test".into(),
        username: "test".into(),
        tenant_id: tenant.map(|id| id.to_string()),
        role_codes: vec!["super_admin".into()],
        permissions: PermissionSet::default(),
        data_scope: DataScope::All,
    }
}

async fn request(
    app: &Router,
    tenant: Option<i64>,
    method: &str,
    uri: &str,
    content_type: &str,
    body: Vec<u8>,
) -> (StatusCode, Vec<u8>) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", content_type)
                .header("tenant-id", "2")
                .header("visit-tenant-id", "2")
                .extension(user(tenant))
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    (
        response.status(),
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
}
async fn json_request(
    app: &Router,
    tenant: Option<i64>,
    method: &str,
    uri: &str,
    value: Value,
) -> (StatusCode, Value) {
    let (status, bytes) = request(
        app,
        tenant,
        method,
        uri,
        "application/json",
        value.to_string().into_bytes(),
    )
    .await;
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
async fn all_instance_paths_enforce_tenant_and_quarantine() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::{
        str::FromStr,
        time::{SystemTime, UNIX_EPOCH},
    };
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!(
        "tenant_test_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .connect_with(
            PgConnectOptions::from_str(&url)
                .unwrap()
                .options([("search_path", schema.as_str())]),
        )
        .await
        .unwrap();
    let ddl = include_str!("../../../../sql/postgresql/0009_cmdb_core.sql")
        .split("-- Menus:")
        .next()
        .unwrap()
        .replace("public.", "");
    sqlx::raw_sql(&ddl).execute(&pool).await.unwrap();
    sqlx::raw_sql("CREATE TABLE system_tenant(id bigint PRIMARY KEY); INSERT INTO system_tenant VALUES (1), (2); CREATE TABLE infra_resource_ticket(id bigint PRIMARY KEY, deleted smallint DEFAULT 0);
        INSERT INTO cmdb_model(id,name,code,unique_key) VALUES (1,'Test','test','name');
        INSERT INTO cmdb_attribute(model_id,name,code,attr_type) VALUES (1,'Name','name','text');
        INSERT INTO cmdb_instance(model_id,attributes) VALUES (1,'{\"name\":\"historical\"}');")
        .execute(&pool).await.unwrap();
    for migration in [
        include_str!("../../../../sql/postgresql/0025_cmdb_instance_unique_values.sql"),
        include_str!("../../../../sql/postgresql/0026_cmdb_tenant_isolation.sql"),
    ] {
        sqlx::raw_sql(&migration.replace("public.", ""))
            .execute(&pool)
            .await
            .unwrap();
    }
    let app =
        routes(CmdbState { pool: pool.clone() }).finish_api(&mut aide::openapi::OpenApi::default());
    assert_eq!(
        json_request(&app, Some(1), "GET", "/cmdb/instance/get?id=1", json!(null))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            None,
            "GET",
            "/cmdb/instance/page?modelId=1",
            json!(null)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let create = json!({"modelId":1,"tenantId":2,"attributes":{"name":"same"}});
    let (status, one) = json_request(
        &app,
        Some(1),
        "POST",
        "/cmdb/instance/create",
        create.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let one: i64 = one["data"].as_str().unwrap().parse().unwrap();
    let (status, two) = json_request(&app, Some(2), "POST", "/cmdb/instance/create", create).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "different tenants can reuse a unique value"
    );
    let two: i64 = two["data"].as_str().unwrap().parse().unwrap();
    let owner: i64 = sqlx::query_scalar("SELECT tenant_id FROM cmdb_instance WHERE id=$1")
        .bind(one)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(owner, 1, "spoofed body and headers cannot select a tenant");
    for tenant in [1, 2] {
        let (status, page) = json_request(
            &app,
            Some(tenant),
            "GET",
            "/cmdb/instance/page?modelId=1",
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(page["data"]["total"], 1);
    }
    let models = json_request(&app, Some(1), "GET", "/cmdb/model/list", json!(null))
        .await
        .1;
    assert_eq!(models["data"][0]["instanceCount"], 1);
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "GET",
            &format!("/cmdb/instance/get?id={one}"),
            json!(null)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "PUT",
            "/cmdb/instance/update",
            json!({"id":one,"attributes":{"name":"stolen"}})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "DELETE",
            &format!("/cmdb/instance/delete?id={one}"),
            json!(null)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            Some(1),
            "POST",
            "/cmdb/relation/bind",
            json!({"sourceId":one,"targetId":two})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert!(
        sqlx::query("INSERT INTO cmdb_relation(tenant_id,source_id,target_id) VALUES (1,$1,$2)")
            .bind(one)
            .bind(two)
            .execute(&pool)
            .await
            .is_err()
    );
    let (status, bytes) = request(
        &app,
        Some(1),
        "GET",
        "/cmdb/instance/export?model_id=1",
        "application/json",
        vec![],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let exported = rustset_framework_common::csv::decode(&bytes).unwrap();
    assert_eq!(exported.headers, vec!["name"]);
    assert_eq!(exported.rows.len(), 1);
    // Import is scoped by authentication, too, including an untrusted tenant field.
    let bytes =
        rustset_framework_common::csv::encode(&["name"], &[vec!["imported".to_owned()]]).unwrap();
    let mut multipart = b"--boundary\r\nContent-Disposition: form-data; name=\"modelId\"\r\n\r\n1\r\n--boundary\r\nContent-Disposition: form-data; name=\"tenantId\"\r\n\r\n2\r\n--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"test.csv\"\r\nContent-Type: text/csv\r\n\r\n".to_vec();
    multipart.extend(bytes);
    multipart.extend(b"\r\n--boundary--\r\n");
    for tenant in [1, 2] {
        let (status, result) = request(
            &app,
            Some(tenant),
            "POST",
            "/cmdb/instance/import",
            "multipart/form-data; boundary=boundary",
            multipart.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<Value>(&result).unwrap()["data"]["created"],
            1
        );
    }
    let imported: i64 = sqlx::query_scalar(
        "SELECT id FROM cmdb_instance WHERE tenant_id=1 AND attributes->>'name'='imported'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let (status, relation) = json_request(
        &app,
        Some(1),
        "POST",
        "/cmdb/relation/bind",
        json!({"sourceId":one,"targetId":imported}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let relation = relation["data"].as_str().unwrap();
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "GET",
            &format!("/cmdb/relation/list-by-instance?instanceId={one}"),
            json!(null)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "DELETE",
            &format!("/cmdb/relation/unbind?instanceId={relation}"),
            json!(null)
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        json_request(
            &app,
            Some(1),
            "DELETE",
            &format!("/cmdb/instance/delete-list?ids={one},{two},1"),
            json!(null)
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        json_request(
            &app,
            Some(2),
            "GET",
            &format!("/cmdb/instance/get?id={two}"),
            json!(null)
        )
        .await
        .0,
        StatusCode::OK
    );
    let historical: (Option<i64>, i16) =
        sqlx::query_as("SELECT tenant_id,deleted FROM cmdb_instance WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(historical, (None, 0));
    let active: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_relation WHERE tenant_id=1 AND deleted=0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active, 0);
    assert!(
        sqlx::query("UPDATE cmdb_instance SET tenant_id=2 WHERE id=$1")
            .bind(imported)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
}
