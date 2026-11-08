-- Register the Kairos/Vben asset operations pages and the server-side
-- permissions enforced by rustset-infra-server. This migration only adds
-- menus and permissions; existing users, roles, and role assignments remain
-- unchanged. super_admin receives the entries through its existing semantics.

SELECT setval(
    'system_menu_seq',
    GREATEST(
        (SELECT COALESCE(MAX(id), 1) FROM system_menu),
        (SELECT last_value FROM system_menu_seq)
    ),
    true
);

INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), '资产运营', '', 1, 35, 0,
    '/asset-ops', 'lucide:boxes', '', 'AssetOps', 0, true, true, true,
    'migration', 'migration', 0
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu
    WHERE deleted = 0 AND path = '/asset-ops' AND parent_id = 0
);

WITH root AS (
    SELECT id
    FROM system_menu
    WHERE deleted = 0 AND path = '/asset-ops' AND parent_id = 0
    ORDER BY id
    LIMIT 1
), page_defs(name, path, icon, component, component_name, sort) AS (
    VALUES
        ('资产管理', 'assets', 'lucide:hard-drive', 'asset-ops/asset/index', 'AssetOpsAsset', 10),
        ('服务商', 'service-provider', 'lucide:briefcase-business', 'asset-ops/service-provider/index', 'AssetOpsServiceProvider', 20),
        ('物理机房', 'machine-room', 'lucide:warehouse', 'asset-ops/machine-room/index', 'AssetOpsMachineRoom', 30),
        ('云资源区', 'cloud-zone', 'lucide:map-pin', 'asset-ops/cloud-platform/zone', 'AssetOpsCloudZone', 40),
        ('云平台', 'cloud-platform', 'lucide:cloud-cog', 'asset-ops/cloud-platform/platform', 'AssetOpsCloudPlatform', 50),
        ('云厂商对接', 'cloud-provider-config', 'lucide:plug-zap', 'asset-ops/cloud-provider-config/index', 'AssetOpsCloudProviderConfig', 60),
        ('网络区域', 'zone', 'lucide:globe', 'asset-ops/zone/index', 'AssetOpsNetworkZone', 70),
        ('安全产品', 'security-product', 'lucide:shield', 'asset-ops/security-product/index', 'AssetOpsSecurityProduct', 80),
        ('业务应用', 'business-application', 'lucide:layout-grid', 'asset-ops/business-application/index', 'AssetOpsBusinessApplication', 90),
        ('业务资源', 'business-resource', 'lucide:package', 'asset-ops/business-resource/index', 'AssetOpsBusinessResource', 100),
        ('资源工单', 'resource-ticket', 'lucide:ticket', 'asset-ops/resource-ticket/index', 'AssetOpsResourceTicket', 110),
        ('扫描任务', 'task', 'lucide:list-checks', 'asset-ops/task/index', 'AssetOpsTask', 120),
        ('风险管理', 'risk', 'lucide:triangle-alert', 'asset-ops/risk/index', 'AssetOpsRisk', 130)
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), page.name, '', 2, page.sort, root.id,
    page.path, page.icon, page.component, page.component_name,
    0, true, true, true, 'migration', 'migration', 0
FROM page_defs page
CROSS JOIN root
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0 AND existing.component = page.component
);

WITH permission_defs(component, resource, action, label, sort) AS (
    VALUES
        ('asset-ops/asset/index', 'asset', 'query', '资产查询', 1),
        ('asset-ops/asset/index', 'asset', 'create', '资产创建', 2),
        ('asset-ops/asset/index', 'asset', 'update', '资产更新', 3),
        ('asset-ops/asset/index', 'asset', 'delete', '资产删除', 4),
        ('asset-ops/service-provider/index', 'service-provider', 'query', '服务商查询', 1),
        ('asset-ops/service-provider/index', 'service-provider', 'create', '服务商创建', 2),
        ('asset-ops/service-provider/index', 'service-provider', 'update', '服务商更新', 3),
        ('asset-ops/service-provider/index', 'service-provider', 'delete', '服务商删除', 4),
        ('asset-ops/machine-room/index', 'machine-room', 'query', '机房查询', 1),
        ('asset-ops/machine-room/index', 'machine-room', 'create', '机房创建', 2),
        ('asset-ops/machine-room/index', 'machine-room', 'update', '机房更新', 3),
        ('asset-ops/machine-room/index', 'machine-room', 'delete', '机房删除', 4),
        ('asset-ops/cloud-platform/zone', 'cloud-zone', 'query', '云资源区查询', 1),
        ('asset-ops/cloud-platform/zone', 'cloud-zone', 'create', '云资源区创建', 2),
        ('asset-ops/cloud-platform/zone', 'cloud-zone', 'update', '云资源区更新', 3),
        ('asset-ops/cloud-platform/zone', 'cloud-zone', 'delete', '云资源区删除', 4),
        ('asset-ops/cloud-platform/platform', 'cloud-platform', 'query', '云平台查询', 1),
        ('asset-ops/cloud-platform/platform', 'cloud-platform', 'create', '云平台创建', 2),
        ('asset-ops/cloud-platform/platform', 'cloud-platform', 'update', '云平台更新', 3),
        ('asset-ops/cloud-platform/platform', 'cloud-platform', 'delete', '云平台删除', 4),
        ('asset-ops/cloud-provider-config/index', 'cloud-provider-config', 'query', '云对接查询', 1),
        ('asset-ops/cloud-provider-config/index', 'cloud-provider-config', 'create', '云对接创建', 2),
        ('asset-ops/cloud-provider-config/index', 'cloud-provider-config', 'update', '云对接更新', 3),
        ('asset-ops/cloud-provider-config/index', 'cloud-provider-config', 'delete', '云对接删除', 4),
        ('asset-ops/zone/index', 'network-zone', 'query', '网络区域查询', 1),
        ('asset-ops/zone/index', 'network-zone', 'create', '网络区域创建', 2),
        ('asset-ops/zone/index', 'network-zone', 'update', '网络区域更新', 3),
        ('asset-ops/zone/index', 'network-zone', 'delete', '网络区域删除', 4),
        ('asset-ops/security-product/index', 'security-product', 'query', '安全产品查询', 1),
        ('asset-ops/security-product/index', 'security-product', 'create', '安全产品创建', 2),
        ('asset-ops/security-product/index', 'security-product', 'update', '安全产品更新', 3),
        ('asset-ops/security-product/index', 'security-product', 'delete', '安全产品删除', 4),
        ('asset-ops/business-application/index', 'business-application', 'query', '业务应用查询', 1),
        ('asset-ops/business-application/index', 'business-application', 'create', '业务应用创建', 2),
        ('asset-ops/business-application/index', 'business-application', 'update', '业务应用更新', 3),
        ('asset-ops/business-application/index', 'business-application', 'delete', '业务应用删除', 4),
        ('asset-ops/business-resource/index', 'business-resource', 'query', '业务资源查询', 1),
        ('asset-ops/business-resource/index', 'business-resource', 'create', '业务资源创建', 2),
        ('asset-ops/business-resource/index', 'business-resource', 'update', '业务资源更新', 3),
        ('asset-ops/business-resource/index', 'business-resource', 'delete', '业务资源删除', 4),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'query', '资源工单查询', 1),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'create', '资源工单创建', 2),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'update', '资源工单更新', 3),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'delete', '资源工单删除', 4),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'approve', '资源工单审批', 5),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'provision', '资源工单配置', 6),
        ('asset-ops/resource-ticket/index', 'resource-ticket', 'deliver', '资源工单交付', 7),
        ('asset-ops/task/index', 'task', 'query', '任务查询', 1),
        ('asset-ops/task/index', 'task', 'create', '任务创建', 2),
        ('asset-ops/task/index', 'task', 'update', '任务更新', 3),
        ('asset-ops/task/index', 'task', 'delete', '任务删除', 4),
        ('asset-ops/task/index', 'task', 'execute', '任务执行', 5),
        ('asset-ops/risk/index', 'risk', 'query', '风险查询', 1),
        ('asset-ops/risk/index', 'risk', 'update', '风险更新', 2),
        ('asset-ops/risk/index', 'risk', 'resolve', '风险处置', 3)
)
INSERT INTO system_menu (
    id, name, permission, type, sort, parent_id, path, icon, component,
    component_name, status, visible, keep_alive, always_show,
    creator, updater, deleted
)
SELECT
    nextval('system_menu_seq'), permission.label,
    'infra:' || permission.resource || ':' || permission.action,
    3, permission.sort, page.id, '', '', '', '', 0, true, true, true,
    'migration', 'migration', 0
FROM permission_defs permission
JOIN system_menu page
  ON page.component = permission.component AND page.deleted = 0
WHERE NOT EXISTS (
    SELECT 1 FROM system_menu existing
    WHERE existing.deleted = 0
      AND existing.permission = 'infra:' || permission.resource || ':' || permission.action
);
