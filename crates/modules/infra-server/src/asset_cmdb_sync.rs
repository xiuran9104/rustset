//! Idempotent projection of the active asset ledger into a dedicated CMDB model.
//!
//! The projection owns only the mapped fields. Extra attributes added to the
//! CMDB model are preserved when an existing instance is refreshed. Source
//! soft-deletes intentionally do not delete CMDB instances because they may
//! already participate in relations or operational workflows.

use rustset_cmdb_api::AttrType;
use rustset_framework_database::PgPool;
use rustset_framework_web::AppError;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

const MODEL_CODE: &str = "infra_asset_inventory";
const UNIQUE_KEY: &str = "asset_id";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncSummary {
    pub(crate) model_id: i64,
    pub(crate) model_created: bool,
    pub(crate) total: usize,
    pub(crate) created: usize,
    pub(crate) updated: usize,
    pub(crate) unchanged: usize,
    /// Existing CMDB instances whose source asset is no longer active.
    pub(crate) stale: usize,
}

#[derive(Clone, Copy)]
struct FieldMapping {
    source: &'static str,
    code: &'static str,
    name: &'static str,
    attr_type: AttrType,
    required: bool,
    show_in_list: bool,
}

macro_rules! field {
    ($code:literal, $name:literal, $kind:ident) => {
        FieldMapping {
            source: $code,
            code: $code,
            name: $name,
            attr_type: AttrType::$kind,
            required: false,
            show_in_list: false,
        }
    };
    ($source:literal => $code:literal, $name:literal, $kind:ident, $required:expr, $show:expr) => {
        FieldMapping {
            source: $source,
            code: $code,
            name: $name,
            attr_type: AttrType::$kind,
            required: $required,
            show_in_list: $show,
        }
    };
}

// All business columns of infra_asset: the original discovery fields, the 43
// inventory fields from migration 0007, and the ownership fields from 0013.
const FIELDS: &[FieldMapping] = &[
    field!("id" => "asset_id", "资产台账 ID", Number, true, true),
    field!("name" => "name", "资产名称", Text, true, true),
    field!("ip" => "ip", "IP 地址", Text, true, true),
    field!("zone" => "zone", "网络区域", Text, true, true),
    field!("ports", "端口指纹", Json),
    field!("last_scanned", "最后扫描时间", Text),
    field!("contact_person", "联系人", Text),
    field!("contact_phone", "联系电话", Text),
    field!("owner", "负责人", Text),
    field!("weight" => "weight", "资产权重", Number, false, true),
    field!("labels", "标签", Json),
    field!("os", "扫描识别系统", Text),
    field!("device_type" => "device_type", "设备类型", Text, false, true),
    field!("city", "所属地市", Text),
    field!("district", "所属区县", Text),
    field!("organization_name" => "organization_name", "所属单位", Text, false, true),
    field!("business_department", "业务部门", Text),
    field!("department_contact", "部门对接人", Text),
    field!("application_name" => "application_name", "应用名称", Text, false, true),
    field!("server_name" => "server_name", "服务器名称", Text, false, true),
    field!("hardware_configuration", "硬件配置", Textarea),
    field!("operating_system" => "operating_system", "操作系统", Text, false, true),
    field!("database_type", "数据库类型", Text),
    field!("launch_date", "上线日期", Date),
    field!("decommission_date", "下线日期", Date),
    field!("application_type", "应用类型", Text),
    field!("network_environment", "网络环境", Text),
    field!("internet_ipv4", "互联网 IPv4", Text),
    field!("internet_ipv6", "互联网 IPv6", Text),
    field!("domain_address", "域名地址", Textarea),
    field!("internal_network_ip", "内部网络 IP", Text),
    field!("government_extranet_ip", "政务外网 IP", Text),
    field!("open_ports", "开放端口", Textarea),
    field!("publishing_endpoint", "发布端", Text),
    field!("publishes_other_endpoint", "是否发布其他端", Bool),
    field!("other_endpoint_name", "其他端名称", Text),
    field!(
        "security_product_installation",
        "安全产品安装情况",
        Textarea
    ),
    field!("development_vendor", "开发厂商", Text),
    field!("development_vendor_contact", "开发厂商联系人", Text),
    field!("security_vendor", "安全厂商", Text),
    field!("security_vendor_contact", "安全厂商联系人", Text),
    field!("operations_vendor", "运维厂商", Text),
    field!("operations_vendor_contact", "运维厂商联系人", Text),
    field!("classified_protection_level", "等保等级", Text),
    field!("classified_protection_assessed", "是否开展等保测评", Bool),
    field!("classified_protection_assessor", "等保测评机构", Text),
    field!(
        "classified_protection_assessment_date",
        "等保测评日期",
        Date
    ),
    field!("classified_protection_score", "等保测评分数", Float),
    field!("classified_protection_filed", "是否完成等保备案", Bool),
    field!("classified_protection_filing_date", "等保备案日期", Date),
    field!("classified_protection_filing_number", "等保备案编号", Text),
    field!(
        "classified_protection_filing_authority",
        "等保备案机关",
        Text
    ),
    field!("cryptography_assessed", "是否开展密评", Bool),
    field!("cryptography_assessment_level", "密评等级", Text),
    field!("cryptography_assessment_date", "密评日期", Date),
    field!("cryptography_assessment_number", "密评编号", Text),
    field!("net_zone_id", "归属网段 ID", Number),
    field!("ownership_source", "归属来源", Text),
];

struct AttributeDefinition {
    attr_type: AttrType,
    required: bool,
    choices: Option<Value>,
    default_value: Option<Value>,
}

pub(crate) async fn sync_assets(pool: &PgPool, actor: &str) -> Result<SyncSummary, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start asset CMDB sync"))?;

    // cmdb_model.code currently has a non-unique partial index. Use the same
    // transaction lock as every other model creator in the gateway.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("cmdb:model:{MODEL_CODE}"))
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to lock asset CMDB model code"))?;

    let model_rows = sqlx::query(
        "SELECT id, unique_key, status FROM cmdb_model
         WHERE code = $1 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(MODEL_CODE)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to find asset CMDB model"))?;
    if model_rows.len() > 1 {
        return Err(AppError::bad_request(format!(
            "CMDB model code {MODEL_CODE:?} is duplicated; resolve it before syncing"
        )));
    }
    let (model_id, model_created) = if let Some(row) = model_rows.first() {
        let status: i16 = row.get("status");
        if status != 0 {
            return Err(AppError::bad_request(
                "the asset inventory CMDB model is disabled",
            ));
        }
        let unique_key: Option<String> = row.get("unique_key");
        if unique_key.as_deref() != Some(UNIQUE_KEY) {
            return Err(AppError::bad_request(format!(
                "CMDB model {MODEL_CODE:?} must use {UNIQUE_KEY:?} as its unique key"
            )));
        }
        (row.get("id"), false)
    } else {
        let id = sqlx::query_scalar(
            "INSERT INTO cmdb_model
             (name, code, description, icon, unique_key, sort, creator, updater)
             VALUES ('资产台账', $1, '由资产管理台账同步；asset_id 对应 infra_asset.id',
                     'lucide:server', $2, 10, $3, $3)
             RETURNING id",
        )
        .bind(MODEL_CODE)
        .bind(UNIQUE_KEY)
        .bind(actor)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to create asset CMDB model"))?;
        (id, true)
    };

    ensure_attributes(&mut tx, model_id, actor).await?;
    let definitions = load_definitions(&mut tx, model_id).await?;

    let source_rows: Vec<Value> =
        sqlx::query_scalar("SELECT to_jsonb(a) FROM infra_asset a WHERE deleted = 0 ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to read the active asset ledger"))?;
    let mut sources = BTreeMap::new();
    for row in source_rows {
        let attributes = map_asset(row)?;
        let asset_id = attributes
            .get(UNIQUE_KEY)
            .and_then(Value::as_i64)
            .ok_or_else(|| AppError::internal("mapped asset has no numeric asset_id"))?;
        apply_defaults_and_validate(&definitions, &mut attributes.clone()).map_err(|error| {
            AppError::bad_request(format!(
                "asset {asset_id} cannot be mapped to CMDB: {error}"
            ))
        })?;
        sources.insert(asset_id, attributes);
    }

    let instance_rows = sqlx::query(
        "SELECT id, attributes FROM cmdb_instance
         WHERE model_id = $1 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(model_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to lock asset CMDB instances"))?;
    let mut instances = BTreeMap::new();
    for row in instance_rows {
        let instance_id: i64 = row.get("id");
        let attributes: Value = row.get("attributes");
        let object = attributes.as_object().cloned().ok_or_else(|| {
            AppError::bad_request(format!(
                "CMDB instance {instance_id} has a non-object attributes payload"
            ))
        })?;
        let asset_id = object
            .get(UNIQUE_KEY)
            .and_then(Value::as_i64)
            .ok_or_else(|| {
                AppError::bad_request(format!(
                    "CMDB instance {instance_id} has no numeric {UNIQUE_KEY}"
                ))
            })?;
        if instances.insert(asset_id, (instance_id, object)).is_some() {
            return Err(AppError::bad_request(format!(
                "multiple CMDB instances use {UNIQUE_KEY} {asset_id}"
            )));
        }
    }

    let mapped_codes: BTreeSet<&str> = FIELDS.iter().map(|field| field.code).collect();
    let total = sources.len();
    let stale = instances
        .keys()
        .filter(|asset_id| !sources.contains_key(asset_id))
        .count();
    let mut created = 0;
    let mut updated = 0;
    let mut unchanged = 0;
    for (asset_id, source) in sources {
        if let Some((instance_id, mut merged)) = instances.remove(&asset_id) {
            let existing = Value::Object(merged.clone());
            for code in &mapped_codes {
                merged.remove(*code);
            }
            merged.extend(source);
            apply_defaults_and_validate(&definitions, &mut merged).map_err(|error| {
                AppError::bad_request(format!(
                    "CMDB instance {instance_id} cannot be synchronized: {error}"
                ))
            })?;
            let merged = Value::Object(merged);
            if existing == merged {
                unchanged += 1;
            } else {
                sqlx::query(
                    "UPDATE cmdb_instance
                     SET attributes = $2, updater = $3, update_time = now()
                     WHERE id = $1 AND deleted = 0",
                )
                .bind(instance_id)
                .bind(merged)
                .bind(actor)
                .execute(&mut *tx)
                .await
                .map_err(|_| AppError::internal("failed to update asset CMDB instance"))?;
                updated += 1;
            }
        } else {
            let mut attributes = source;
            apply_defaults_and_validate(&definitions, &mut attributes).map_err(|error| {
                AppError::bad_request(format!("asset {asset_id} cannot be created: {error}"))
            })?;
            sqlx::query(
                "INSERT INTO cmdb_instance (model_id, attributes, creator, updater)
                 VALUES ($1, $2, $3, $3)",
            )
            .bind(model_id)
            .bind(Value::Object(attributes))
            .bind(actor)
            .execute(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to create asset CMDB instance"))?;
            created += 1;
        }
    }

    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit asset CMDB sync"))?;
    Ok(SyncSummary {
        model_id,
        model_created,
        total,
        created,
        updated,
        unchanged,
        stale,
    })
}

async fn ensure_attributes(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
    actor: &str,
) -> Result<(), AppError> {
    let rows = sqlx::query(
        "SELECT code, attr_type, required FROM cmdb_attribute
         WHERE model_id = $1 AND deleted = 0 ORDER BY id FOR UPDATE",
    )
    .bind(model_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| AppError::internal("failed to inspect asset CMDB attributes"))?;
    let mut existing = BTreeMap::new();
    for row in rows {
        let code: String = row.get("code");
        if existing
            .insert(
                code.clone(),
                (
                    row.get::<String, _>("attr_type"),
                    row.get::<bool, _>("required"),
                ),
            )
            .is_some()
        {
            return Err(AppError::bad_request(format!(
                "CMDB model {MODEL_CODE:?} has duplicate attribute {code:?}"
            )));
        }
    }
    for (sort, field) in FIELDS.iter().enumerate() {
        if let Some((attr_type, required)) = existing.get(field.code) {
            if attr_type != field.attr_type.code() || *required != field.required {
                return Err(AppError::bad_request(format!(
                    "CMDB attribute {:?} is incompatible with the asset mapping; expected type {} and required={} ",
                    field.code,
                    field.attr_type.code(),
                    field.required
                )));
            }
            continue;
        }
        sqlx::query(
            "INSERT INTO cmdb_attribute
             (model_id, name, code, attr_type, required, show_in_list, sort, creator, updater)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $8)",
        )
        .bind(model_id)
        .bind(field.name)
        .bind(field.code)
        .bind(field.attr_type.code())
        .bind(field.required)
        .bind(field.show_in_list)
        .bind(sort as i32)
        .bind(actor)
        .execute(&mut *connection)
        .await
        .map_err(|_| AppError::internal("failed to create asset CMDB attribute"))?;
    }
    Ok(())
}

async fn load_definitions(
    connection: &mut sqlx::PgConnection,
    model_id: i64,
) -> Result<BTreeMap<String, AttributeDefinition>, AppError> {
    let rows = sqlx::query(
        "SELECT code, attr_type, required, choices, default_value
         FROM cmdb_attribute WHERE model_id = $1 AND deleted = 0 ORDER BY id",
    )
    .bind(model_id)
    .fetch_all(&mut *connection)
    .await
    .map_err(|_| AppError::internal("failed to load asset CMDB attributes"))?;
    let mut definitions = BTreeMap::new();
    for row in rows {
        let code: String = row.get("code");
        let type_code: String = row.get("attr_type");
        let attr_type = AttrType::from_code(&type_code).ok_or_else(|| {
            AppError::bad_request(format!(
                "CMDB attribute {code:?} uses unknown type {type_code:?}"
            ))
        })?;
        if definitions
            .insert(
                code.clone(),
                AttributeDefinition {
                    attr_type,
                    required: row.get("required"),
                    choices: row.get("choices"),
                    default_value: row.get("default_value"),
                },
            )
            .is_some()
        {
            return Err(AppError::bad_request(format!(
                "CMDB model has duplicate attribute {code:?}"
            )));
        }
    }
    Ok(definitions)
}

fn map_asset(row: Value) -> Result<Map<String, Value>, AppError> {
    let source = row
        .as_object()
        .ok_or_else(|| AppError::internal("asset row is not a JSON object"))?;
    let mut attributes = Map::new();
    for field in FIELDS {
        let Some(mut value) = source.get(field.source).cloned() else {
            if field.required {
                return Err(AppError::internal(format!(
                    "asset row is missing required column {:?}",
                    field.source
                )));
            }
            continue;
        };
        if value.is_null() || value.as_str().is_some_and(|text| text.trim().is_empty()) {
            if field.required {
                return Err(AppError::bad_request(format!(
                    "asset field {:?} is empty",
                    field.source
                )));
            }
            continue;
        }
        if matches!(field.code, "ports" | "labels") && value.is_string() {
            value = serde_json::from_str(value.as_str().unwrap_or_default()).map_err(|_| {
                AppError::bad_request(format!(
                    "asset field {:?} does not contain valid JSON",
                    field.source
                ))
            })?;
        }
        field.attr_type.validate(&value, None).map_err(|reason| {
            AppError::bad_request(format!("asset field {:?} {reason}", field.source))
        })?;
        attributes.insert(field.code.to_string(), value);
    }
    Ok(attributes)
}

fn apply_defaults_and_validate(
    definitions: &BTreeMap<String, AttributeDefinition>,
    attributes: &mut Map<String, Value>,
) -> Result<(), String> {
    let unknown: Vec<_> = attributes
        .keys()
        .filter(|code| !definitions.contains_key(*code))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(format!("unknown attributes: {}", unknown.join(", ")));
    }
    for (code, definition) in definitions {
        let empty = attributes.get(code).is_none_or(is_empty_value);
        if empty {
            if let Some(default) = definition
                .default_value
                .as_ref()
                .filter(|value| !is_empty_value(value))
            {
                definition
                    .attr_type
                    .validate(default, definition.choices.as_ref())
                    .map_err(|reason| format!("attribute {code:?} default {reason}"))?;
                attributes.insert(code.clone(), default.clone());
            } else if definition.required {
                return Err(format!("required attribute {code:?} has no value"));
            }
            continue;
        }
        let value = attributes.get(code).unwrap_or(&Value::Null);
        definition
            .attr_type
            .validate(value, definition.choices.as_ref())
            .map_err(|reason| format!("attribute {code:?} {reason}"))?;
    }
    Ok(())
}

fn is_empty_value(value: &Value) -> bool {
    value.is_null()
        || value.as_str().is_some_and(|text| text.trim().is_empty())
        || value.as_array().is_some_and(Vec::is_empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn mapping_covers_the_inventory_schema_without_duplicate_codes() {
        assert_eq!(FIELDS.len(), 58);
        let codes: BTreeSet<_> = FIELDS.iter().map(|field| field.code).collect();
        assert_eq!(codes.len(), FIELDS.len());
        assert_eq!(FIELDS.first().map(|field| field.code), Some(UNIQUE_KEY));
    }

    #[test]
    fn maps_source_id_and_json_collections_and_omits_empty_optionals() {
        let mut row = Map::new();
        for field in FIELDS {
            let value = match field.attr_type {
                AttrType::Number => json!(1),
                AttrType::Float => json!(1.5),
                AttrType::Bool => json!(false),
                AttrType::Date => json!("2026-09-28"),
                AttrType::Json => json!("[]"),
                _ => json!("value"),
            };
            row.insert(field.source.to_string(), value);
        }
        row.insert("contact_phone".into(), json!(""));
        let mapped = map_asset(Value::Object(row)).unwrap();
        assert_eq!(mapped[UNIQUE_KEY], 1);
        assert_eq!(mapped["ports"], json!([]));
        assert_eq!(mapped["labels"], json!([]));
        assert!(!mapped.contains_key("contact_phone"));
    }

    #[test]
    fn rejects_malformed_collection_json() {
        let row = json!({
            "id": 1,
            "name": "server",
            "ip": "10.0.0.1",
            "zone": "Intranet",
            "ports": "not-json"
        });
        assert!(map_asset(row).is_err());
    }

    #[tokio::test]
    #[ignore = "requires TEST_DATABASE_URL pointing at PostgreSQL"]
    async fn sync_is_concurrent_safe_idempotent_and_preserves_extensions() {
        use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
        use std::str::FromStr;

        let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
        let admin = sqlx::PgPool::connect(&url).await.unwrap();
        let schema = format!("asset_cmdb_sync_test_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let options = PgConnectOptions::from_str(&url)
            .unwrap()
            .options([("search_path", schema.as_str())]);
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .unwrap();

        let cmdb_ddl = include_str!("../../../../sql/postgresql/0009_cmdb_core.sql")
            .split("-- Menus:")
            .next()
            .unwrap()
            .replace("public.", "");
        sqlx::raw_sql(&cmdb_ddl).execute(&pool).await.unwrap();
        let mut columns = vec!["id bigint".to_string()];
        columns.extend(FIELDS.iter().skip(1).map(|field| {
            let sql_type = match field.attr_type {
                AttrType::Number => "bigint",
                AttrType::Float => "double precision",
                AttrType::Bool => "boolean",
                AttrType::Date => "date",
                AttrType::Json => "text",
                _ => "text",
            };
            format!("{} {sql_type}", field.source)
        }));
        columns.push("deleted smallint DEFAULT 0 NOT NULL".to_string());
        sqlx::raw_sql(&format!("CREATE TABLE infra_asset ({})", columns.join(",")))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO infra_asset (id, name, ip, zone, ports, labels, weight)
             VALUES (1, 'server-1', '10.0.0.1', 'Intranet', '[]', '[]', 50)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let (first, second) =
            tokio::join!(sync_assets(&pool, "tester"), sync_assets(&pool, "tester"));
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(first.created + second.created, 1);
        assert_eq!(first.unchanged + second.unchanged, 1);
        let models: i64 =
            sqlx::query_scalar("SELECT count(*) FROM cmdb_model WHERE code = $1 AND deleted = 0")
                .bind(MODEL_CODE)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(models, 1);

        let model_id: i64 = sqlx::query_scalar("SELECT id FROM cmdb_model WHERE code = $1")
            .bind(MODEL_CODE)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO cmdb_attribute
             (model_id, name, code, attr_type, creator, updater)
             VALUES ($1, '运维备注', 'operations_note', 'text', 'tester', 'tester')",
        )
        .bind(model_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE cmdb_instance
             SET attributes = jsonb_set(attributes, '{operations_note}', $1::jsonb)",
        )
        .bind(json!("manual"))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE infra_asset SET name = 'server-renamed' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let refreshed = sync_assets(&pool, "tester").await.unwrap();
        assert_eq!(refreshed.updated, 1);
        let attributes: Value = sqlx::query_scalar("SELECT attributes FROM cmdb_instance")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(attributes["name"], "server-renamed");
        assert_eq!(attributes["operations_note"], "manual");

        sqlx::query("UPDATE infra_asset SET deleted = 1 WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        let without_source = sync_assets(&pool, "tester").await.unwrap();
        assert_eq!(without_source.total, 0);
        assert_eq!(without_source.stale, 1);
        let instances: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM cmdb_instance WHERE model_id = $1 AND deleted = 0",
        )
        .bind(model_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(instances, 1);

        pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }
}
