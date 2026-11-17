use super::*;

#[test]
fn network_policy_import_validates_action_and_required_columns() {
    let headers = POLICY_COLUMNS
        .iter()
        .map(|column| (*column).to_string())
        .collect::<Vec<_>>();
    let mut row = vec![String::new(); headers.len()];
    for (index, value) in [
        "核心防火墙",
        "目的单位",
        "目的项目",
        "源单位",
        "源项目",
        "互联网域",
        "10.0.0.0/24",
        "服务器域",
        "10.1.0.10",
        "tcp/3389",
        "申请人",
        "2026-09-27",
        "正向",
        "allow",
    ]
    .into_iter()
    .enumerate()
    {
        row[index] = value.into();
    }
    assert!(payload(NETWORK_POLICY, &headers, &row).is_ok());
    row[13] = "unknown".into();
    assert_eq!(
        payload(NETWORK_POLICY, &headers, &row).unwrap_err(),
        "action 必须为 allow 或 deny"
    );
}

#[test]
fn asset_import_requires_name_and_ip() {
    let headers = ASSET_COLUMNS
        .iter()
        .map(|column| (*column).to_string())
        .collect::<Vec<_>>();
    let row = vec![String::new(); headers.len()];
    assert_eq!(
        payload(ASSET, &headers, &row).unwrap_err(),
        "缺少必填字段 name"
    );
}
