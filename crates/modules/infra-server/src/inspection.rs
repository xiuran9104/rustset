//! TCP observations compared with inventory and explicitly approved baselines.
//! A timeout is inconclusive, and an open port alone is not a vulnerability.
use crate::InfraState;
use aide::axum::ApiRouter;
use aide::axum::routing::{get, post};
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
use sqlx::{PgPool, Row};
use std::{
    collections::BTreeSet,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use uuid::Uuid;

pub fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/inspection/list", get(list))
        .api_route("/infra/inspection/results", get(results))
        .api_route("/infra/inspection/run", post(run))
        .api_route(
            "/infra/inspection/baseline",
            get(baseline).put(save_baseline),
        )
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScanRequest {
    pub target_ips: Vec<String>,
    pub ports: Vec<i32>,
    pub name: Option<String>,
}

fn normalize(request: &ScanRequest) -> Result<(Vec<IpAddr>, Vec<i32>), AppError> {
    if request.target_ips.is_empty() || request.target_ips.len() > 64 {
        return Err(AppError::bad_request("每次核查需要 1–64 个明确的 IP 地址"));
    }
    let ips = request
        .target_ips
        .iter()
        .map(|v| {
            v.trim().parse::<IpAddr>().map_err(|_| {
                AppError::bad_request("目标必须是有效 IP 地址，不支持域名或网段表达式")
            })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let ports = normalize_ports(&request.ports, false)?;
    Ok((ips.into_iter().collect(), ports))
}

fn normalize_ports(ports: &[i32], allow_empty: bool) -> Result<Vec<i32>, AppError> {
    if (!allow_empty && ports.is_empty())
        || ports.len() > 128
        || ports.iter().any(|p| !(1..=65535).contains(p))
    {
        return Err(AppError::bad_request(
            "端口必须在 1–65535 之间，每次最多 128 个",
        ));
    }
    Ok(ports
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

async fn run(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(request): Json<ScanRequest>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    start_scan(&s.pool, &user.username, request)
        .await
        .map(|v| Json(ApiResponse::new(v)))
}

pub(crate) async fn start_scan(
    pool: &PgPool,
    operator: &str,
    request: ScanRequest,
) -> Result<Value, AppError> {
    let (ips, ports) = normalize(&request)?;
    let name = request.name.as_deref().unwrap_or("资产基线核查").trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("任务名称需要 1–128 个字符"));
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(220022)")
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    sqlx::query("UPDATE infra_task SET status='failed',error_message='执行中断或超时，请重新核查',end_time=now()::text,update_time=now() WHERE task_kind='inspection' AND status='running' AND update_time < now()-interval '15 minutes'").execute(&mut *tx).await.map_err(db_error)?;
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM infra_task WHERE task_kind='inspection' AND status='running'",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(db_error)?;
    if count >= 4 {
        return Err(AppError::bad_request("已有 4 个核查任务在执行，请稍后重试"));
    }
    let id = Uuid::new_v4().to_string();
    let summary = if ips.len() == 1 {
        ips[0].to_string()
    } else {
        format!("{} 等 {} 个 IP", ips[0], ips.len())
    };
    sqlx::query("INSERT INTO infra_task(id,name,target,status,port_policy,created_by,task_kind,scan_ports,total_targets,start_time) VALUES($1,$2,$3,'running','custom',$4,'inspection',$5,$6,now()::text)")
        .bind(&id).bind(name).bind(summary).bind(operator).bind(&ports).bind(ips.len() as i32).execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    let worker_pool = pool.clone();
    let task_id = id.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(&worker_pool, &task_id, ips, ports).await {
            tracing::error!(task_id, ?error, "inspection failed");
            let _=sqlx::query("UPDATE infra_task SET status='failed',error_message='核查执行或结果保存失败，请重新核查',end_time=now()::text,update_time=now() WHERE id=$1")
                .bind(&task_id).execute(&worker_pool).await;
        }
    });
    Ok(json!({"taskId":id,"message":"核查任务已启动"}))
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Observation {
    Open,
    Closed,
    Uncertain,
}

async fn probe(ip: IpAddr, port: i32) -> Observation {
    match tokio::time::timeout(
        Duration::from_millis(800),
        tokio::net::TcpStream::connect(SocketAddr::new(ip, port as u16)),
    )
    .await
    {
        Ok(Ok(_)) => Observation::Open,
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::ConnectionRefused => Observation::Closed,
        _ => Observation::Uncertain,
    }
}

fn differences(registered: bool, baseline: Option<&[i32]>, open: &[i32]) -> Vec<Value> {
    if open.is_empty() {
        return vec![
            json!({"kind":"no_open_ports","severity":"Info","description":"本次未发现开放 TCP 端口，不代表资产离线"}),
        ];
    }
    if !registered {
        return vec![
            json!({"kind":"unknown_asset","port":0,"severity":"Medium","description":"发现可连接的 IP，但资产台账未登记"}),
        ];
    }
    let Some(allowed) = baseline else {
        return vec![
            json!({"kind":"baseline_missing","severity":"Info","description":"资产已登记，尚未确认允许开放的 TCP 端口基线"}),
        ];
    };
    open.iter().filter(|p|!allowed.contains(p)).map(|p|json!({"kind":"unexpected_port","port":p,"severity":"Medium","description":format!("TCP 端口 {p} 开放，超出已确认基线；需核查业务用途") })).collect()
}

async fn registered(pool: &PgPool, ip: &str) -> Result<bool, sqlx::Error> {
    // Compare normalized IPs in Rust too: IPv6 has multiple equivalent spellings
    // and ledger columns may hold comma-separated lists.
    let candidates: Vec<String> = sqlx::query_scalar(
        "SELECT ip FROM infra_asset WHERE deleted=0 AND ip IS NOT NULL
         UNION SELECT ip_address FROM infra_cloud_resource WHERE deleted=0 AND ip_address <> ''
         UNION SELECT management_ip FROM infra_physical_resource WHERE deleted=0 AND COALESCE(management_ip,'') <> ''
         UNION SELECT business_ip FROM infra_physical_resource WHERE deleted=0 AND COALESCE(business_ip,'') <> ''
         UNION SELECT ipmi_address FROM infra_physical_resource WHERE deleted=0 AND COALESCE(ipmi_address,'') <> ''",
    )
        .fetch_all(pool).await?;
    let needle = ip.parse::<IpAddr>().ok();
    Ok(candidates.iter().any(|v| {
        v.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .any(|p| p.parse::<IpAddr>().ok().is_some_and(|v| Some(v) == needle))
    }))
}

async fn execute(
    pool: &PgPool,
    task: &str,
    ips: Vec<IpAddr>,
    ports: Vec<i32>,
) -> Result<(), sqlx::Error> {
    for ip in ips {
        let mut observations = Vec::new();
        for chunk in ports.chunks(32) {
            let mut probes = tokio::task::JoinSet::new();
            for &port in chunk {
                probes.spawn(async move { (port, probe(ip, port).await) });
            }
            while let Some(result) = probes.join_next().await {
                observations.push(result.map_err(|e| sqlx::Error::Protocol(e.to_string()))?);
            }
        }
        save_observation(pool, task, ip, &observations).await?;
    }
    sqlx::query("UPDATE infra_task SET status='completed',end_time=now()::text,update_time=now() WHERE id=$1")
        .bind(task).execute(pool).await?;
    Ok(())
}

async fn save_observation(
    pool: &PgPool,
    task: &str,
    ip: IpAddr,
    observations: &[(i32, Observation)],
) -> Result<(), sqlx::Error> {
    let address = ip.to_string();
    let known = registered(pool, &address).await?;
    let baseline: Option<Vec<i32>> =
        sqlx::query_scalar("SELECT allowed_ports FROM infra_inspection_baseline WHERE ip=$1::inet")
            .bind(&address)
            .fetch_optional(pool)
            .await?;
    let select = |state| {
        observations
            .iter()
            .filter(|(_, s)| *s == state)
            .map(|(p, _)| *p)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    };
    let open = select(Observation::Open);
    let uncertain = select(Observation::Uncertain);
    let differences = differences(known, baseline.as_deref(), &open);
    let mut tx = pool.begin().await?;
    // Serialize risk refresh/closure for overlapping scans of the same address.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("inspection:{address}"))
        .execute(&mut *tx)
        .await?;
    let mut risk_ids = Vec::new();
    for diff in &differences {
        let kind = diff["kind"].as_str().unwrap_or_default();
        if !matches!(kind, "unknown_asset" | "unexpected_port") {
            continue;
        }
        let port = diff["port"].as_i64().unwrap_or(0) as i32;
        let key = format!("{kind}:{address}:{port}");
        let risk: String=sqlx::query_scalar("INSERT INTO infra_risk(id,asset_ip,port,severity,description,status,inspection_key,solution) VALUES($1,$2,$3,$4,$5,'open',$6,'核实资产归属和业务用途，完成整改后重新核查') ON CONFLICT(inspection_key) WHERE deleted=0 AND inspection_key IS NOT NULL DO UPDATE SET update_time=now(),description=EXCLUDED.description,status=CASE WHEN infra_risk.status='resolved' THEN 'open' ELSE infra_risk.status END RETURNING id")
            .bind(Uuid::new_v4().to_string()).bind(&address).bind(port).bind(diff["severity"].as_str().unwrap_or("Medium")).bind(diff["description"].as_str().unwrap_or_default()).bind(key).fetch_one(&mut *tx).await?;
        risk_ids.push(risk);
    }
    // Fresh registration resolves unknown-asset alerts. Port closure requires a
    // refused connection or an explicitly approved baseline, never a timeout.
    let old=sqlx::query("SELECT id,port,inspection_key FROM infra_risk WHERE asset_ip=$1 AND inspection_key IS NOT NULL AND deleted=0 AND status NOT IN ('ignored','false_positive','resolved')").bind(&address).fetch_all(&mut *tx).await?;
    for row in old {
        let key: String = row.get("inspection_key");
        let port: i32 = row.get("port");
        let resolved = if key.starts_with("unknown_asset:") {
            known
        } else {
            observations.iter().any(|(p, state)| {
                *p == port
                    && (*state == Observation::Closed
                        || (known && baseline.as_ref().is_some_and(|b| b.contains(p))))
            })
        };
        if resolved {
            sqlx::query("UPDATE infra_risk SET status='resolved',update_time=now() WHERE id=$1")
                .bind(row.get::<String, _>("id"))
                .execute(&mut *tx)
                .await?;
        }
    }
    sqlx::query("INSERT INTO infra_inspection_result(id,task_id,ip,registered,baseline_ports,open_ports,uncertain_ports,differences,risk_ids) VALUES($1,$2,$3::inet,$4,$5,$6,$7,$8,$9)")
        .bind(Uuid::new_v4().to_string()).bind(task).bind(&address).bind(known).bind(&baseline).bind(&open).bind(&uncertain).bind(json!(differences)).bind(&risk_ids).execute(&mut *tx).await?;
    sqlx::query("UPDATE infra_task SET completed_targets=completed_targets+1,found_assets=found_assets+$2,found_risks=found_risks+$3,update_time=now() WHERE id=$1")
        .bind(task).bind(i32::from(!open.is_empty())).bind(risk_ids.len() as i32).execute(&mut *tx).await?;
    tx.commit().await
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(?error, "inspection database error");
    AppError::internal("资产核查数据操作失败")
}

async fn list(State(s): State<InfraState>) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    sqlx::query("UPDATE infra_task SET status='failed',error_message='执行中断或超时，请重新核查',end_time=now()::text,update_time=now() WHERE task_kind='inspection' AND status='running' AND update_time < now()-interval '15 minutes'").execute(&s.pool).await.map_err(db_error)?;
    let rows=sqlx::query_scalar::<_,Value>("SELECT to_jsonb(t) FROM infra_task t WHERE task_kind='inspection' AND deleted=0 ORDER BY create_time DESC LIMIT 200").fetch_all(&s.pool).await.map_err(db_error)?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(crate::table_value).collect(),
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ResultQuery {
    task_id: String,
}
async fn results(
    State(s): State<InfraState>,
    Query(q): Query<ResultQuery>,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let rows=sqlx::query_scalar::<_,Value>("SELECT to_jsonb(r) || jsonb_build_object('ip',host(r.ip),'risks',COALESCE((SELECT jsonb_agg(to_jsonb(k)) FROM infra_risk k WHERE k.id=ANY(r.risk_ids) AND k.deleted=0),'[]'::jsonb)) FROM infra_inspection_result r WHERE task_id=$1 ORDER BY r.ip").bind(q.task_id).fetch_all(&s.pool).await.map_err(db_error)?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(crate::table_value).collect(),
    )))
}

#[derive(Deserialize, JsonSchema)]
struct IpQuery {
    ip: String,
}
async fn baseline(
    State(s): State<InfraState>,
    Query(q): Query<IpQuery>,
) -> Result<Json<ApiResponse<Value>>, AppError> {
    let ip =
        q.ip.parse::<IpAddr>()
            .map_err(|_| AppError::bad_request("IP 无效"))?
            .to_string();
    let value=sqlx::query_scalar::<_,Value>("SELECT to_jsonb(b) || jsonb_build_object('ip',host(ip)) FROM infra_inspection_baseline b WHERE ip=$1::inet").bind(ip).fetch_optional(&s.pool).await.map_err(db_error)?;
    Ok(Json(ApiResponse::new(
        value.map(crate::table_value).unwrap_or(Value::Null),
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct BaselineRequest {
    ip: String,
    allowed_ports: Vec<i32>,
    reason: String,
}
async fn save_baseline(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<BaselineRequest>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let ip =
        p.ip.parse::<IpAddr>()
            .map_err(|_| AppError::bad_request("IP 无效"))?
            .to_string();
    let ports = normalize_ports(&p.allowed_ports, true)?;
    if p.reason.trim().is_empty() || p.reason.chars().count() > 1000 {
        return Err(AppError::bad_request("请填写基线确认依据，最多 1000 字"));
    }
    if !registered(&s.pool, &ip).await.map_err(db_error)? {
        return Err(AppError::bad_request("请先在资产台账登记该 IP"));
    }
    sqlx::query("INSERT INTO infra_inspection_baseline(ip,allowed_ports,reason,updated_by) VALUES($1::inet,$2,$3,$4) ON CONFLICT(ip) DO UPDATE SET allowed_ports=EXCLUDED.allowed_ports,reason=EXCLUDED.reason,updated_by=EXCLUDED.updated_by,update_time=now()")
        .bind(ip).bind(ports).bind(p.reason.trim()).bind(user.username).execute(&s.pool).await.map_err(db_error)?;
    Ok(Json(ApiResponse::new(true)))
}
