//! Centralized route authorization for the infra module.
//!
//! Every infra route must appear in `ROUTE_PERMISSIONS` with the permission
//! code required to call it (migration plan §4.2: an explicit registry, and
//! unregistered operations are denied by default). The middleware runs after
//! the gateway-wide database authentication, so a request reaching this layer
//! without a `CurrentUser` was never authenticated.
//!
//! `super_admin` bypasses permission checks, matching
//! `system_server::management::shared::require`.

use axum::{
    extract::{MatchedPath, Request},
    http::Method,
    middleware::Next,
    response::Response,
};
use rustset_framework_security::{CurrentUser, Permission};
use rustset_framework_web::AppError;

/// (HTTP method, registered route pattern, required permission).
/// None = any authenticated user may call (route is not permission-gated yet).
static ROUTE_PERMISSIONS: &[(&str, &str, Option<&str>)] = &[
    (
        "DELETE",
        "/infra/application-endpoint/delete",
        Some("infra:application-endpoint:delete"),
    ),
    (
        "DELETE",
        "/infra/application-endpoint/delete-list",
        Some("infra:application-endpoint:delete"),
    ),
    (
        "DELETE",
        "/infra/approval-rule/delete",
        Some("infra:approval-rule:delete"),
    ),
    ("DELETE", "/infra/asset/delete", Some("infra:asset:delete")),
    (
        "DELETE",
        "/infra/asset/delete-list",
        Some("infra:asset:delete"),
    ),
    (
        "DELETE",
        "/infra/asset/{id}/port/{port}",
        Some("infra:asset:update"),
    ),
    (
        "DELETE",
        "/infra/business-application/delete",
        Some("infra:business-application:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-asset/delete",
        Some("infra:cloud-asset:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-platform/delete",
        Some("infra:cloud-platform:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-platform/delete-list",
        Some("infra:cloud-platform:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-provider-config/delete",
        Some("infra:cloud-provider-config:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-provider-config/delete-list",
        Some("infra:cloud-provider-config:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-resource/delete",
        Some("infra:cloud-resource:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-resource/delete-list",
        Some("infra:cloud-resource:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-zone/delete",
        Some("infra:cloud-zone:delete"),
    ),
    (
        "DELETE",
        "/infra/cloud-zone/delete-list",
        Some("infra:cloud-zone:delete"),
    ),
    (
        "DELETE",
        "/infra/codegen/delete",
        Some("infra:codegen:delete"),
    ),
    (
        "DELETE",
        "/infra/codegen/delete-list",
        Some("infra:codegen:delete"),
    ),
    (
        "DELETE",
        "/infra/config/delete",
        Some("infra:config:delete"),
    ),
    (
        "DELETE",
        "/infra/config/delete-list",
        Some("infra:config:delete"),
    ),
    (
        "DELETE",
        "/infra/data-source-config/delete",
        Some("infra:data-source-config:delete"),
    ),
    (
        "DELETE",
        "/infra/data-source-config/delete-list",
        Some("infra:data-source-config:delete"),
    ),
    (
        "DELETE",
        "/infra/file-config/delete",
        Some("infra:file-config:delete"),
    ),
    (
        "DELETE",
        "/infra/file-config/delete-list",
        Some("infra:file-config:delete"),
    ),
    ("DELETE", "/infra/file/delete", Some("infra:file:delete")),
    (
        "DELETE",
        "/infra/file/delete-list",
        Some("infra:file:delete"),
    ),
    ("DELETE", "/infra/job/delete", Some("infra:job:delete")),
    ("DELETE", "/infra/job/delete-list", Some("infra:job:delete")),
    (
        "DELETE",
        "/infra/machine-room/delete",
        Some("infra:machine-room:delete"),
    ),
    (
        "DELETE",
        "/infra/machine-room/delete-list",
        Some("infra:machine-room:delete"),
    ),
    (
        "DELETE",
        "/infra/network-policy/delete",
        Some("infra:network-policy:delete"),
    ),
    (
        "DELETE",
        "/infra/network-policy/delete-list",
        Some("infra:network-policy:delete"),
    ),
    (
        "DELETE",
        "/infra/network-zone/delete",
        Some("infra:network-zone:delete"),
    ),
    (
        "DELETE",
        "/infra/network-zone/delete-list",
        Some("infra:network-zone:delete"),
    ),
    (
        "DELETE",
        "/infra/physical-resource/delete",
        Some("infra:physical-resource:delete"),
    ),
    (
        "DELETE",
        "/infra/physical-resource/delete-list",
        Some("infra:physical-resource:delete"),
    ),
    (
        "DELETE",
        "/infra/resource-ticket/delete",
        Some("infra:resource-ticket:delete"),
    ),
    (
        "DELETE",
        "/infra/resource-ticket/delete-list",
        Some("infra:resource-ticket:delete"),
    ),
    (
        "DELETE",
        "/infra/security-product/delete",
        Some("infra:security-product:delete"),
    ),
    (
        "DELETE",
        "/infra/security-product/delete-list",
        Some("infra:security-product:delete"),
    ),
    (
        "DELETE",
        "/infra/service-provider/delete",
        Some("infra:service-provider:delete"),
    ),
    (
        "DELETE",
        "/infra/service-provider/delete-list",
        Some("infra:service-provider:delete"),
    ),
    ("DELETE", "/infra/task/delete", Some("infra:task:delete")),
    (
        "DELETE",
        "/infra/task/delete-list",
        Some("infra:task:delete"),
    ),
    (
        "GET",
        "/infra/api-access-log/export-excel",
        Some("infra:api-access-log:export"),
    ),
    (
        "GET",
        "/infra/api-access-log/page",
        Some("infra:api-access-log:query"),
    ),
    (
        "GET",
        "/infra/api-error-log/export-excel",
        Some("infra:api-error-log:export"),
    ),
    (
        "GET",
        "/infra/api-error-log/page",
        Some("infra:api-error-log:query"),
    ),
    (
        "GET",
        "/infra/application-endpoint/get",
        Some("infra:application-endpoint:query"),
    ),
    (
        "GET",
        "/infra/application-endpoint/list",
        Some("infra:application-endpoint:query"),
    ),
    (
        "GET",
        "/infra/application-endpoint/list-by-app",
        Some("infra:application-endpoint:query"),
    ),
    (
        "GET",
        "/infra/application-endpoint/page",
        Some("infra:application-endpoint:query"),
    ),
    (
        "GET",
        "/infra/approval-rule/list",
        Some("infra:approval-rule:query"),
    ),
    (
        "GET",
        "/infra/approval-rule/page",
        Some("infra:approval-rule:query"),
    ),
    ("GET", "/infra/asset/export-csv", Some("infra:asset:query")),
    ("GET", "/infra/asset/get", Some("infra:asset:query")),
    (
        "GET",
        "/infra/asset/import-template",
        Some("infra:asset:create"),
    ),
    ("GET", "/infra/asset/list", Some("infra:asset:query")),
    ("GET", "/infra/asset/page", Some("infra:asset:query")),
    (
        "GET",
        "/infra/business-application/get",
        Some("infra:business-application:query"),
    ),
    (
        "GET",
        "/infra/business-application/list",
        Some("infra:business-application:query"),
    ),
    (
        "GET",
        "/infra/business-application/page",
        Some("infra:business-application:query"),
    ),
    ("GET", "/infra/capabilities", None),
    (
        "GET",
        "/infra/cloud-asset/get",
        Some("infra:cloud-asset:query"),
    ),
    (
        "GET",
        "/infra/cloud-asset/list",
        Some("infra:cloud-asset:query"),
    ),
    (
        "GET",
        "/infra/cloud-asset/page",
        Some("infra:cloud-asset:query"),
    ),
    (
        "GET",
        "/infra/cloud-platform/get",
        Some("infra:cloud-platform:query"),
    ),
    (
        "GET",
        "/infra/cloud-platform/list",
        Some("infra:cloud-platform:query"),
    ),
    (
        "GET",
        "/infra/cloud-platform/list-by-zone",
        Some("infra:cloud-platform:query"),
    ),
    (
        "GET",
        "/infra/cloud-platform/page",
        Some("infra:cloud-platform:query"),
    ),
    (
        "GET",
        "/infra/cloud-provider-config/get",
        Some("infra:cloud-provider-config:query"),
    ),
    (
        "GET",
        "/infra/cloud-provider-config/list",
        Some("infra:cloud-provider-config:query"),
    ),
    (
        "GET",
        "/infra/cloud-provider-config/page",
        Some("infra:cloud-provider-config:query"),
    ),
    (
        "GET",
        "/infra/cloud-resource/get",
        Some("infra:cloud-resource:query"),
    ),
    (
        "GET",
        "/infra/cloud-resource/list",
        Some("infra:cloud-resource:query"),
    ),
    (
        "GET",
        "/infra/cloud-resource/page",
        Some("infra:cloud-resource:query"),
    ),
    (
        "GET",
        "/infra/cloud-zone/get",
        Some("infra:cloud-zone:query"),
    ),
    (
        "GET",
        "/infra/cloud-zone/list",
        Some("infra:cloud-zone:query"),
    ),
    (
        "GET",
        "/infra/cloud-zone/page",
        Some("infra:cloud-zone:query"),
    ),
    (
        "GET",
        "/infra/codegen/db/table/list",
        Some("infra:codegen:query"),
    ),
    ("GET", "/infra/codegen/detail", Some("infra:codegen:query")),
    (
        "GET",
        "/infra/codegen/download",
        Some("infra:codegen:query"),
    ),
    ("GET", "/infra/codegen/preview", Some("infra:codegen:query")),
    (
        "GET",
        "/infra/codegen/table/list",
        Some("infra:codegen:query"),
    ),
    (
        "GET",
        "/infra/codegen/table/page",
        Some("infra:codegen:query"),
    ),
    (
        "GET",
        "/infra/config/export-excel",
        Some("infra:config:export"),
    ),
    ("GET", "/infra/config/get", Some("infra:config:query")),
    (
        "GET",
        "/infra/config/get-value-by-key",
        Some("infra:config:query"),
    ),
    ("GET", "/infra/config/page", Some("infra:config:query")),
    (
        "GET",
        "/infra/data-source-config/get",
        Some("infra:data-source-config:query"),
    ),
    (
        "GET",
        "/infra/data-source-config/list",
        Some("infra:data-source-config:query"),
    ),
    (
        "GET",
        "/infra/file-config/get",
        Some("infra:file-config:query"),
    ),
    (
        "GET",
        "/infra/file-config/page",
        Some("infra:file-config:query"),
    ),
    (
        "GET",
        "/infra/file-config/test",
        Some("infra:file-config:query"),
    ),
    ("GET", "/infra/file/page", Some("infra:file:query")),
    ("GET", "/infra/file/presigned-url", Some("infra:file:query")),
    (
        "GET",
        "/infra/inspection/baseline",
        Some("infra:asset:query"),
    ),
    ("GET", "/infra/inspection/list", Some("infra:task:query")),
    ("GET", "/infra/inspection/results", Some("infra:task:query")),
    (
        "GET",
        "/infra/job-log/export-excel",
        Some("infra:job:export"),
    ),
    ("GET", "/infra/job-log/get", Some("infra:job:query")),
    ("GET", "/infra/job-log/page", Some("infra:job:query")),
    ("GET", "/infra/job/export-excel", Some("infra:job:export")),
    ("GET", "/infra/job/get", Some("infra:job:query")),
    ("GET", "/infra/job/get_next_times", Some("infra:job:query")),
    ("GET", "/infra/job/page", Some("infra:job:query")),
    (
        "GET",
        "/infra/machine-room/get",
        Some("infra:machine-room:query"),
    ),
    (
        "GET",
        "/infra/machine-room/list",
        Some("infra:machine-room:query"),
    ),
    (
        "GET",
        "/infra/machine-room/page",
        Some("infra:machine-room:query"),
    ),
    ("GET", "/infra/monitor/postgresql", None),
    ("GET", "/infra/monitor/rust", None),
    ("GET", "/infra/monitor/traces", None),
    (
        "GET",
        "/infra/network-policy/export-csv",
        Some("infra:network-policy:query"),
    ),
    (
        "GET",
        "/infra/network-policy/get",
        Some("infra:network-policy:query"),
    ),
    (
        "GET",
        "/infra/network-policy/import-template",
        Some("infra:network-policy:create"),
    ),
    (
        "GET",
        "/infra/network-policy/list",
        Some("infra:network-policy:query"),
    ),
    (
        "GET",
        "/infra/network-policy/page",
        Some("infra:network-policy:query"),
    ),
    (
        "GET",
        "/infra/network-zone/get",
        Some("infra:network-zone:query"),
    ),
    (
        "GET",
        "/infra/network-zone/list",
        Some("infra:network-zone:query"),
    ),
    (
        "GET",
        "/infra/network-zone/page",
        Some("infra:network-zone:query"),
    ),
    (
        "GET",
        "/infra/physical-resource/get",
        Some("infra:physical-resource:query"),
    ),
    (
        "GET",
        "/infra/physical-resource/list",
        Some("infra:physical-resource:query"),
    ),
    (
        "GET",
        "/infra/physical-resource/page",
        Some("infra:physical-resource:query"),
    ),
    (
        "GET",
        "/infra/redis/get-monitor-info",
        Some("infra:redis:query"),
    ),
    (
        "GET",
        "/infra/resource-ticket/get",
        Some("infra:resource-ticket:query"),
    ),
    (
        "GET",
        "/infra/resource-ticket/page",
        Some("infra:resource-ticket:query"),
    ),
    ("GET", "/infra/risk/get", Some("infra:risk:query")),
    ("GET", "/infra/risk/list", Some("infra:risk:query")),
    ("GET", "/infra/risk/page", Some("infra:risk:query")),
    (
        "GET",
        "/infra/security-product/get",
        Some("infra:security-product:query"),
    ),
    (
        "GET",
        "/infra/security-product/list",
        Some("infra:security-product:query"),
    ),
    (
        "GET",
        "/infra/security-product/page",
        Some("infra:security-product:query"),
    ),
    (
        "GET",
        "/infra/service-provider/get",
        Some("infra:service-provider:query"),
    ),
    (
        "GET",
        "/infra/service-provider/list",
        Some("infra:service-provider:query"),
    ),
    (
        "GET",
        "/infra/service-provider/page",
        Some("infra:service-provider:query"),
    ),
    ("GET", "/infra/task/get", Some("infra:task:query")),
    ("GET", "/infra/task/list", Some("infra:task:query")),
    ("GET", "/infra/task/page", Some("infra:task:query")),
    ("GET", "/upload/{*path}", Some("infra:file:query")),
    (
        "POST",
        "/infra/application-endpoint/create",
        Some("infra:application-endpoint:create"),
    ),
    (
        "POST",
        "/infra/approval-rule/create",
        Some("infra:approval-rule:create"),
    ),
    ("POST", "/infra/asset/create", Some("infra:asset:create")),
    (
        "POST",
        "/infra/asset/import-csv",
        Some("infra:asset:create"),
    ),
    (
        "POST",
        "/infra/asset/{id}/port/add",
        Some("infra:asset:update"),
    ),
    (
        "POST",
        "/infra/business-application/create",
        Some("infra:business-application:create"),
    ),
    (
        "POST",
        "/infra/cloud-asset/create",
        Some("infra:cloud-asset:create"),
    ),
    (
        "POST",
        "/infra/cloud-platform/create",
        Some("infra:cloud-platform:create"),
    ),
    (
        "POST",
        "/infra/cloud-provider-config/create",
        Some("infra:cloud-provider-config:create"),
    ),
    (
        "POST",
        "/infra/cloud-provider-config/test-connection",
        Some("infra:cloud-provider-config:update"),
    ),
    (
        "POST",
        "/infra/cloud-resource/create",
        Some("infra:cloud-resource:create"),
    ),
    (
        "POST",
        "/infra/cloud-zone/create",
        Some("infra:cloud-zone:create"),
    ),
    (
        "POST",
        "/infra/codegen/create-list",
        Some("infra:codegen:update"),
    ),
    ("POST", "/infra/config/create", Some("infra:config:create")),
    (
        "POST",
        "/infra/data-source-config/create",
        Some("infra:data-source-config:create"),
    ),
    (
        "POST",
        "/infra/file-config/create",
        Some("infra:file-config:create"),
    ),
    ("POST", "/infra/file/create", Some("infra:file:create")),
    ("POST", "/infra/file/upload", None),
    ("POST", "/infra/inspection/run", Some("infra:task:execute")),
    ("POST", "/infra/job/create", Some("infra:job:create")),
    ("POST", "/infra/job/sync", Some("infra:job:update")),
    (
        "POST",
        "/infra/machine-room/create",
        Some("infra:machine-room:create"),
    ),
    (
        "POST",
        "/infra/network-policy/create",
        Some("infra:network-policy:create"),
    ),
    (
        "POST",
        "/infra/network-policy/import-csv",
        Some("infra:network-policy:create"),
    ),
    (
        "POST",
        "/infra/network-policy/recheck-risks",
        Some("infra:network-policy:update"),
    ),
    (
        "POST",
        "/infra/network-zone/create",
        Some("infra:network-zone:create"),
    ),
    (
        "POST",
        "/infra/physical-resource/create",
        Some("infra:physical-resource:create"),
    ),
    (
        "POST",
        "/infra/resource-ticket/create",
        Some("infra:resource-ticket:create"),
    ),
    (
        "POST",
        "/infra/resource-ticket/{id}/approve",
        Some("infra:resource-ticket:approve"),
    ),
    (
        "POST",
        "/infra/resource-ticket/{id}/deliver",
        Some("infra:resource-ticket:deliver"),
    ),
    (
        "POST",
        "/infra/resource-ticket/{id}/provision",
        Some("infra:resource-ticket:provision"),
    ),
    (
        "POST",
        "/infra/security-product/create",
        Some("infra:security-product:create"),
    ),
    (
        "POST",
        "/infra/service-provider/create",
        Some("infra:service-provider:create"),
    ),
    ("POST", "/infra/task/create", Some("infra:task:create")),
    ("POST", "/infra/task/retry", Some("infra:task:execute")),
    (
        "POST",
        "/infra/task/trigger-scan",
        Some("infra:task:execute"),
    ),
    (
        "PUT",
        "/infra/api-error-log/update-status",
        Some("infra:api-error-log:update-status"),
    ),
    (
        "PUT",
        "/infra/application-endpoint/update",
        Some("infra:application-endpoint:update"),
    ),
    (
        "PUT",
        "/infra/approval-rule/update",
        Some("infra:approval-rule:update"),
    ),
    ("PUT", "/infra/asset/update", Some("infra:asset:update")),
    (
        "PUT",
        "/infra/asset/{id}/port/{port}",
        Some("infra:asset:update"),
    ),
    (
        "PUT",
        "/infra/business-application/update",
        Some("infra:business-application:update"),
    ),
    (
        "PUT",
        "/infra/cloud-asset/update",
        Some("infra:cloud-asset:update"),
    ),
    (
        "PUT",
        "/infra/cloud-platform/update",
        Some("infra:cloud-platform:update"),
    ),
    (
        "PUT",
        "/infra/cloud-provider-config/update",
        Some("infra:cloud-provider-config:update"),
    ),
    (
        "PUT",
        "/infra/cloud-resource/update",
        Some("infra:cloud-resource:update"),
    ),
    (
        "PUT",
        "/infra/cloud-zone/update",
        Some("infra:cloud-zone:update"),
    ),
    (
        "PUT",
        "/infra/codegen/sync-from-db",
        Some("infra:codegen:update"),
    ),
    ("PUT", "/infra/codegen/update", Some("infra:codegen:update")),
    ("PUT", "/infra/config/update", Some("infra:config:update")),
    (
        "PUT",
        "/infra/data-source-config/update",
        Some("infra:data-source-config:update"),
    ),
    (
        "PUT",
        "/infra/file-config/update",
        Some("infra:file-config:update"),
    ),
    (
        "PUT",
        "/infra/file-config/update-master",
        Some("infra:file-config:update"),
    ),
    (
        "PUT",
        "/infra/inspection/baseline",
        Some("infra:asset:update"),
    ),
    ("PUT", "/infra/job/trigger", Some("infra:job:trigger")),
    ("PUT", "/infra/job/update", Some("infra:job:update")),
    ("PUT", "/infra/job/update-status", Some("infra:job:update")),
    (
        "PUT",
        "/infra/machine-room/update",
        Some("infra:machine-room:update"),
    ),
    (
        "PUT",
        "/infra/network-policy/update",
        Some("infra:network-policy:update"),
    ),
    (
        "PUT",
        "/infra/network-zone/update",
        Some("infra:network-zone:update"),
    ),
    (
        "PUT",
        "/infra/physical-resource/update",
        Some("infra:physical-resource:update"),
    ),
    (
        "PUT",
        "/infra/resource-ticket/update",
        Some("infra:resource-ticket:update"),
    ),
    (
        "PUT",
        "/infra/risk/{id}/resolve",
        Some("infra:risk:resolve"),
    ),
    (
        "PUT",
        "/infra/risk/{id}/status/{status}",
        Some("infra:risk:update"),
    ),
    (
        "PUT",
        "/infra/security-product/update",
        Some("infra:security-product:update"),
    ),
    (
        "PUT",
        "/infra/service-provider/update",
        Some("infra:service-provider:update"),
    ),
    ("PUT", "/infra/task/cancel", Some("infra:task:execute")),
    ("PUT", "/infra/task/update", Some("infra:task:update")),
];

pub async fn authorize(
    method: Method,
    matched_path: Option<MatchedPath>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let user = request
        .extensions()
        .get::<CurrentUser>()
        .cloned()
        .ok_or_else(|| AppError::unauthorized("authentication required"))?;

    let pattern = matched_path
        .as_ref()
        .map(MatchedPath::as_str)
        .ok_or_else(|| AppError::forbidden("unregistered infra route"))?;

    let Some(required) = required_permission(method.as_str(), pattern) else {
        // Not in the registry: deny by default (migration plan §4.2).
        return Err(AppError::forbidden("unregistered infra route"));
    };
    let Some(required) = required else {
        // Registered as login-only.
        return Ok(next.run(request).await);
    };

    if user.role_codes.iter().any(|role| role == "super_admin") {
        return Ok(next.run(request).await);
    }
    let permission = Permission::new(required)
        .map_err(|_| AppError::internal("invalid infra permission policy"))?;
    if user.can(&permission) {
        Ok(next.run(request).await)
    } else {
        Err(AppError::forbidden("permission denied"))
    }
}

fn required_permission<'a>(method: &str, pattern: &str) -> Option<Option<&'a str>> {
    ROUTE_PERMISSIONS
        .binary_search_by(|(m, p, _)| (*m, *p).cmp(&(method, pattern)))
        .ok()
        .map(|index| ROUTE_PERMISSIONS[index].2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_every_entry_with_sorted_keys() {
        assert_eq!(ROUTE_PERMISSIONS.len(), 207);
        for window in ROUTE_PERMISSIONS.windows(2) {
            assert!(
                (window[0].0, window[0].1) < (window[1].0, window[1].1),
                "registry must stay sorted for binary search: {} {}",
                window[0].1,
                window[1].1
            );
        }
    }

    #[test]
    fn maps_routes_to_expected_permissions() {
        assert_eq!(
            required_permission("GET", "/infra/config/page"),
            Some(Some("infra:config:query"))
        );
        assert_eq!(
            required_permission("DELETE", "/infra/asset/{id}/port/{port}"),
            Some(Some("infra:asset:update"))
        );
        assert_eq!(
            required_permission("POST", "/infra/resource-ticket/{id}/approve"),
            Some(Some("infra:resource-ticket:approve"))
        );
        assert_eq!(
            required_permission("POST", "/infra/task/trigger-scan"),
            Some(Some("infra:task:execute"))
        );
        assert_eq!(
            required_permission("POST", "/infra/inspection/run"),
            Some(Some("infra:task:execute"))
        );
        assert_eq!(
            required_permission("GET", "/infra/inspection/results"),
            Some(Some("infra:task:query"))
        );
        assert_eq!(
            required_permission("PUT", "/infra/inspection/baseline"),
            Some(Some("infra:asset:update"))
        );
        assert_eq!(
            required_permission("GET", "/infra/network-policy/page"),
            Some(Some("infra:network-policy:query"))
        );
        assert_eq!(
            required_permission("GET", "/infra/capabilities"),
            Some(None)
        );
    }

    #[test]
    fn unknown_routes_are_denied() {
        assert_eq!(required_permission("GET", "/infra/not-a-thing"), None);
        assert_eq!(required_permission("POST", "/infra/config/page"), None);
    }
}
