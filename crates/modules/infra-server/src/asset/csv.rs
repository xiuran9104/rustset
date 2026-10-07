use super::*;
use axum::{
    extract::{DefaultBodyLimit, Multipart},
    response::Response,
};
use rustset_framework_common::csv::{self as exchange, ImportResult};

const ASSET_COLUMNS: &[&str] = &[
    "name",
    "ip",
    "zone",
    "ports",
    "last_scanned",
    "contact_person",
    "contact_phone",
    "owner",
    "weight",
    "labels",
    "os",
    "device_type",
    "city",
    "district",
    "organization_name",
    "business_department",
    "department_contact",
    "application_name",
    "server_name",
    "hardware_configuration",
    "operating_system",
    "database_type",
    "launch_date",
    "decommission_date",
    "application_type",
    "network_environment",
    "internet_ipv4",
    "internet_ipv6",
    "domain_address",
    "internal_network_ip",
    "government_extranet_ip",
    "open_ports",
    "publishing_endpoint",
    "publishes_other_endpoint",
    "other_endpoint_name",
    "security_product_installation",
    "development_vendor",
    "development_vendor_contact",
    "security_vendor",
    "security_vendor_contact",
    "operations_vendor",
    "operations_vendor_contact",
    "classified_protection_level",
    "classified_protection_assessed",
    "classified_protection_assessor",
    "classified_protection_assessment_date",
    "classified_protection_score",
    "classified_protection_filed",
    "classified_protection_filing_date",
    "classified_protection_filing_number",
    "classified_protection_filing_authority",
    "cryptography_assessed",
    "cryptography_assessment_level",
    "cryptography_assessment_date",
    "cryptography_assessment_number",
];
const POLICY_COLUMNS: &[&str] = &[
    "firewall_name",
    "destination_organization",
    "destination_project",
    "source_organization",
    "source_project",
    "source_security_zone",
    "source_ip",
    "destination_security_zone",
    "destination_ip",
    "service_port",
    "applicant",
    "application_date",
    "traffic_direction",
    "action",
    "implementer",
    "implementation_date",
    "delivery_date",
];

pub(super) fn routes() -> ApiRouter<InfraState> {
    ApiRouter::new()
        .api_route("/infra/asset/export-csv", get(asset_export))
        .api_route("/infra/asset/import-template", get(asset_template))
        .api_route("/infra/asset/import-csv", post(asset_import))
        .api_route("/infra/network-policy/export-csv", get(policy_export))
        .api_route(
            "/infra/network-policy/import-template",
            get(policy_template),
        )
        .api_route("/infra/network-policy/import-csv", post(policy_import))
        .layer(DefaultBodyLimit::max(exchange::MAX_BYTES + 64 * 1024))
}

macro_rules! handlers {
    ($export:ident, $template:ident, $import:ident, $spec:ident, $columns:ident, $filename:literal) => {
        async fn $export(
            State(s): State<InfraState>,
            user: CurrentUser,
        ) -> Result<Response, AppError> {
            export(
                &s,
                &TenantContext::from_user(&user)?,
                $spec,
                $columns,
                $filename,
            )
            .await
        }
        async fn $template(user: CurrentUser) -> Result<Response, AppError> {
            TenantContext::from_user(&user)?;
            exchange::response($filename, $columns, &[]).map_err(AppError::internal)
        }
        async fn $import(
            State(s): State<InfraState>,
            user: CurrentUser,
            multipart: Multipart,
        ) -> Result<Json<ApiResponse<ImportResult>>, AppError> {
            import(
                &s,
                &TenantContext::from_user(&user)?,
                $spec,
                $columns,
                multipart,
            )
            .await
        }
    };
}
handlers!(
    asset_export,
    asset_template,
    asset_import,
    ASSET,
    ASSET_COLUMNS,
    "assets.csv"
);
handlers!(
    policy_export,
    policy_template,
    policy_import,
    NETWORK_POLICY,
    POLICY_COLUMNS,
    "network-policies.csv"
);

async fn export(
    s: &InfraState,
    tenant: &TenantContext,
    spec: TableSpec,
    columns: &[&str],
    filename: &'static str,
) -> Result<Response, AppError> {
    let rows = sqlx::query_scalar::<_, Value>(&format!(
        "SELECT to_jsonb(t) FROM {} t WHERE tenant_id=$1 AND deleted=0 ORDER BY id LIMIT 10001",
        spec.table
    ))
    .bind(tenant.id())
    .fetch_all(&s.pool)
    .await
    .map_err(|_| AppError::internal("导出读取失败"))?;
    if rows.len() > exchange::MAX_ROWS {
        return Err(AppError::bad_request(
            "单次导出上限为 10000 条，请先缩小数据范围",
        ));
    }
    let rows = rows
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|key| match &row[*key] {
                    Value::Null => String::new(),
                    Value::String(text) => text.clone(),
                    value => value.to_string(),
                })
                .collect()
        })
        .collect::<Vec<_>>();
    exchange::response(filename, columns, &rows).map_err(AppError::internal)
}

fn payload(spec: TableSpec, headers: &[String], row: &[String]) -> Result<Value, String> {
    let mut result = serde_json::Map::new();
    for (key, value) in headers.iter().zip(row) {
        if value.is_empty() {
            continue;
        }
        let parsed = if [
            "publishes_other_endpoint",
            "classified_protection_assessed",
            "classified_protection_filed",
            "cryptography_assessed",
        ]
        .contains(&key.as_str())
        {
            match value.as_str() {
                "true" => json!(true),
                "false" => json!(false),
                _ => return Err(format!("{key} 必须为 true 或 false")),
            }
        } else if key == "weight" {
            json!(
                value
                    .parse::<i32>()
                    .map_err(|_| format!("{key} 必须为整数"))?
            )
        } else if key == "classified_protection_score" {
            let number = value
                .parse::<f64>()
                .map_err(|_| format!("{key} 必须为数字"))?;
            if !number.is_finite() {
                return Err(format!("{key} 必须为有限数字"));
            }
            json!(number)
        } else if key == "ports" || key == "labels" {
            let parsed: Value =
                serde_json::from_str(value).map_err(|_| format!("{key} 必须为 JSON 数组"))?;
            if !parsed.is_array() {
                return Err(format!("{key} 必须为 JSON 数组"));
            }
            json!(parsed.to_string())
        } else {
            if key.ends_with("_date") || key == "expire_at" {
                if value.len() != 10
                    || chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").is_err()
                {
                    return Err(format!("{key} 必须为 YYYY-MM-DD"));
                }
            }
            json!(value)
        };
        result.insert(key.clone(), parsed);
    }
    let required = if spec.table == ASSET.table {
        &ASSET_COLUMNS[..2]
    } else {
        &POLICY_COLUMNS[..14]
    };
    for key in required {
        if result
            .get(*key)
            .and_then(Value::as_str)
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(format!("缺少必填字段 {key}"));
        }
    }
    if spec.table == ASSET.table {
        result.entry("zone").or_insert(json!("Intranet"));
    } else if ![Some("allow"), Some("deny")].contains(&result.get("action").and_then(Value::as_str))
    {
        return Err("action 必须为 allow 或 deny".into());
    }
    Ok(Value::Object(result))
}

async fn import(
    s: &InfraState,
    tenant: &TenantContext,
    spec: TableSpec,
    columns: &[&str],
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<ImportResult>>, AppError> {
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::bad_request("上传表单无效"))?
    {
        if field.name() == Some("file") {
            if bytes.is_some() {
                return Err(AppError::bad_request("每次只允许一个文件"));
            }
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|_| AppError::bad_request("读取 CSV 失败"))?,
            );
        }
    }
    let bytes = bytes.ok_or_else(|| AppError::bad_request("缺少 file"))?;
    import_bytes(s, tenant, spec, columns, &bytes)
        .await
        .map(|result| Json(ApiResponse::new(result)))
}

async fn import_bytes(
    s: &InfraState,
    tenant: &TenantContext,
    spec: TableSpec,
    columns: &[&str],
    bytes: &[u8],
) -> Result<ImportResult, AppError> {
    let table = exchange::decode(bytes).map_err(AppError::bad_request)?;
    for key in &table.headers {
        if !columns.contains(&key.as_str()) {
            return Err(AppError::bad_request(format!("不支持的列: {key}")));
        }
    }
    let required = if spec.table == ASSET.table {
        &ASSET_COLUMNS[..2]
    } else {
        &POLICY_COLUMNS[..14]
    };
    for key in required {
        if !table.headers.iter().any(|header| header == key) {
            return Err(AppError::bad_request(format!("缺少必填表头 {key}")));
        }
    }
    let mut result = ImportResult::default();
    for (index, row) in table.rows.iter().enumerate() {
        if row.iter().all(|value| value.is_empty()) {
            continue;
        }
        let payload = match payload(spec, &table.headers, row) {
            Ok(payload) => payload,
            Err(error) => {
                result.failed(index + 2, error);
                continue;
            }
        };
        let created = if spec.table == NETWORK_POLICY.table {
            policy_risk::create(&s.pool, tenant, payload).await
        } else {
            repository::create(&s.pool, spec, tenant, payload).await
        };
        match created {
            Ok(Json(created)) => {
                if spec.table == ASSET.table {
                    if let Ok(id) = created.data.parse() {
                        if let Err(error) = auto_attribute_ownership(&s.pool, tenant, id).await {
                            result.failed(
                                index + 2,
                                format!("已创建，但自动归属失败: {}", error.message()),
                            );
                            continue;
                        }
                    }
                }
                result.created += 1;
            }
            Err(error) => result.failed(index + 2, error.message()),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
