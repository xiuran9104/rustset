use aide::axum::ApiRouter;
use aide::axum::routing::{get, post};
use axum::{
    Json,
    extract::{DefaultBodyLimit, Multipart, State},
    response::Response,
};
use chrono::NaiveDateTime;
use rustset_framework_common::{ApiResponse, csv};
use rustset_framework_security::CurrentUser;
use rustset_framework_web::AppError;
use sqlx::Row;

use crate::SystemState;

use super::shared::require;

const COLUMNS: &[&str] = &[
    "name",
    "contact_name",
    "contact_mobile",
    "status",
    "websites",
    "package_id",
    "expire_time",
    "account_count",
];

pub(super) fn routes() -> ApiRouter<SystemState> {
    ApiRouter::new()
        .api_route("/system/tenant/export-csv", get(export))
        .api_route("/system/tenant/import-template", get(template))
        .api_route("/system/tenant/import-csv", post(import))
        .layer(DefaultBodyLimit::max(csv::MAX_BYTES + 64 * 1024))
}

async fn export(State(state): State<SystemState>, user: CurrentUser) -> Result<Response, AppError> {
    require(&user, "system:tenant:export")?;
    let records = sqlx::query(
        "SELECT name,contact_name,contact_mobile,status,websites,package_id,
                expire_time,account_count
         FROM system_tenant WHERE deleted=0 ORDER BY id LIMIT 10001",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|_| AppError::internal("导出租户失败"))?;
    if records.len() > csv::MAX_ROWS {
        return Err(AppError::bad_request("单次导出上限为 10000 条"));
    }
    let rows = records
        .into_iter()
        .map(|row| {
            vec![
                row.get::<String, _>("name"),
                row.get::<String, _>("contact_name"),
                row.get::<Option<String>, _>("contact_mobile")
                    .unwrap_or_default(),
                row.get::<i16, _>("status").to_string(),
                row.get::<Option<String>, _>("websites").unwrap_or_default(),
                row.get::<i64, _>("package_id").to_string(),
                row.get::<NaiveDateTime, _>("expire_time").to_string(),
                row.get::<i32, _>("account_count").to_string(),
            ]
        })
        .collect::<Vec<_>>();
    csv::response("system-tenants.csv", COLUMNS, &rows).map_err(AppError::internal)
}

async fn template(user: CurrentUser) -> Result<Response, AppError> {
    require(&user, "system:tenant:create")?;
    csv::response("system-tenants-template.csv", COLUMNS, &[]).map_err(AppError::internal)
}

async fn import(
    State(state): State<SystemState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<csv::ImportResult>>, AppError> {
    require(&user, "system:tenant:create")?;
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
    let table = csv::decode(&bytes.ok_or_else(|| AppError::bad_request("缺少 file"))?)
        .map_err(AppError::bad_request)?;
    for header in &table.headers {
        if !COLUMNS.contains(&header.as_str()) {
            return Err(AppError::bad_request(format!("不支持的列: {header}")));
        }
    }
    for required in ["name", "contact_name", "package_id"] {
        if !table.headers.iter().any(|header| header == required) {
            return Err(AppError::bad_request(format!("缺少必填表头 {required}")));
        }
    }

    let mut result = csv::ImportResult::default();
    for (index, row) in table.rows.iter().enumerate() {
        if row.iter().all(|value| value.is_empty()) {
            continue;
        }
        let value = |name: &str| {
            table
                .headers
                .iter()
                .position(|header| header == name)
                .and_then(|position| row.get(position))
                .map(String::as_str)
                .unwrap_or("")
                .trim()
        };
        let row_no = index + 2;
        let name = value("name");
        let contact_name = value("contact_name");
        if name.is_empty() || contact_name.is_empty() {
            result.failed(row_no, "name 和 contact_name 不能为空");
            continue;
        }
        let package_id = match value("package_id").parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                result.failed(row_no, "package_id 必须为正整数");
                continue;
            }
        };
        let status = match parse_number::<i16>(value("status"), 0, "status") {
            Ok(value) if matches!(value, 0 | 1) => value,
            _ => {
                result.failed(row_no, "status 必须为 0 或 1");
                continue;
            }
        };
        let account_count = match parse_number::<i32>(value("account_count"), 100, "account_count")
        {
            Ok(value) if value > 0 => value,
            _ => {
                result.failed(row_no, "account_count 必须为正整数");
                continue;
            }
        };
        let expire_time = if value("expire_time").is_empty() {
            "2099-12-31 23:59:59"
        } else {
            value("expire_time")
        };
        if NaiveDateTime::parse_from_str(expire_time, "%Y-%m-%d %H:%M:%S").is_err() {
            result.failed(row_no, "expire_time 必须为 YYYY-MM-DD HH:MM:SS");
            continue;
        }

        let inserted = sqlx::query_scalar::<_, i64>(
            "INSERT INTO system_tenant
                (id,name,contact_name,contact_mobile,status,websites,package_id,
                 expire_time,account_count,creator,updater)
             SELECT nextval('system_tenant_seq'),$1,$2,NULLIF($3,''),$4,$5,$6,$7::timestamp,$8,$9,$9
             WHERE EXISTS(
                 SELECT 1 FROM system_tenant_package
                 WHERE id=$6 AND status=0 AND deleted=0)
             RETURNING id",
        )
        .bind(name)
        .bind(contact_name)
        .bind(value("contact_mobile"))
        .bind(status)
        .bind(value("websites"))
        .bind(package_id)
        .bind(expire_time)
        .bind(account_count)
        .bind(&user.username)
        .fetch_optional(&state.pool)
        .await;
        match inserted {
            Ok(Some(_)) => result.created += 1,
            Ok(None) => result.failed(row_no, "package_id 不存在或未启用"),
            Err(error) => result.failed(row_no, tenant_insert_error(error)),
        }
    }
    Ok(Json(ApiResponse::new(result)))
}

fn parse_number<T>(value: &str, default: T, field: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    if value.is_empty() {
        Ok(default)
    } else {
        value.parse().map_err(|_| format!("{field} 必须为整数"))
    }
}

fn tenant_insert_error(error: sqlx::Error) -> String {
    match error {
        sqlx::Error::Database(database) => database.message().to_owned(),
        _ => "创建租户失败".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_csv_columns_are_stable() {
        let encoded = csv::encode(COLUMNS, &[]).unwrap();
        assert_eq!(csv::decode(&encoded).unwrap().headers, COLUMNS);
    }
}
