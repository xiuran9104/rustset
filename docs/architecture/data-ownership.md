# Data ownership boundary

The authenticated JWT tenant is the only trusted tenant source. Clients cannot select another tenant in request data.

Tenant-owned data includes CMDB instances, relations and network zones; assets, policies, risks, inspections, tasks and tickets; providers, rooms, cloud inventory, security products, applications, approval rules and network zones. Historical rows with `tenant_id IS NULL` remain quarantined until ownership is assigned.

CMDB models, attributes and triggers are global shared schemas. Platform configuration, storage configuration, scheduler definitions, code-generation metadata, operational logs and the high-risk-port catalog are global administrator resources.

Tenant repositories derive `TenantContext` from `CurrentUser` and apply it consistently to list, detail, create, update, delete, import and export. Cross-resource ownership uses composite `(tenant_id, id)` references where applicable.
