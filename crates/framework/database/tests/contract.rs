//! 数据库合同测试（docs/xinchuang-adaptation-plan.md §4.1.7、§6.1 第二层）。
//!
//! 以下不变量必须在四种数据库上分别证明：同一套测试合同跑在
//! PostgreSQL（本文件默认目标）与金仓 / openGauss / 达梦适配器上，
//! 通过 `script/test-database-migrations.sh --db <flavor>` 或对真实
//! 国产库设置 `TEST_DATABASE_URL` 后执行：
//!
//! ```bash
//! TEST_DATABASE_URL='postgres://…' \
//!   cargo test -p rustset-framework-database --test contract -- --ignored
//! ```
//!
//! 测试使用自带临时表，不触碰业务表；连接中断与恢复（§4.1.7.5）属于
//! 故障注入验收（§6.3），不在单测内模拟。
//!
//! 未覆盖的不变量（达梦适配器实装时补充）：
//! * §4.1.7.5 连接中断不伪造写入成功 —— 故障注入场景。

use std::sync::Arc;
use std::time::Duration;

use rustset_framework_database::{DatabaseConfig, DbError, DbPool};
use sqlx::Row;

/// 每个测试独立建表，互不干扰（cargo 默认并行执行）。
async fn scratch(pool: &DbPool, ddl: &'static str) {
    // 多语句 DDL 走简单协议（prepared statement 不允许多命令）。
    sqlx::raw_sql(ddl)
        .execute(pool)
        .await
        .expect("create scratch tables");
}

async fn test_pool(max_connections: u32) -> DbPool {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let config = DatabaseConfig::new(url, 1, max_connections, Duration::from_secs(10))
        .expect("valid test database config");
    rustset_framework_database::connect(&config)
        .await
        .expect("connect test database")
}

/// §4.1.7.1 多步写入在同一事务中提交；部分失败完整回滚，不出现半提交。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn transaction_rollback_is_atomic() {
    let pool = test_pool(2).await;
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_rb;
         CREATE TABLE contract_rb(id bigint PRIMARY KEY, tag text NOT NULL)",
    )
    .await;

    // 成功路径：两步写入一起提交。
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("INSERT INTO contract_rb(id, tag) VALUES (1, 'a'), (2, 'b')")
        .execute(&mut *tx)
        .await
        .expect("insert pair");
    tx.commit().await.expect("commit pair");
    let committed: i64 = sqlx::query_scalar("SELECT count(*) FROM contract_rb")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(committed, 2);

    // 失败路径：同事务第二步违反主键，回滚后第一步的写入也必须消失。
    let mut tx = pool.begin().await.expect("begin failing tx");
    sqlx::query("INSERT INTO contract_rb(id, tag) VALUES (3, 'c')")
        .execute(&mut *tx)
        .await
        .expect("first step succeeds");
    let failure = sqlx::query("INSERT INTO contract_rb(id, tag) VALUES (3, 'dup')")
        .execute(&mut *tx)
        .await;
    assert!(failure.is_err(), "duplicate insert must fail");
    tx.rollback().await.expect("rollback");
    let after_rollback: i64 = sqlx::query_scalar("SELECT count(*) FROM contract_rb")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after_rollback, 2, "partial writes must not survive rollback");
}

/// §4.1.7.2 `FOR UPDATE` 事务流并发互斥：第二个写者在锁释放前被阻塞，
/// 释放后读到第一个事务已提交的值（读已提交语义）。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn for_update_serializes_concurrent_writers() {
    let pool = Arc::new(test_pool(3).await);
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_fu_state;
         DROP TABLE IF EXISTS contract_fu;
         CREATE TABLE contract_fu(id bigint PRIMARY KEY, n bigint NOT NULL);
         CREATE TABLE contract_fu_state(phase text NOT NULL);
         INSERT INTO contract_fu VALUES (1, 0), (2, 0);
         INSERT INTO contract_fu_state VALUES ('init')",
    )
    .await;

    // 写者 A：先锁行，再通过状态表（同事务）宣告“已持锁”，随后持锁
    // 400ms 完成写入并提交。
    let writer_a = Arc::clone(&pool);
    let task_a = tokio::spawn(async move {
        let mut tx = writer_a.begin().await.expect("A begin");
        let n: i64 =
            sqlx::query_scalar("SELECT n FROM contract_fu WHERE id = 1 FOR UPDATE")
                .fetch_one(&mut *tx)
                .await
                .expect("A lock row");
        // 通过独立自动提交连接宣告持锁（事务内写入要提交后才可见，
        // 就无法证明等待方真正被行锁阻塞）。
        sqlx::query("UPDATE contract_fu_state SET phase = 'holding'")
            .execute(writer_a.as_ref())
            .await
            .expect("A announce");
        tokio::time::sleep(Duration::from_millis(400)).await;
        sqlx::query("UPDATE contract_fu SET n = $1 WHERE id = 1")
            .bind(n + 1)
            .execute(&mut *tx)
            .await
            .expect("A write");
        tx.commit().await.expect("A commit");
    });

    // 写者 B：等到 A 宣告持锁后再抢同一行 —— 必须阻塞到 A 提交，
    // 然后在读已提交语义下看到 A 的写入。
    let writer_b = Arc::clone(&pool);
    let task_b = tokio::spawn(async move {
        loop {
            let phase: String =
                sqlx::query_scalar("SELECT phase FROM contract_fu_state LIMIT 1")
                    .fetch_one(writer_b.as_ref())
                    .await
                    .expect("poll phase");
            if phase == "holding" {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let mut tx = writer_b.begin().await.expect("B begin");
        let n: i64 =
            sqlx::query_scalar("SELECT n FROM contract_fu WHERE id = 1 FOR UPDATE")
                .fetch_one(&mut *tx)
                .await
                .expect("B blocked until A committed, then locked");
        tx.commit().await.expect("B commit");
        n
    });

    task_a.await.expect("writer A");
    let observed_by_b = task_b.await.expect("writer B");
    assert_eq!(observed_by_b, 1, "B must observe A's committed write");

    // 行 2 未被触碰，证明锁定范围正确。
    let untouched: i64 = sqlx::query_scalar("SELECT n FROM contract_fu WHERE id = 2")
        .fetch_one(pool.as_ref())
        .await
        .unwrap();
    assert_eq!(untouched, 0);
}

/// §4.1.7.2 锁等待超时：锁持有者阻塞期间，等待方在 lock_timeout 到期后
/// 收到统一分类的 `DbError::LockWaitTimeout`，而不是内部错误。
///
/// openGauss 不提供 `lock_timeout` GUC（§4.1.8 已登记的平台差异），
/// 该不变量在 og 上无法用同一方法证明，跳过并留痕。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn lock_wait_timeout_maps_to_db_error() {
    if rustset_framework_database::DatabaseFlavor::detect()
        == rustset_framework_database::DatabaseFlavor::OpenGauss
    {
        eprintln!(
            "SKIP (openGauss): no lock_timeout GUC — 锁等待超时不变量在 og 上另行登记，\
             依赖死锁检测与语句超时（statement_timeout）路径"
        );
        return;
    }
    let pool = Arc::new(test_pool(3).await);
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_lt_state;
         DROP TABLE IF EXISTS contract_lt;
         CREATE TABLE contract_lt(id bigint PRIMARY KEY, n bigint NOT NULL);
         CREATE TABLE contract_lt_state(phase text NOT NULL);
         INSERT INTO contract_lt VALUES (1, 0);
         INSERT INTO contract_lt_state VALUES ('init')",
    )
    .await;

    let holder = Arc::clone(&pool);
    let task_holder = tokio::spawn(async move {
        let mut tx = holder.begin().await.expect("holder begin");
        sqlx::query("SELECT n FROM contract_lt WHERE id = 1 FOR UPDATE")
            .fetch_one(&mut *tx)
            .await
            .expect("holder lock");
        // 独立自动提交连接宣告持锁，等待方立即可见（见 for_update 同注）。
        sqlx::query("UPDATE contract_lt_state SET phase = 'holding'")
            .execute(holder.as_ref())
            .await
            .expect("holder announce");
        tokio::time::sleep(Duration::from_millis(500)).await;
        tx.commit().await.expect("holder commit");
    });

    loop {
        let phase: String = sqlx::query_scalar("SELECT phase FROM contract_lt_state LIMIT 1")
            .fetch_one(pool.as_ref())
            .await
            .unwrap();
        if phase == "holding" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let waiter_error = async {
        let mut tx = pool.begin().await.expect("waiter begin");
        sqlx::query("SET LOCAL lock_timeout = '150ms'")
            .execute(&mut *tx)
            .await
            .expect("set lock timeout");
        let result: Result<i64, sqlx::Error> =
            sqlx::query_scalar("SELECT n FROM contract_lt WHERE id = 1 FOR UPDATE")
                .fetch_one(&mut *tx)
                .await;
        result.expect_err("waiter must fail with lock timeout")
    }
    .await;

    task_holder.await.expect("holder finishes");
    let classified = DbError::from(waiter_error);
    assert!(
        matches!(classified, DbError::LockWaitTimeout),
        "lock timeout must classify as DbError::LockWaitTimeout, got: {classified}"
    );
    assert!(!classified.is_client_error());
}

/// §4.1.7.3 唯一约束冲突映射为业务可识别的冲突错误（客户端输入类）。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn unique_violation_classifies_as_client_error() {
    let pool = test_pool(1).await;
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_uq;
         CREATE TABLE contract_uq(id bigint PRIMARY KEY, code text NOT NULL UNIQUE)",
    )
    .await;

    sqlx::query("INSERT INTO contract_uq VALUES (1, 'same')")
        .execute(&pool)
        .await
        .expect("first insert");
    let error = sqlx::query("INSERT INTO contract_uq VALUES (2, 'same')")
        .execute(&pool)
        .await
        .expect_err("duplicate insert must fail");

    let classified = DbError::from(error);
    assert!(matches!(classified, DbError::UniqueViolation { .. }));
    assert!(classified.is_client_error());
    // 约束名依赖服务器错误响应的 constraint 字段；openGauss 不回传
    // （§4.1.8 登记的平台差异），仅 PG 家族成员断言名字透传。
    if rustset_framework_database::DatabaseFlavor::detect()
        != rustset_framework_database::DatabaseFlavor::OpenGauss
    {
        let constraint = classified.constraint().expect("constraint name reported");
        assert!(constraint.contains("code"), "constraint is {constraint}");
    }
}

/// §4.1.7.3 外键冲突同样收敛到统一分类。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn foreign_key_violation_classifies_as_client_error() {
    let pool = test_pool(1).await;
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_fk_child;
         DROP TABLE IF EXISTS contract_fk_parent;
         CREATE TABLE contract_fk_parent(id bigint PRIMARY KEY);
         CREATE TABLE contract_fk_child(
             id bigint PRIMARY KEY,
             pid bigint NOT NULL REFERENCES contract_fk_parent(id))",
    )
    .await;

    sqlx::query("INSERT INTO contract_fk_parent VALUES (1)")
        .execute(&pool)
        .await
        .expect("parent");
    let error = sqlx::query("INSERT INTO contract_fk_child VALUES (1, 999)")
        .execute(&pool)
        .await
        .expect_err("orphan insert must fail");

    let classified = DbError::from(error);
    assert!(matches!(classified, DbError::ForeignKeyViolation { .. }));
    assert!(classified.is_client_error());
}

/// §4.1.7.4 隔离级别在各库显式声明：串行化事务正常提交。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn explicit_isolation_level_is_honored() {
    let pool = test_pool(1).await;
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_iso;
         CREATE TABLE contract_iso(id bigint PRIMARY KEY, n bigint NOT NULL)",
    )
    .await;

    {
        // 连接作用域收窄：max=1 的池在连接归还前无法服务后续查询。
        let mut connection = pool.acquire().await.expect("acquire");
        sqlx::query("BEGIN ISOLATION LEVEL SERIALIZABLE")
            .execute(&mut *connection)
            .await
            .expect("begin serializable");
        sqlx::query("INSERT INTO contract_iso VALUES (1, 1)")
            .execute(&mut *connection)
            .await
            .expect("write inside serializable tx");
        sqlx::query("COMMIT")
            .execute(&mut *connection)
            .await
            .expect("commit serializable tx");
    }

    let committed: i64 = sqlx::query_scalar("SELECT count(*) FROM contract_iso")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(committed, 1);
}

/// `DbRow` 读取面：类型集覆盖 i64/String/bool/f64/时间/JSON（§4.1.1）。
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn row_reading_covers_the_facade_type_set() {
    let pool = test_pool(1).await;
    scratch(
        &pool,
        "DROP TABLE IF EXISTS contract_row;
         CREATE TABLE contract_row(
             n bigint NOT NULL, label text NOT NULL, flag boolean NOT NULL,
             ratio double precision NOT NULL, seen timestamp NOT NULL,
             payload text NOT NULL)",
    )
    .await;

    sqlx::query("INSERT INTO contract_row VALUES (42, '文本', true, 0.5, '2026-01-02 03:04:05', '{\"a\":1}')")
        .execute(&pool)
        .await
        .expect("seed row");

    let row = sqlx::query("SELECT * FROM contract_row")
        .fetch_one(&pool)
        .await
        .expect("fetch");
    assert_eq!(row.get::<i64, _>("n"), 42);
    assert_eq!(row.get::<String, _>("label"), "文本");
    assert!(row.get::<bool, _>("flag"));
    assert!((row.get::<f64, _>("ratio") - 0.5).abs() < f64::EPSILON);
    let _seen = row.get::<chrono::NaiveDateTime, _>("seen");
    let payload: String = row.get("payload");
    let parsed: serde_json::Value = serde_json::from_str(&payload).expect("payload is JSON");
    assert_eq!(parsed["a"], 1);
}
