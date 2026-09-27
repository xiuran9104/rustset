use serde::Deserialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use std::{net::IpAddr, sync::Arc, time::Duration};
use tokio::{sync::Semaphore, task::JoinSet, time};
use uuid::Uuid;

const LEASE_SECONDS: i64 = 30;
const PROBE_TIMEOUT_MS: u64 = 800;
const PROBES_PER_BATCH: usize = 64;

#[derive(Debug, FromRow)]
struct Job {
    id: String,
    tenant_id: i64,
    task_kind: String,
    payload: Value,
    attempt_count: i32,
    max_attempts: i32,
    timeout_seconds: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanPayload {
    target_ips: Vec<IpAddr>,
    ports: Vec<i32>,
}

pub(super) fn start(pool: PgPool) {
    let concurrency = std::env::var("SCAN_WORKER_CONCURRENCY")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(4)
        .clamp(1, 32);
    let worker_id = format!("{}-{}", std::process::id(), Uuid::new_v4());
    tokio::spawn(async move {
        let slots = Arc::new(Semaphore::new(concurrency));
        let mut poll_tick = time::interval(Duration::from_secs(1));
        let mut polls = 0_u8;
        loop {
            poll_tick.tick().await;
            if polls == 0 {
                if let Err(error) = recover_expired(&pool).await {
                    tracing::error!(?error, "failed to recover expired scan leases");
                }
            }
            polls = (polls + 1) % 10;
            while let Ok(permit) = slots.clone().try_acquire_owned() {
                let job = match claim(&pool, &worker_id).await {
                    Ok(job) => job,
                    Err(error) => {
                        tracing::error!(?error, "failed to claim scan task");
                        drop(permit);
                        break;
                    }
                };
                let Some(job) = job else {
                    drop(permit);
                    break;
                };
                let job_pool = pool.clone();
                let owner = worker_id.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    run_claimed(job_pool, owner, job).await;
                });
            }
        }
    });
}

async fn claim(pool: &PgPool, worker_id: &str) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as::<_, Job>(
        "WITH candidate AS (
             SELECT id FROM infra_task
             WHERE task_kind IN ('scan','inspection') AND status IN ('queued','retrying')
               AND deleted=0 AND cancel_requested=false AND next_attempt_at <= now()
             ORDER BY next_attempt_at,create_time
             FOR UPDATE SKIP LOCKED LIMIT 1
         )
         UPDATE infra_task task SET status='running',lease_owner=$1,
             lease_expires_at=now()+make_interval(secs => $2),heartbeat_at=now(),
             attempt_count=attempt_count+1,start_time=COALESCE(start_time,now()::text),
             end_time=NULL,error_message=NULL,completed_targets=0,
             found_assets=0,found_risks=0,update_time=now()
         FROM candidate WHERE task.id=candidate.id
         RETURNING task.id,task.tenant_id,task.task_kind,task.payload,task.attempt_count,
             task.max_attempts,task.timeout_seconds",
    )
    .bind(worker_id)
    .bind(LEASE_SECONDS as i32)
    .fetch_optional(pool)
    .await
}

async fn recover_expired(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE infra_task SET
             status=CASE
                 WHEN cancel_requested THEN 'cancelled'
                 WHEN attempt_count >= max_attempts THEN 'failed'
                 ELSE 'retrying' END,
             error_message=CASE
                 WHEN cancel_requested THEN '任务已取消'
                 WHEN attempt_count >= max_attempts THEN '执行租约过期，已达到最大重试次数'
                 ELSE '执行租约过期，等待重试' END,
             next_attempt_at=CASE
                 WHEN attempt_count < max_attempts AND NOT cancel_requested
                 THEN now()+interval '5 seconds' ELSE next_attempt_at END,
             end_time=CASE
                 WHEN cancel_requested OR attempt_count >= max_attempts THEN now()::text
                 ELSE NULL END,
             lease_owner=NULL,lease_expires_at=NULL,update_time=now()
         WHERE task_kind IN ('scan','inspection') AND status='running' AND deleted=0
           AND lease_expires_at < now()",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn run_claimed(pool: PgPool, owner: String, job: Job) {
    let timeout = Duration::from_secs(job.timeout_seconds.max(1) as u64);
    let outcome = time::timeout(timeout, execute_job(&pool, &owner, &job)).await;
    let result = match outcome {
        Ok(result) => result,
        Err(_) => Err("任务执行超时".to_owned()),
    };
    match result {
        Ok((assets, risks)) => {
            let completed = sqlx::query(
                "UPDATE infra_task SET status='completed',found_assets=$3,found_risks=$4,
                     end_time=now()::text,lease_owner=NULL,lease_expires_at=NULL,
                     heartbeat_at=now(),error_message=NULL,update_time=now()
                 WHERE id=$1 AND lease_owner=$2 AND status='running'
                   AND cancel_requested=false",
            )
            .bind(&job.id)
            .bind(&owner)
            .bind(assets)
            .bind(risks)
            .execute(&pool)
            .await;
            match completed {
                Ok(result) if result.rows_affected() == 0 => {
                    let _ = finish_cancelled(&pool, &owner, &job.id).await;
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!(task_id = job.id, ?error, "failed to complete scan task");
                }
            }
        }
        Err(reason) if reason == "任务已取消" => {
            let _ = finish_cancelled(&pool, &owner, &job.id).await;
        }
        Err(reason) => {
            let retrying = job.attempt_count < job.max_attempts;
            let delay = 5_i64.saturating_mul(1_i64 << (job.attempt_count - 1).clamp(0, 6));
            let status = if retrying { "retrying" } else { "failed" };
            if let Err(error) = sqlx::query(
                "UPDATE infra_task SET status=$3,error_message=$4,
                     next_attempt_at=CASE WHEN $3='retrying'
                         THEN now()+make_interval(secs => $5) ELSE next_attempt_at END,
                     end_time=CASE WHEN $3='failed' THEN now()::text ELSE NULL END,
                     lease_owner=NULL,lease_expires_at=NULL,update_time=now()
                 WHERE id=$1 AND lease_owner=$2 AND status='running'",
            )
            .bind(&job.id)
            .bind(&owner)
            .bind(status)
            .bind(reason)
            .bind(delay as i32)
            .execute(&pool)
            .await
            {
                tracing::error!(task_id = job.id, ?error, "failed to retry scan task");
            }
        }
    }
}

async fn execute_job(pool: &PgPool, owner: &str, job: &Job) -> Result<(i32, i32), String> {
    let payload: ScanPayload = serde_json::from_value(job.payload.clone())
        .map_err(|_| "任务参数无效，无法恢复执行".to_owned())?;
    if payload.target_ips.is_empty()
        || payload.target_ips.len() > 64
        || payload.ports.is_empty()
        || payload.ports.len() > 65_535
        || payload
            .ports
            .iter()
            .any(|port| !(1..=65_535).contains(port))
    {
        return Err("任务目标或端口范围无效".to_owned());
    }

    if job.task_kind == "inspection" {
        let tenant =
            rustset_framework_tenant::TenantContext::from_persisted_id(Some(job.tenant_id))
                .map_err(|error| error.message().to_owned())?;
        return crate::inspection::execute_queued(
            pool,
            &tenant,
            &job.id,
            owner,
            payload.target_ips,
            payload.ports,
        )
        .await;
    }
    execute_scan(pool, owner, job, payload).await
}

async fn execute_scan(
    pool: &PgPool,
    owner: &str,
    job: &Job,
    payload: ScanPayload,
) -> Result<(i32, i32), String> {
    let mut found_assets = 0;
    let mut found_risks = 0;
    let mut observed_risk_keys = Vec::new();
    for ip in payload.target_ips {
        let mut ip_open = false;
        for ports in payload.ports.chunks(PROBES_PER_BATCH) {
            ensure_active(pool, owner, &job.id).await?;
            let mut probes = JoinSet::new();
            for &port in ports {
                probes.spawn(async move {
                    let socket = std::net::SocketAddr::new(ip, port as u16);
                    let open = time::timeout(
                        Duration::from_millis(PROBE_TIMEOUT_MS),
                        tokio::net::TcpStream::connect(socket),
                    )
                    .await
                    .is_ok_and(|result| result.is_ok());
                    (port, open)
                });
            }
            while let Some(result) = probes.join_next().await {
                let (port, open) = result.map_err(|error| error.to_string())?;
                if open {
                    ip_open = true;
                    found_risks += 1;
                    observed_risk_keys.push(format!("task_scan:{}:{ip}:{port}", job.id));
                    save_open_port_risk(pool, job, ip, port).await?;
                }
            }
            renew_lease(pool, owner, &job.id).await?;
        }
        if ip_open {
            found_assets += 1;
        }
    }
    sqlx::query(
        "UPDATE infra_risk SET status='resolved',update_time=now()
         WHERE tenant_id=$1 AND source_type='scan_task' AND deleted=0
           AND inspection_key LIKE $2 AND NOT (inspection_key = ANY($3))
           AND status NOT IN ('ignored','false_positive','resolved')",
    )
    .bind(job.tenant_id)
    .bind(format!("task_scan:{}:%", job.id))
    .bind(&observed_risk_keys)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok((found_assets, found_risks))
}

pub(crate) async fn ensure_active(pool: &PgPool, owner: &str, task_id: &str) -> Result<(), String> {
    let state: Option<(bool,)> = sqlx::query_as(
        "SELECT cancel_requested FROM infra_task
         WHERE id=$1 AND lease_owner=$2 AND status='running'",
    )
    .bind(task_id)
    .bind(owner)
    .fetch_optional(pool)
    .await
    .map_err(|error| error.to_string())?;
    match state {
        Some((false,)) => Ok(()),
        Some((true,)) => Err("任务已取消".to_owned()),
        None => Err("任务执行租约已失效".to_owned()),
    }
}

pub(crate) async fn renew_lease(pool: &PgPool, owner: &str, task_id: &str) -> Result<(), String> {
    let affected = sqlx::query(
        "UPDATE infra_task SET heartbeat_at=now(),
             lease_expires_at=now()+make_interval(secs => $3),update_time=now()
         WHERE id=$1 AND lease_owner=$2 AND status='running' AND cancel_requested=false",
    )
    .bind(task_id)
    .bind(owner)
    .bind(LEASE_SECONDS as i32)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?
    .rows_affected();
    if affected == 1 {
        Ok(())
    } else {
        Err("任务已取消".to_owned())
    }
}

async fn save_open_port_risk(
    pool: &PgPool,
    job: &Job,
    ip: IpAddr,
    port: i32,
) -> Result<(), String> {
    let key = format!("task_scan:{}:{ip}:{port}", job.id);
    sqlx::query(
        "INSERT INTO infra_risk
            (id,asset_ip,port,severity,description,status,inspection_key,tenant_id,source_type)
         VALUES($1,$2,$3,'Low',$4,'open',$5,$6,'scan_task')
         ON CONFLICT(tenant_id,inspection_key)
             WHERE deleted=0 AND inspection_key IS NOT NULL
         DO UPDATE SET description=EXCLUDED.description,
             status=CASE WHEN infra_risk.status='resolved' THEN 'open' ELSE infra_risk.status END,
             update_time=now()",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(ip.to_string())
    .bind(port)
    .bind(format!("扫描任务发现 TCP 端口 {port} 开放"))
    .bind(key)
    .bind(job.tenant_id)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;
    Ok(())
}

async fn finish_cancelled(pool: &PgPool, owner: &str, task_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE infra_task SET status='cancelled',error_message='任务已取消',
             end_time=now()::text,lease_owner=NULL,lease_expires_at=NULL,update_time=now()
         WHERE id=$1 AND lease_owner=$2 AND status='running'",
    )
    .bind(task_id)
    .bind(owner)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustset_framework_tenant::TenantContext;

    #[test]
    fn durable_payload_is_strict_and_replayable() {
        let parsed: ScanPayload = serde_json::from_value(serde_json::json!({
            "targetIps": ["127.0.0.1"],
            "ports": [22, 443]
        }))
        .unwrap();
        assert_eq!(
            parsed.target_ips,
            vec!["127.0.0.1".parse::<IpAddr>().unwrap()]
        );
        assert_eq!(parsed.ports, vec![22, 443]);
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the latest migrations"]
    async fn claims_idempotently_probes_and_recovers_an_expired_lease() {
        let pool = PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port() as i32;
        let accept = tokio::spawn(async move {
            let _ = listener.accept().await;
        });
        let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
        let key = format!("worker-test-{}", Uuid::new_v4());
        let first = super::super::enqueue(
            &pool,
            &tenant,
            "test",
            "worker integration",
            "127.0.0.1",
            "custom",
            vec!["127.0.0.1".parse().unwrap()],
            vec![port],
            Some(&key),
            Some(3),
            Some(30),
            &serde_json::json!({}),
        )
        .await
        .unwrap();
        let duplicate = super::super::enqueue(
            &pool,
            &tenant,
            "test",
            "worker integration duplicate",
            "127.0.0.1",
            "custom",
            vec!["127.0.0.1".parse().unwrap()],
            vec![port],
            Some(&key),
            Some(3),
            Some(30),
            &serde_json::json!({}),
        )
        .await
        .unwrap();
        assert_eq!(first, duplicate);

        let owner = "integration-worker";
        let job = claim(&pool, owner).await.unwrap().unwrap();
        assert_eq!(job.id, first);
        assert_eq!(execute_job(&pool, owner, &job).await.unwrap(), (1, 1));
        accept.await.unwrap();
        sqlx::query("UPDATE infra_task SET lease_expires_at=now()-interval '1 second' WHERE id=$1")
            .bind(&first)
            .execute(&pool)
            .await
            .unwrap();
        recover_expired(&pool).await.unwrap();
        let status: String = sqlx::query_scalar("SELECT status FROM infra_task WHERE id=$1")
            .bind(&first)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "retrying");
        sqlx::query("DELETE FROM infra_risk WHERE inspection_key LIKE $1")
            .bind(format!("task_scan:{first}:%"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM infra_task WHERE id=$1")
            .bind(first)
            .execute(&pool)
            .await
            .unwrap();

        let inspection_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let inspection_port = inspection_listener.local_addr().unwrap().port() as i32;
        let inspection_accept = tokio::spawn(async move {
            let _ = inspection_listener.accept().await;
        });
        let inspection_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO infra_task
                (id,name,target,status,port_policy,created_by,tenant_id,task_kind,
                 scan_ports,total_targets,payload,max_attempts,timeout_seconds)
             VALUES($1,'inspection worker test','127.0.0.1','queued','custom','test',
                    1,'inspection',$2,1,$3,3,30)",
        )
        .bind(&inspection_id)
        .bind(vec![inspection_port])
        .bind(serde_json::json!({
            "targetIps":["127.0.0.1"],
            "ports":[inspection_port]
        }))
        .execute(&pool)
        .await
        .unwrap();
        let inspection_job = claim(&pool, owner).await.unwrap().unwrap();
        assert_eq!(inspection_job.id, inspection_id);
        assert_eq!(
            execute_job(&pool, owner, &inspection_job).await.unwrap(),
            (1, 1)
        );
        inspection_accept.await.unwrap();
        let inspection_results = crate::inspection::load_results(&pool, &tenant, &inspection_id)
            .await
            .unwrap();
        assert_eq!(inspection_results.len(), 1);
        assert_eq!(inspection_results[0].open_ports, vec![inspection_port]);
        assert_eq!(inspection_results[0].risks.len(), 1);
        sqlx::query(
            "DELETE FROM infra_risk WHERE id=ANY(
                 SELECT unnest(risk_ids) FROM infra_inspection_result WHERE task_id=$1)",
        )
        .bind(&inspection_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("DELETE FROM infra_inspection_result WHERE task_id=$1")
            .bind(&inspection_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM infra_task WHERE id=$1")
            .bind(inspection_id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
