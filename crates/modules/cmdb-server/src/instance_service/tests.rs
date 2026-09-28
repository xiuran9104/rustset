use super::*;
use serde_json::json;

fn object(value: Value) -> Map<String, Value> {
    value.as_object().unwrap().clone()
}

#[test]
fn bulk_ids_reject_partial_input_and_deduplicate() {
    assert_eq!(parse_ids("3, 1,3").unwrap(), vec![1, 3]);
    for invalid in ["", "1,no", "1,", "0", "-1"] {
        assert!(parse_ids(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn batch_update_ids_are_bounded_and_deduplicated() {
    assert_eq!(normalize_batch_ids(&[3, 1, 3]).unwrap(), vec![1, 3]);
    assert!(normalize_batch_ids(&[]).is_err());
    assert!(normalize_batch_ids(&[0, 1]).is_err());
    assert!(normalize_batch_ids(&(1..=501).collect::<Vec<_>>()).is_err());
}

#[test]
fn request_envelopes_require_object_attributes_and_integer_ids() {
    use rustset_cmdb_api::{CreateInstanceRequest, UpdateInstanceRequest};
    assert!(
        serde_json::from_value::<CreateInstanceRequest>(
            json!({"modelId": 1, "attributes": {"nested": {"a": 1}}})
        )
        .is_ok()
    );
    assert!(
        serde_json::from_value::<CreateInstanceRequest>(json!({"modelId": 1, "attributes": []}))
            .is_err()
    );
    assert!(serde_json::from_value::<UpdateInstanceRequest>(json!({"attributes": {}})).is_err());
    assert!(
        serde_json::from_value::<UpdateInstanceRequest>(json!({"id": "1", "attributes": {}}))
            .is_err()
    );
}

#[test]
fn validation_preserves_patch_semantics_and_rejects_unknown_fields() {
    let definitions = vec![
        AttributeDef {
            code: "name".into(),
            attr_type: AttrType::Text,
            required: true,
            choices: None,
            default_value: None,
            expression: None,
        },
        AttributeDef {
            code: "count".into(),
            attr_type: AttrType::Number,
            required: false,
            choices: None,
            default_value: Some(json!(0)),
            expression: None,
        },
    ];
    assert!(validate_payload(&definitions, &json!({"count": 2}), true).is_err());
    assert!(validate_payload(&definitions, &json!({"name": "a", "unknown": 1}), true).is_err());
    let created = validate_payload(&definitions, &json!({"name": "a"}), true).unwrap();
    assert_eq!(created["count"], 0);
    let patch = validate_payload(&definitions, &json!({"count": 2}), false).unwrap();
    assert!(!patch.contains_key("name"));
    let clear = validate_payload(&definitions, &json!({"count": null}), false).unwrap();
    assert_eq!(clear["count"], Value::Null);
    assert!(validate_payload(&definitions, &json!({"name": ""}), false).is_err());
}

#[test]
fn validation_rejects_invalid_or_empty_required_defaults() {
    let invalid = vec![AttributeDef {
        code: "count".into(),
        attr_type: AttrType::Number,
        required: false,
        choices: None,
        default_value: Some(json!("not-a-number")),
        expression: None,
    }];
    assert!(validate_payload(&invalid, &json!({}), true).is_err());

    let empty_required = vec![AttributeDef {
        code: "name".into(),
        attr_type: AttrType::Text,
        required: true,
        choices: None,
        default_value: Some(json!("  ")),
        expression: None,
    }];
    assert!(validate_payload(&empty_required, &json!({}), true).is_err());
}

/// Uses a fresh schema, never the caller's business tables. No global migration
/// or baseline mutation is needed to exercise the transaction protocol.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
async fn concurrent_writes_and_delete_rollback() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::{
        str::FromStr,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!(
        "cmdb_test_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let options = PgConnectOptions::from_str(&url)
        .unwrap()
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    let ddl = include_str!("../../../../../sql/postgresql/0009_cmdb_core.sql")
        .split("-- Menus:")
        .next()
        .unwrap()
        .replace("public.", "");
    sqlx::raw_sql(&ddl).execute(&pool).await.unwrap();
    let migration =
        include_str!("../../../../../sql/postgresql/0025_cmdb_instance_unique_values.sql")
            .replace("public.", "");
    sqlx::raw_sql(&migration).execute(&pool).await.unwrap();
    sqlx::raw_sql("CREATE TABLE system_tenant(id bigint PRIMARY KEY); INSERT INTO system_tenant VALUES (1), (2); CREATE TABLE infra_resource_ticket(id bigint PRIMARY KEY, deleted smallint DEFAULT 0);")
        .execute(&pool).await.unwrap();
    let migration = include_str!("../../../../../sql/postgresql/0026_cmdb_tenant_isolation.sql")
        .replace("public.", "");
    sqlx::raw_sql(&migration).execute(&pool).await.unwrap();
    let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
    let model_id: i64 = sqlx::query_scalar("INSERT INTO cmdb_model(name, code, unique_key) VALUES ('Test', 'test', 'name') RETURNING id")
        .fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO cmdb_attribute(model_id, name, code, attr_type) VALUES ($1, 'Name', 'name', 'text'), ($1, 'Left', 'left', 'number'), ($1, 'Right', 'right', 'number')")
        .bind(model_id).execute(&pool).await.unwrap();

    // A separate transaction owns the model: a writer must wait for it.
    let mut guard = pool.begin().await.unwrap();
    lock_model(&mut guard, model_id).await.unwrap();
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            create(
                &pool,
                &tenant,
                model_id,
                object(json!({"name": "blocked"})),
                "test"
            )
        )
        .await
        .is_err()
    );
    guard.rollback().await.unwrap();

    let (first, second) = tokio::join!(
        create(
            &pool,
            &tenant,
            model_id,
            object(json!({"name": "same"})),
            "first"
        ),
        create(
            &pool,
            &tenant,
            model_id,
            object(json!({"name": "same"})),
            "second"
        ),
    );
    assert_ne!(
        first.is_ok(),
        second.is_ok(),
        "exactly one duplicate create commits"
    );
    let id = first.or(second).unwrap();
    let (left, right) = tokio::join!(
        update(&pool, &tenant, id, object(json!({"left": 1})), "left"),
        update(&pool, &tenant, id, object(json!({"right": 2})), "right"),
    );
    left.unwrap();
    right.unwrap();
    let attributes: Value =
        sqlx::query_scalar("SELECT attributes FROM cmdb_instance WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(attributes, json!({"name": "same", "left": 1, "right": 2}));
    let other = create(
        &pool,
        &tenant,
        model_id,
        object(json!({"name": "other", "left": 1})),
        "test",
    )
    .await
    .unwrap();
    let summary = batch_update(&pool, &tenant, &[id, other], object(json!({"right": 9})), "batch")
        .await
        .unwrap();
    assert_eq!(summary.updated, 2);
    assert_eq!(summary.unchanged, 0);
    assert!(
        batch_update(
            &pool,
            &tenant,
            &[id, other],
            object(json!({"name": "duplicate"})),
            "batch"
        )
        .await
        .is_err()
    );
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT attributes->>'name' FROM cmdb_instance WHERE id = ANY($1) ORDER BY id",
    )
    .bind(vec![id, other])
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(names, vec!["same", "other"]);
    let before_atomic: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_instance WHERE deleted = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        create_many(
            &pool,
            &tenant,
            model_id,
            vec![
                (2, object(json!({"name": "atomic-good"}))),
                (3, object(json!({"name": "same"}))),
            ],
            "atomic",
        )
        .await
        .is_err()
    );
    let after_atomic: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_instance WHERE deleted = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        before_atomic, after_atomic,
        "atomic import must roll back all rows"
    );
    assert!(
        update(
            &pool,
            &tenant,
            other,
            object(json!({"name": "same"})),
            "test"
        )
        .await
        .is_err()
    );
    let mut tx = pool.begin().await.unwrap();
    lock_model(&mut tx, model_id).await.unwrap();
    assert!(
        validate_unique_key_change(&mut tx, model_id, Some("left"))
            .await
            .is_err()
    );
    tx.rollback().await.unwrap();

    // The database protects direct SQL writers as well as the service.
    let duplicate = sqlx::query(
        "INSERT INTO cmdb_instance(model_id, tenant_id, attributes) VALUES ($1, 1, '{\"name\":\"same\"}')",
    )
    .bind(model_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        duplicate.as_database_error().unwrap().constraint(),
        Some("cmdb_instance_unique_value_key")
    );
    let key_change = sqlx::query("UPDATE cmdb_model SET unique_key = 'left' WHERE id = $1")
        .bind(model_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        key_change.as_database_error().unwrap().constraint(),
        Some("cmdb_instance_unique_value_key")
    );
    let key: String = sqlx::query_scalar("SELECT unique_key FROM cmdb_model WHERE id = $1")
        .bind(model_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(key, "name");

    // Concurrent direct inserts cannot bypass the registry, even without API locks.
    let insert = || {
        sqlx::query("INSERT INTO cmdb_instance(model_id, tenant_id, attributes) VALUES ($1, 1, $2)")
            .bind(model_id)
            .bind(json!({"name": "direct"}))
            .execute(&pool)
    };
    let (a, b) = tokio::join!(insert(), insert());
    assert_ne!(a.is_ok(), b.is_ok());
    // JSONB equality treats numerically equivalent values as the same key.
    sqlx::query("INSERT INTO cmdb_instance(model_id, tenant_id, attributes) VALUES ($1, 1, $2)")
        .bind(model_id)
        .bind(json!({"name": 42}))
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        sqlx::query(
            "INSERT INTO cmdb_instance(model_id, tenant_id, attributes) VALUES ($1, 1, $2)"
        )
        .bind(model_id)
        .bind(json!({"name": 42.0}))
        .execute(&pool)
        .await
        .is_err()
    );
    for value in [Value::Null, json!(" "), json!([])] {
        for _ in 0..2 {
            sqlx::query(
                "INSERT INTO cmdb_instance(model_id, tenant_id, attributes) VALUES ($1, 1, $2)",
            )
            .bind(model_id)
            .bind(json!({"name": value}))
            .execute(&pool)
            .await
            .unwrap();
        }
    }
    // Rerunning the migration reconstructs the same registrations.
    sqlx::raw_sql(&migration).execute(&pool).await.unwrap();

    // Force relation cleanup to fail: the instance deletion must roll back.
    sqlx::query("INSERT INTO cmdb_relation(source_id, target_id, tenant_id) VALUES ($1, $2, 1)")
        .bind(id)
        .bind(other)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_detach() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'test failure'; END $$; CREATE TRIGGER reject_detach BEFORE UPDATE ON cmdb_relation FOR EACH ROW EXECUTE FUNCTION reject_detach();")
        .execute(&pool).await.unwrap();
    assert!(delete(&pool, &tenant, &[id], "test", true).await.is_err());
    let deleted: i16 = sqlx::query_scalar("SELECT deleted FROM cmdb_instance WHERE id = $1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deleted, 0);
    sqlx::query("DROP TRIGGER reject_detach ON cmdb_relation")
        .execute(&pool)
        .await
        .unwrap();
    delete(&pool, &tenant, &[id], "test", true).await.unwrap();
    let active_relations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_relation WHERE deleted = 0")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_relations, 0);
    create(
        &pool,
        &tenant,
        model_id,
        object(json!({"name": "same"})),
        "test",
    )
    .await
    .unwrap();

    let restore = sqlx::query("UPDATE cmdb_instance SET deleted = 0 WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert_eq!(
        restore.as_database_error().unwrap().constraint(),
        Some("cmdb_instance_unique_value_key")
    );
    sqlx::query("UPDATE cmdb_model SET unique_key = NULL WHERE id = $1")
        .bind(model_id)
        .execute(&pool)
        .await
        .unwrap();
    let registrations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cmdb_instance_unique_value WHERE model_id = $1")
            .bind(model_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(registrations, 0);
    sqlx::query("UPDATE cmdb_model SET unique_key = 'name' WHERE id = $1")
        .bind(model_id)
        .execute(&pool)
        .await
        .unwrap();

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
