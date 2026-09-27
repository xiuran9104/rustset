use super::*;
use rustset_framework_security::{DataScope, Permission, PermissionSet};

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL and PostgreSQL CREATEDB permission"]
async fn tool_queries_use_authenticated_tenant_and_permissions() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::{
        str::FromStr,
        time::{SystemTime, UNIX_EPOCH},
    };
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let database = format!(
        "ai_tenant_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    sqlx::query(&format!(
        "CREATE DATABASE {database} TEMPLATE template0 ENCODING 'UTF8'"
    ))
    .execute(&admin)
    .await
    .unwrap();
    let pool = PgPoolOptions::new()
        .connect_with(
            PgConnectOptions::from_str(&url)
                .unwrap()
                .database(&database),
        )
        .await
        .unwrap();
    sqlx::raw_sql("CREATE SCHEMA ai; CREATE TABLE ai.tools(name text, status int);
        INSERT INTO ai.tools VALUES ('cmdb_instance_query',1),('cmdb_model_list',1);
        CREATE TABLE cmdb_model(id bigint,name text,code text,description text,deleted int,status int,sort int);
        INSERT INTO cmdb_model VALUES(1,'test','test',NULL,0,0,0);
        CREATE TABLE cmdb_attribute(model_id bigint,deleted int);
        CREATE TABLE cmdb_instance(id bigint,model_id bigint,tenant_id bigint,attributes jsonb,deleted int,update_time timestamp DEFAULT now());
        INSERT INTO cmdb_instance(id,model_id,tenant_id,attributes,deleted) VALUES (1,1,1,'{\"name\":\"one\"}',0),(2,1,2,'{\"name\":\"two\"}',0),(3,1,NULL,'{\"name\":\"quarantine\"}',0);")
        .execute(&pool).await.unwrap();
    let mut user = CurrentUser {
        user_id: "u".into(),
        username: "u".into(),
        tenant_id: Some("1".into()),
        role_codes: vec!["super_admin".into()],
        permissions: PermissionSet::new([Permission::new("*").unwrap()]),
        data_scope: DataScope::All,
    };
    let args = json!({"model_code":"test","tenantId":2,"tenant_id":2});
    let result: Value = serde_json::from_str(
        &execute(&pool, &user, "cmdb_instance_query", &args)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["instances"].as_array().unwrap().len(), 1);
    assert_eq!(result["instances"][0]["attributes"]["name"], "one");
    let models: Value = serde_json::from_str(
        &execute(&pool, &user, "cmdb_model_list", &json!({}))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(models["models"][0]["instances"], 1);
    user.tenant_id = Some("2".into());
    let result: Value = serde_json::from_str(
        &execute(&pool, &user, "cmdb_instance_query", &args)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(result["instances"][0]["attributes"]["name"], "two");
    user.tenant_id = None;
    assert!(
        execute(&pool, &user, "cmdb_instance_query", &args)
            .await
            .is_err()
    );
    user.tenant_id = Some("1".into());
    user.permissions = PermissionSet::default();
    assert!(
        execute(&pool, &user, "cmdb_instance_query", &args)
            .await
            .is_err()
    );
    pool.close().await;
    sqlx::query(&format!("DROP DATABASE {database}"))
        .execute(&admin)
        .await
        .unwrap();
}
