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
use rustset_framework_tenant::TenantContext;
use rustset_framework_web::AppError;
use rustset_infra_api::{
    InspectionBaselineResponse, InspectionDifference, InspectionResultQuery,
    InspectionResultResponse, IpQuery, QueuedTaskResponse, RunInspectionRequest,
    SaveInspectionBaselineRequest, ScanTaskResponse,
};
use serde_json::json;
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

fn normalize(request: &RunInspectionRequest) -> Result<(Vec<IpAddr>, Vec<i32>), AppError> {
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
    Json(request): Json<RunInspectionRequest>,
) -> Result<Json<ApiResponse<QueuedTaskResponse>>, AppError> {
    start_scan(
        &s.pool,
        TenantContext::from_user(&user)?,
        &user.username,
        request,
    )
    .await
    .map(|v| Json(ApiResponse::new(v)))
}

pub(crate) async fn start_scan(
    pool: &PgPool,
    tenant: TenantContext,
    operator: &str,
    request: RunInspectionRequest,
) -> Result<QueuedTaskResponse, AppError> {
    let (ips, ports) = normalize(&request)?;
    let name = request.name.as_deref().unwrap_or("资产基线核查").trim();
    if name.is_empty() || name.chars().count() > 128 {
        return Err(AppError::bad_request("任务名称需要 1–128 个字符"));
    }
    let mut tx = pool.begin().await.map_err(db_error)?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("inspection:{}", tenant.id()))
        .execute(&mut *tx)
        .await
        .map_err(db_error)?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM infra_task WHERE tenant_id=$1 AND task_kind='inspection' AND status IN ('queued','retrying','running') AND deleted=0").bind(tenant.id()).fetch_one(&mut *tx).await.map_err(db_error)?;
    if count >= 4 {
        return Err(AppError::bad_request("已有 4 个核查任务在执行，请稍后重试"));
    }
    let id = Uuid::new_v4().to_string();
    let summary = if ips.len() == 1 {
        ips[0].to_string()
    } else {
        format!("{} 等 {} 个 IP", ips[0], ips.len())
    };
    let payload = json!({"targetIps":ips,"ports":ports});
    sqlx::query("INSERT INTO infra_task(id,name,target,status,port_policy,created_by,task_kind,scan_ports,total_targets,tenant_id,payload,max_attempts,timeout_seconds) VALUES($1,$2,$3,'queued','custom',$4,'inspection',$5,$6,$7,$8,3,900)")
        .bind(&id).bind(name).bind(summary).bind(operator).bind(&ports).bind(ips.len() as i32).bind(tenant.id()).bind(payload).execute(&mut *tx).await.map_err(db_error)?;
    tx.commit().await.map_err(db_error)?;
    Ok(QueuedTaskResponse {
        task_id: id,
        message: "核查任务已排队".to_owned(),
    })
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

fn differences(
    registered: bool,
    baseline: Option<&[i32]>,
    open: &[i32],
) -> Vec<InspectionDifference> {
    if open.is_empty() {
        return vec![InspectionDifference {
            kind: "no_open_ports".to_owned(),
            port: None,
            severity: "Info".to_owned(),
            description: "本次未发现开放 TCP 端口，不代表资产离线".to_owned(),
        }];
    }
    if !registered {
        return vec![InspectionDifference {
            kind: "unknown_asset".to_owned(),
            port: Some(0),
            severity: "Medium".to_owned(),
            description: "发现可连接的 IP，但资产台账未登记".to_owned(),
        }];
    }
    let Some(allowed) = baseline else {
        return vec![InspectionDifference {
            kind: "baseline_missing".to_owned(),
            port: None,
            severity: "Info".to_owned(),
            description: "资产已登记，尚未确认允许开放的 TCP 端口基线".to_owned(),
        }];
    };
    open.iter()
        .filter(|p| !allowed.contains(p))
        .map(|p| InspectionDifference {
            kind: "unexpected_port".to_owned(),
            port: Some(*p),
            severity: "Medium".to_owned(),
            description: format!("TCP 端口 {p} 开放，超出已确认基线；需核查业务用途"),
        })
        .collect()
}

async fn registered(pool: &PgPool, tenant: &TenantContext, ip: &str) -> Result<bool, sqlx::Error> {
    // Compare normalized IPs in Rust too: IPv6 has multiple equivalent spellings
    // and ledger columns may hold comma-separated lists.
    let candidates: Vec<String> = sqlx::query_scalar(
        "SELECT ip FROM infra_asset WHERE tenant_id=$1 AND deleted=0 AND ip IS NOT NULL
         UNION SELECT ip_address FROM infra_cloud_resource WHERE deleted=0 AND ip_address <> ''
         UNION SELECT management_ip FROM infra_physical_resource WHERE deleted=0 AND COALESCE(management_ip,'') <> ''
         UNION SELECT business_ip FROM infra_physical_resource WHERE deleted=0 AND COALESCE(business_ip,'') <> ''
         UNION SELECT ipmi_address FROM infra_physical_resource WHERE deleted=0 AND COALESCE(ipmi_address,'') <> ''",
    )
        .bind(tenant.id()).fetch_all(pool).await?;
    let needle = ip.parse::<IpAddr>().ok();
    Ok(candidates.iter().any(|v| {
        v.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
            .any(|p| p.parse::<IpAddr>().ok().is_some_and(|v| Some(v) == needle))
    }))
}

pub(crate) async fn execute_queued(
    pool: &PgPool,
    tenant: &TenantContext,
    task: &str,
    lease_owner: &str,
    ips: Vec<IpAddr>,
    ports: Vec<i32>,
) -> Result<(i32, i32), String> {
    for ip in ips {
        crate::task::worker::ensure_active(pool, lease_owner, task).await?;
        let mut observations = Vec::new();
        for chunk in ports.chunks(32) {
            let mut probes = tokio::task::JoinSet::new();
            for &port in chunk {
                probes.spawn(async move { (port, probe(ip, port).await) });
            }
            while let Some(result) = probes.join_next().await {
                observations.push(result.map_err(|error| error.to_string())?);
            }
        }
        save_observation(pool, tenant, task, ip, &observations)
            .await
            .map_err(|error| error.to_string())?;
        crate::task::worker::renew_lease(pool, lease_owner, task).await?;
    }
    sqlx::query_as("SELECT found_assets,found_risks FROM infra_task WHERE id=$1")
        .bind(task)
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())
}

async fn save_observation(
    pool: &PgPool,
    tenant: &TenantContext,
    task: &str,
    ip: IpAddr,
    observations: &[(i32, Observation)],
) -> Result<(), sqlx::Error> {
    let address = ip.to_string();
    let known = registered(pool, tenant, &address).await?;
    let baseline: Option<Vec<i32>> = sqlx::query_scalar(
        "SELECT allowed_ports FROM infra_inspection_baseline WHERE ip=$1::inet AND tenant_id=$2",
    )
    .bind(&address)
    .bind(tenant.id())
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
        .bind(format!("inspection:{}:{address}", tenant.id()))
        .execute(&mut *tx)
        .await?;
    let mut risk_ids = Vec::new();
    for diff in &differences {
        let kind = diff.kind.as_str();
        if !matches!(kind, "unknown_asset" | "unexpected_port") {
            continue;
        }
        let port = diff.port.unwrap_or(0);
        let key = format!("{kind}:{address}:{port}");
        let risk: String=sqlx::query_scalar("INSERT INTO infra_risk(id,asset_ip,port,severity,description,status,inspection_key,solution,tenant_id) VALUES($1,$2,$3,$4,$5,'open',$6,'核实资产归属和业务用途，完成整改后重新核查',$7) ON CONFLICT(tenant_id,inspection_key) WHERE deleted=0 AND inspection_key IS NOT NULL DO UPDATE SET update_time=now(),description=EXCLUDED.description,status=CASE WHEN infra_risk.status='resolved' THEN 'open' ELSE infra_risk.status END RETURNING id")
            .bind(Uuid::new_v4().to_string()).bind(&address).bind(port).bind(&diff.severity).bind(&diff.description).bind(key).bind(tenant.id()).fetch_one(&mut *tx).await?;
        risk_ids.push(risk);
    }
    // Fresh registration resolves unknown-asset alerts. Port closure requires a
    // refused connection or an explicitly approved baseline, never a timeout.
    let old=sqlx::query("SELECT id,port,inspection_key FROM infra_risk WHERE asset_ip=$1 AND tenant_id=$2 AND inspection_key IS NOT NULL AND deleted=0 AND status NOT IN ('ignored','false_positive','resolved')").bind(&address).bind(tenant.id()).fetch_all(&mut *tx).await?;
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
    sqlx::query("INSERT INTO infra_inspection_result(id,task_id,ip,registered,baseline_ports,open_ports,uncertain_ports,differences,risk_ids,tenant_id) VALUES($1,$2,$3::inet,$4,$5,$6,$7,$8,$9,$10)
        ON CONFLICT(task_id,ip) DO UPDATE SET registered=EXCLUDED.registered,baseline_ports=EXCLUDED.baseline_ports,open_ports=EXCLUDED.open_ports,uncertain_ports=EXCLUDED.uncertain_ports,differences=EXCLUDED.differences,risk_ids=EXCLUDED.risk_ids,tenant_id=EXCLUDED.tenant_id,create_time=now()")
        .bind(Uuid::new_v4().to_string()).bind(task).bind(&address).bind(known).bind(&baseline).bind(&open).bind(&uncertain).bind(json!(differences)).bind(&risk_ids).bind(tenant.id()).execute(&mut *tx).await?;
    sqlx::query("UPDATE infra_task SET completed_targets=completed_targets+1,found_assets=found_assets+$2,found_risks=found_risks+$3,update_time=now() WHERE id=$1")
        .bind(task).bind(i32::from(!open.is_empty())).bind(risk_ids.len() as i32).execute(&mut *tx).await?;
    tx.commit().await
}

fn db_error(error: sqlx::Error) -> AppError {
    tracing::error!(?error, "inspection database error");
    AppError::internal("资产核查数据操作失败")
}

async fn list(
    State(s): State<InfraState>,
    user: CurrentUser,
) -> Result<Json<ApiResponse<Vec<ScanTaskResponse>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let rows = crate::task::repository::list_kind(&s.pool, tenant.id(), "inspection")
        .await
        .map_err(db_error)?;
    Ok(Json(ApiResponse::new(rows)))
}

async fn results(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(q): Query<InspectionResultQuery>,
) -> Result<Json<ApiResponse<Vec<InspectionResultResponse>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    Ok(Json(ApiResponse::new(
        load_results(&s.pool, &tenant, &q.task_id).await?,
    )))
}

pub(crate) async fn load_results(
    pool: &PgPool,
    tenant: &TenantContext,
    task_id: &str,
) -> Result<Vec<InspectionResultResponse>, AppError> {
    let rows = sqlx::query(
        "SELECT r.id,r.task_id,host(r.ip) AS ip,r.registered,r.baseline_ports,
            r.open_ports,r.uncertain_ports,r.differences,r.create_time,
            COALESCE((SELECT jsonb_agg(jsonb_build_object(
                'id',k.id,'assetIp',k.asset_ip,'port',k.port,'severity',k.severity,
                'description',k.description,'solution',k.solution,'status',k.status))
              FROM infra_risk k WHERE k.id=ANY(r.risk_ids)
                AND k.tenant_id=r.tenant_id AND k.deleted=0),'[]'::jsonb) AS risks
         FROM infra_inspection_result r
         WHERE r.task_id=$1 AND r.tenant_id=$2 ORDER BY r.ip",
    )
    .bind(task_id)
    .bind(tenant.id())
    .fetch_all(pool)
    .await
    .map_err(db_error)?;
    let rows = rows
        .into_iter()
        .map(|row| {
            let differences = serde_json::from_value(row.get("differences"))
                .map_err(|_| AppError::internal("核查差异数据格式无效"))?;
            let risks = serde_json::from_value(row.get("risks"))
                .map_err(|_| AppError::internal("核查风险数据格式无效"))?;
            Ok(InspectionResultResponse {
                id: row.get("id"),
                task_id: row.get("task_id"),
                ip: row.get("ip"),
                registered: row.get("registered"),
                baseline_ports: row.get("baseline_ports"),
                open_ports: row.get("open_ports"),
                uncertain_ports: row.get("uncertain_ports"),
                differences,
                risks,
                create_time: row
                    .get::<chrono::NaiveDateTime, _>("create_time")
                    .and_utc()
                    .to_rfc3339(),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(rows)
}

async fn baseline(
    State(s): State<InfraState>,
    user: CurrentUser,
    Query(q): Query<IpQuery>,
) -> Result<Json<ApiResponse<Option<InspectionBaselineResponse>>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let ip =
        q.ip.parse::<IpAddr>()
            .map_err(|_| AppError::bad_request("IP 无效"))?
            .to_string();
    let row = sqlx::query(
        "SELECT host(ip) AS ip,allowed_ports,reason,updated_by,update_time
         FROM infra_inspection_baseline WHERE ip=$1::inet AND tenant_id=$2",
    )
    .bind(ip)
    .bind(tenant.id())
    .fetch_optional(&s.pool)
    .await
    .map_err(db_error)?;
    let value = row.map(|row| InspectionBaselineResponse {
        ip: row.get("ip"),
        allowed_ports: row.get("allowed_ports"),
        reason: row.get("reason"),
        updated_by: row.get("updated_by"),
        update_time: row
            .get::<chrono::NaiveDateTime, _>("update_time")
            .and_utc()
            .to_rfc3339(),
    });
    Ok(Json(ApiResponse::new(value)))
}

async fn save_baseline(
    State(s): State<InfraState>,
    user: CurrentUser,
    Json(p): Json<SaveInspectionBaselineRequest>,
) -> Result<Json<ApiResponse<bool>>, AppError> {
    let tenant = TenantContext::from_user(&user)?;
    let ip =
        p.ip.parse::<IpAddr>()
            .map_err(|_| AppError::bad_request("IP 无效"))?
            .to_string();
    let ports = normalize_ports(&p.allowed_ports, true)?;
    if p.reason.trim().is_empty() || p.reason.chars().count() > 1000 {
        return Err(AppError::bad_request("请填写基线确认依据，最多 1000 字"));
    }
    if !registered(&s.pool, &tenant, &ip).await.map_err(db_error)? {
        return Err(AppError::bad_request("请先在资产台账登记该 IP"));
    }
    sqlx::query("INSERT INTO infra_inspection_baseline(ip,allowed_ports,reason,updated_by,tenant_id) VALUES($1::inet,$2,$3,$4,$5) ON CONFLICT(tenant_id,ip) DO UPDATE SET allowed_ports=EXCLUDED.allowed_ports,reason=EXCLUDED.reason,updated_by=EXCLUDED.updated_by,update_time=now()")
        .bind(ip).bind(ports).bind(p.reason.trim()).bind(user.username).bind(tenant.id()).execute(&s.pool).await.map_err(db_error)?;
    Ok(Json(ApiResponse::new(true)))
}
