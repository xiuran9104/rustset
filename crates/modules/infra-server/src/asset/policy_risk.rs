use super::*;
use schemars::JsonSchema;
use serde::Serialize;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PortRange {
    protocol: String,
    start: i32,
    end: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct RiskRule {
    id: i64,
    name: String,
    protocol: String,
    port_start: i32,
    port_end: i32,
    severity: String,
    description: String,
    solution: String,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct RiskMatchSummary {
    pub policies_checked: usize,
    pub warnings_matched: usize,
}

pub(super) fn validate_service_ports(value: &str) -> Result<(), AppError> {
    parse_service_ports(value).map(|_| ())
}

fn parse_service_ports(value: &str) -> Result<Vec<PortRange>, AppError> {
    let normalized = value.trim().to_ascii_lowercase().replace(['，', '；'], ",");
    if normalized.is_empty() {
        return Err(AppError::bad_request("service_port 不能为空"));
    }

    let mut ranges = Vec::new();
    for raw in normalized.split(|character: char| {
        character == ',' || character == ';' || character.is_ascii_whitespace()
    }) {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        if ranges.len() >= 128 {
            return Err(AppError::bad_request("service_port 最多包含 128 个端口项"));
        }

        let (protocol, expression) = if let Some((protocol, expression)) = token.split_once('/') {
            (protocol, expression)
        } else if let Some((protocol, expression)) = token.split_once(':') {
            (protocol, expression)
        } else {
            ("tcp", token)
        };
        if !matches!(protocol, "tcp" | "udp" | "any") {
            return Err(AppError::bad_request(format!(
                "service_port 协议不支持: {protocol}"
            )));
        }
        if matches!(expression, "*" | "any" | "all") {
            ranges.push(PortRange {
                protocol: protocol.to_string(),
                start: 1,
                end: 65_535,
            });
            continue;
        }

        let (start, end) = if let Some((start, end)) = expression.split_once('-') {
            (parse_port(start)?, parse_port(end)?)
        } else {
            let port = parse_port(expression)?;
            (port, port)
        };
        if start > end {
            return Err(AppError::bad_request(format!(
                "service_port 范围起始值不能大于结束值: {token}"
            )));
        }
        ranges.push(PortRange {
            protocol: protocol.to_string(),
            start,
            end,
        });
    }
    if ranges.is_empty() {
        return Err(AppError::bad_request("service_port 未包含有效端口"));
    }
    Ok(ranges)
}

fn parse_port(value: &str) -> Result<i32, AppError> {
    value
        .parse::<i32>()
        .ok()
        .filter(|port| (1..=65_535).contains(port))
        .ok_or_else(|| AppError::bad_request(format!("端口必须在 1-65535 之间: {value}")))
}

fn matches(rule: &RiskRule, range: &PortRange) -> bool {
    (rule.protocol == "any" || range.protocol == "any" || rule.protocol == range.protocol)
        && range.start <= rule.port_end
        && range.end >= rule.port_start
}

pub(super) async fn create(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    payload: Value,
) -> Result<Json<ApiResponse<String>>, AppError> {
    let mut payload = crate::camel_payload_to_snake(payload);
    validate_payload(&payload, true)?;
    payload
        .as_object_mut()
        .ok_or_else(|| AppError::bad_request("record must be an object"))?
        .insert("tenant_id".into(), json!(tenant.id()));
    let missing = crate::missing_required_fields(pool, NETWORK_POLICY.table, &payload).await?;
    if !missing.is_empty() {
        return Err(AppError::bad_request(format!(
            "missing required fields: {}",
            missing.join(", ")
        )));
    }
    let mut columns =
        crate::table_writable_columns(pool, NETWORK_POLICY.table, &payload, false).await?;
    columns.push("tenant_id".into());
    let values = columns
        .iter()
        .map(|column| format!("r.{column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {table} ({}) SELECT {values} FROM jsonb_populate_record(NULL::{table}, $1::jsonb) r RETURNING id",
        columns.join(", "),
        table = NETWORK_POLICY.table
    );

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start network-policy transaction"))?;
    let id: i64 = sqlx::query_scalar(&sql)
        .bind(payload)
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| crate::record_query_error("create", error))?;
    sync_policy(&mut tx, tenant.id(), id).await?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit network-policy transaction"))?;
    Ok(Json(ApiResponse::new(id.to_string())))
}

pub(super) async fn update(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    payload: Value,
) -> Result<Json<ApiResponse<()>>, AppError> {
    let id = payload
        .get("id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| AppError::bad_request("id is required"))?;
    let mut payload = crate::camel_payload_to_snake(payload);
    if let Some(object) = payload.as_object_mut() {
        object.remove("tenant_id");
    }
    validate_payload(&payload, false)?;
    let columns = crate::table_writable_columns(pool, NETWORK_POLICY.table, &payload, true).await?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start network-policy transaction"))?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM infra_network_policy WHERE id=$1 AND tenant_id=$2 AND deleted=0 FOR UPDATE)",
    )
    .bind(id)
    .bind(tenant.id())
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock network policy"))?;
    if !exists {
        return Err(AppError::not_found("record not found"));
    }
    if !columns.is_empty() {
        let set = columns
            .iter()
            .map(|column| format!("{column}=r.{column}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "UPDATE {table} t SET {set}, update_time=now() FROM jsonb_populate_record(NULL::{table}, $2::jsonb) r WHERE t.id=$1 AND t.tenant_id=$3 AND t.deleted=0",
            table = NETWORK_POLICY.table
        );
        sqlx::query(&sql)
            .bind(id)
            .bind(payload)
            .bind(tenant.id())
            .execute(&mut *tx)
            .await
            .map_err(|error| crate::record_query_error("update", error))?;
    }
    sync_policy(&mut tx, tenant.id(), id).await?;
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit network-policy transaction"))?;
    Ok(Json(ApiResponse::new(())))
}

fn validate_payload(payload: &Value, require_fields: bool) -> Result<(), AppError> {
    if let Some(action) = payload.get("action").and_then(Value::as_str) {
        if !matches!(action, "allow" | "deny") {
            return Err(AppError::bad_request("action 必须为 allow 或 deny"));
        }
    } else if require_fields {
        return Err(AppError::bad_request("action 不能为空"));
    }
    if let Some(service_port) = payload.get("service_port").and_then(Value::as_str) {
        validate_service_ports(service_port)?;
    } else if require_fields {
        return Err(AppError::bad_request("service_port 不能为空"));
    }
    Ok(())
}

pub(super) async fn delete(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    ids: &[i64],
) -> Result<Json<ApiResponse<()>>, AppError> {
    if ids.is_empty() || ids.iter().any(|id| *id <= 0) {
        return Err(AppError::bad_request("ids are required"));
    }
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start network-policy transaction"))?;
    let deleted: Vec<i64> = sqlx::query_scalar(
        "UPDATE infra_network_policy SET deleted=1, update_time=now()
         WHERE id=ANY($1) AND tenant_id=$2 AND deleted=0 RETURNING id",
    )
    .bind(ids)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to delete network policies"))?;
    if !deleted.is_empty() {
        close_policy_risks(&mut tx, tenant.id(), &deleted).await?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit network-policy transaction"))?;
    Ok(Json(ApiResponse::new(())))
}

pub(super) async fn recheck(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
) -> Result<Json<ApiResponse<RiskMatchSummary>>, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start risk matching transaction"))?;
    let policy_ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM infra_network_policy WHERE tenant_id=$1 AND deleted=0 ORDER BY id FOR UPDATE",
    )
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to read network policies"))?;
    let mut warnings_matched = 0;
    for policy_id in &policy_ids {
        warnings_matched += sync_policy(&mut tx, tenant.id(), *policy_id).await?;
    }
    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit risk matching transaction"))?;
    Ok(Json(ApiResponse::new(RiskMatchSummary {
        policies_checked: policy_ids.len(),
        warnings_matched,
    })))
}

async fn sync_policy(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: i64,
    policy_id: i64,
) -> Result<usize, AppError> {
    let policy: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT action,service_port,firewall_name,source_ip,destination_ip
         FROM infra_network_policy WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(policy_id)
    .bind(tenant_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| AppError::internal("failed to read network policy"))?;
    let Some((action, service_port, firewall_name, source_ip, destination_ip)) = policy else {
        close_policy_risks(tx, tenant_id, &[policy_id]).await?;
        return Ok(0);
    };
    let ranges = parse_service_ports(&service_port)?;
    if action != "allow" {
        close_policy_risks(tx, tenant_id, &[policy_id]).await?;
        return Ok(0);
    }

    let rules = sqlx::query_as::<_, RiskRule>(
        "SELECT id,name,protocol,port_start,port_end,severity,description,solution
         FROM infra_high_risk_port_rule WHERE enabled AND deleted=0 ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(|_| AppError::internal("failed to read high-risk port rules"))?;
    let mut active_keys = Vec::new();
    for rule in &rules {
        let Some(range) = ranges.iter().find(|range| matches(rule, range)) else {
            continue;
        };
        let matched_port = range.start.max(rule.port_start);
        let key = format!("network_policy:{policy_id}:{}", rule.id);
        let description = format!(
            "网络策略“{firewall_name}”允许 {source_ip} 访问 {destination_ip} 的高风险端口 {matched_port}（{}）。{}",
            rule.name, rule.description
        );
        sqlx::query(
            "INSERT INTO infra_risk
                (id,asset_ip,port,severity,description,solution,status,inspection_key,
                 tenant_id,source_type,source_id,rule_id)
             VALUES($1,$2,$3,$4,$5,$6,'open',$7,$8,'network_policy',$9,$10)
             ON CONFLICT(tenant_id,inspection_key)
                 WHERE deleted=0 AND inspection_key IS NOT NULL
             DO UPDATE SET asset_ip=EXCLUDED.asset_ip,port=EXCLUDED.port,
                 severity=EXCLUDED.severity,description=EXCLUDED.description,
                 solution=EXCLUDED.solution,source_type=EXCLUDED.source_type,
                 source_id=EXCLUDED.source_id,rule_id=EXCLUDED.rule_id,
                 status=CASE WHEN infra_risk.status='resolved' THEN 'open' ELSE infra_risk.status END,
                 update_time=now()",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&destination_ip)
        .bind(matched_port)
        .bind(&rule.severity)
        .bind(description)
        .bind(&rule.solution)
        .bind(&key)
        .bind(tenant_id)
        .bind(policy_id)
        .bind(rule.id)
        .execute(&mut **tx)
        .await
        .map_err(|_| AppError::internal("failed to save network-policy risk"))?;
        active_keys.push(key);
    }

    if active_keys.is_empty() {
        close_policy_risks(tx, tenant_id, &[policy_id]).await?;
    } else {
        sqlx::query(
            "UPDATE infra_risk SET status='resolved',update_time=now()
             WHERE tenant_id=$1 AND source_type='network_policy' AND source_id=$2
               AND deleted=0 AND NOT(inspection_key=ANY($3))
               AND status NOT IN ('resolved','ignored','false_positive')",
        )
        .bind(tenant_id)
        .bind(policy_id)
        .bind(&active_keys)
        .execute(&mut **tx)
        .await
        .map_err(|_| AppError::internal("failed to close stale network-policy risks"))?;
    }
    Ok(active_keys.len())
}

async fn close_policy_risks(
    tx: &mut Transaction<'_, Postgres>,
    tenant_id: i64,
    policy_ids: &[i64],
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE infra_risk SET status='resolved',update_time=now()
         WHERE tenant_id=$1 AND source_type='network_policy' AND source_id=ANY($2)
           AND deleted=0 AND status NOT IN ('resolved','ignored','false_positive')",
    )
    .bind(tenant_id)
    .bind(policy_ids)
    .execute(&mut **tx)
    .await
    .map_err(|_| AppError::internal("failed to close network-policy risks"))?;
    Ok(())
}

pub(super) async fn list(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
) -> Result<Json<ApiResponse<Vec<Value>>>, AppError> {
    let rows = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(p) || jsonb_build_object('high_risk_count',(
             SELECT count(*) FROM infra_risk r
             WHERE r.tenant_id=p.tenant_id AND r.source_type='network_policy'
               AND r.source_id=p.id AND r.deleted=0
               AND r.status NOT IN ('resolved','ignored','false_positive')))
         FROM infra_network_policy p WHERE p.tenant_id=$1 AND p.deleted=0 ORDER BY p.id DESC",
    )
    .bind(tenant.id())
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list network policies"))?;
    Ok(Json(ApiResponse::new(
        rows.into_iter().map(crate::table_value).collect(),
    )))
}

pub(super) async fn page(
    pool: &sqlx::PgPool,
    tenant: &TenantContext,
    params: QueryParams,
) -> Result<Json<ApiResponse<crate::Page<Value>>>, AppError> {
    let size = params.page_size.unwrap_or(10).clamp(1, 200);
    let offset = (params.page_no.unwrap_or(1).max(1) - 1).saturating_mul(size);
    let total = sqlx::query_scalar(
        "SELECT count(*) FROM infra_network_policy WHERE tenant_id=$1 AND deleted=0",
    )
    .bind(tenant.id())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count network policies"))?;
    let list = sqlx::query_scalar::<_, Value>(
        "SELECT to_jsonb(p) || jsonb_build_object('high_risk_count',(
             SELECT count(*) FROM infra_risk r
             WHERE r.tenant_id=p.tenant_id AND r.source_type='network_policy'
               AND r.source_id=p.id AND r.deleted=0
               AND r.status NOT IN ('resolved','ignored','false_positive')))
         FROM infra_network_policy p WHERE p.tenant_id=$1 AND p.deleted=0
         ORDER BY p.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(tenant.id())
    .bind(size)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list network policies"))?
    .into_iter()
    .map(crate::table_value)
    .collect();
    Ok(Json(ApiResponse::new(crate::Page { list, total })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_port_notation() {
        assert_eq!(
            parse_service_ports("22, 80，443; tcp/3389 udp:137-139").unwrap(),
            vec![
                PortRange {
                    protocol: "tcp".into(),
                    start: 22,
                    end: 22
                },
                PortRange {
                    protocol: "tcp".into(),
                    start: 80,
                    end: 80
                },
                PortRange {
                    protocol: "tcp".into(),
                    start: 443,
                    end: 443
                },
                PortRange {
                    protocol: "tcp".into(),
                    start: 3389,
                    end: 3389
                },
                PortRange {
                    protocol: "udp".into(),
                    start: 137,
                    end: 139
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_ports_and_protocols() {
        assert!(parse_service_ports("0").is_err());
        assert!(parse_service_ports("65536").is_err());
        assert!(parse_service_ports("9000-8000").is_err());
        assert!(parse_service_ports("icmp/8").is_err());
    }

    #[test]
    fn range_overlap_honors_protocol() {
        let rule = RiskRule {
            id: 1,
            name: "RDP".into(),
            protocol: "tcp".into(),
            port_start: 3389,
            port_end: 3389,
            severity: "Critical".into(),
            description: String::new(),
            solution: String::new(),
        };
        assert!(matches(
            &rule,
            &PortRange {
                protocol: "tcp".into(),
                start: 3300,
                end: 3400
            }
        ));
        assert!(!matches(
            &rule,
            &PortRange {
                protocol: "udp".into(),
                start: 3300,
                end: 3400
            }
        ));
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL with the latest migrations"]
    async fn policy_changes_create_and_close_tenant_scoped_risks() {
        let pool = sqlx::PgPool::connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let tenant = TenantContext::from_persisted_id(Some(1)).unwrap();
        let other_tenant = TenantContext::from_persisted_id(Some(121)).unwrap();
        let created = create(
            &pool,
            &tenant,
            json!({
                "firewall_name": "integration-risk-firewall",
                "destination_organization": "测试单位",
                "destination_project": "测试项目",
                "source_organization": "来源单位",
                "source_project": "来源项目",
                "source_security_zone": "互联网域",
                "source_ip": "0.0.0.0/0",
                "destination_security_zone": "服务器域",
                "destination_ip": "10.1.2.3",
                "service_port": "443,tcp/3389",
                "applicant": "tester",
                "application_date": "2026-09-27",
                "traffic_direction": "正向",
                "action": "allow"
            }),
        )
        .await
        .unwrap()
        .0;
        let policy_id = created.data.parse::<i64>().unwrap();

        let warning: (String, String, i32) = sqlx::query_as(
            "SELECT severity,status,port FROM infra_risk
             WHERE tenant_id=1 AND source_type='network_policy' AND source_id=$1",
        )
        .bind(policy_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(warning, ("Critical".into(), "open".into(), 3389));
        let leaked: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM infra_risk
             WHERE tenant_id=$1 AND source_type='network_policy' AND source_id=$2",
        )
        .bind(other_tenant.id())
        .bind(policy_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(leaked, 0);

        let _ = update(&pool, &tenant, json!({"id": policy_id, "action": "deny"}))
            .await
            .unwrap();
        let status: String = sqlx::query_scalar(
            "SELECT status FROM infra_risk
             WHERE tenant_id=1 AND source_type='network_policy' AND source_id=$1",
        )
        .bind(policy_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(status, "resolved");
        let _ = delete(&pool, &tenant, &[policy_id]).await.unwrap();
    }
}
