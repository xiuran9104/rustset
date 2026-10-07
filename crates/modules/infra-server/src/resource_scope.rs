#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResourceScope {
    Global,
    Tenant,
}

pub(crate) const GLOBAL_TABLES: &[&str] = &[
    "infra_api_access_log",
    "infra_api_error_log",
    "infra_codegen_column",
    "infra_codegen_table",
    "infra_config",
    "infra_data_source_config",
    "infra_file",
    "infra_file_config",
    "infra_high_risk_port_rule",
    "infra_job",
    "infra_job_log",
];
pub(crate) const TENANT_TABLES: &[&str] = &[
    "infra_application_endpoint",
    "infra_approval_rule",
    "infra_asset",
    "infra_business_application",
    "infra_cloud_asset",
    "infra_cloud_platform",
    "infra_cloud_provider_config",
    "infra_cloud_resource",
    "infra_cloud_zone",
    "infra_inspection_baseline",
    "infra_inspection_result",
    "infra_machine_room",
    "infra_network_policy",
    "infra_network_zone",
    "infra_physical_resource",
    "infra_resource_ticket",
    "infra_risk",
    "infra_security_product",
    "infra_service_provider",
    "infra_task",
];
pub(crate) fn scope_for(table: &str) -> Option<ResourceScope> {
    GLOBAL_TABLES
        .contains(&table)
        .then_some(ResourceScope::Global)
        .or_else(|| {
            TENANT_TABLES
                .contains(&table)
                .then_some(ResourceScope::Tenant)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn registry_is_sorted_unique_and_disjoint() {
        for tables in [GLOBAL_TABLES, TENANT_TABLES] {
            assert!(tables.windows(2).all(|pair| pair[0] < pair[1]));
            assert_eq!(tables.len(), tables.iter().collect::<BTreeSet<_>>().len());
        }
        assert!(
            GLOBAL_TABLES
                .iter()
                .all(|table| !TENANT_TABLES.contains(table))
        );
        assert_eq!(scope_for("infra_asset"), Some(ResourceScope::Tenant));
        assert_eq!(scope_for("infra_config"), Some(ResourceScope::Global));
    }
}
