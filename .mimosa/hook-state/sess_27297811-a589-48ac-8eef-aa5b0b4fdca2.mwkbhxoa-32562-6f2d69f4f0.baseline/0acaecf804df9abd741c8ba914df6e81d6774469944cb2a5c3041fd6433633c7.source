use axum::Json;
use rustset_framework_common::ApiResponse;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct DictDataSummary {
    #[serde(rename = "colorType")]
    color_type: String,
    #[serde(rename = "createTime")]
    create_time: Option<String>,
    #[serde(rename = "cssClass")]
    css_class: String,
    #[serde(rename = "dictType")]
    dict_type: String,
    id: Option<i64>,
    label: String,
    remark: String,
    sort: Option<i32>,
    status: i32,
    value: String,
}

pub async fn simple_list() -> Json<ApiResponse<Vec<DictDataSummary>>> {
    Json(ApiResponse::new(Vec::new()))
}
