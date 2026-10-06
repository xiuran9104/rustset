use super::*;

#[test]
fn bulk_ids_preserve_uuid_and_legacy_string_ids() {
    let uuid = "9b07ef8d-27f7-4485-85cd-c9f4e900a87b";
    assert_eq!(
        parse_task_ids(&format!(" {uuid},legacy,{uuid} ")).unwrap(),
        vec![uuid, "legacy"]
    );
    for invalid in ["", " ", "id,", ",id", "a,,b"] {
        assert!(parse_task_ids(invalid).is_err());
    }
}

#[test]
fn scan_submission_normalizes_targets_ports_and_policies() {
    assert_eq!(
        parse_targets("127.0.0.1, ::1;127.0.0.1").unwrap(),
        vec![
            "127.0.0.1".parse::<std::net::IpAddr>().unwrap(),
            "::1".parse::<std::net::IpAddr>().unwrap(),
        ]
    );
    assert!(parse_targets("10.0.0.0/24").is_err());
    assert_eq!(normalize_ports(vec![443, 22, 443]).unwrap(), vec![22, 443]);
    assert!(normalize_ports(vec![0]).is_err());
    assert_eq!(ports_for_policy("COMMON").unwrap().len(), 28);
    assert_eq!(ports_for_policy("TOP100").unwrap().len(), 28);
    assert!(ports_for_policy("TOP1000").unwrap().len() > 1_000);
    assert_eq!(ports_for_policy("ALL").unwrap().len(), 65_535);
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
async fn bulk_delete_is_atomic_and_retryable() {
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::str::FromStr;
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let admin = sqlx::PgPool::connect(&url).await.unwrap();
    let schema = format!("task_test_{}", Uuid::new_v4().simple());
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
    sqlx::raw_sql("CREATE TABLE infra_task(id text PRIMARY KEY, tenant_id bigint NOT NULL, deleted smallint DEFAULT 0, cancel_requested boolean DEFAULT false, update_time timestamp DEFAULT now());
        INSERT INTO infra_task(id,tenant_id) VALUES ('first',1), ('second',1);
        CREATE FUNCTION reject_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.id = 'second' THEN RAISE EXCEPTION 'test failure'; END IF; RETURN NEW; END $$;
        CREATE TRIGGER reject_delete BEFORE UPDATE ON infra_task FOR EACH ROW EXECUTE FUNCTION reject_delete();")
        .execute(&pool).await.unwrap();
    let ids = vec!["first".into(), "second".into()];
    let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
    assert!(delete_tasks(&pool, &tenant, &ids).await.is_err());
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_task WHERE deleted = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(active, 2);
    sqlx::query("DROP TRIGGER reject_delete ON infra_task")
        .execute(&pool)
        .await
        .unwrap();
    delete_tasks(&pool, &tenant, &ids).await.unwrap();
    delete_tasks(&pool, &tenant, &ids).await.unwrap();
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_task WHERE deleted = 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(active, 0);
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
}
