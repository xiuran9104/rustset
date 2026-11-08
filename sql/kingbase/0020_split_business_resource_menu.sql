-- 0020: split the mixed 业务资源 ledger into two focused pages under 业务管理:
-- 云资源 (cloud servers) keeps menu 30252 with its permission buttons, and a
-- new 物理资源 (physical servers) menu is added beside it. Both pages share
-- the infra:business-resource:* permission codes.

UPDATE system_menu
SET name = '云资源',
    path = 'cloud-resource',
    icon = 'lucide:cloud',
    component = 'asset-ops/business-resource/cloud/index',
    component_name = 'AssetOpsCloudResource',
    updater = 'migration',
    update_time = now()
WHERE id = 30252
  AND deleted = 0
  AND component = 'asset-ops/business-resource/index';

INSERT INTO system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component,
     component_name, status, visible, keep_alive, always_show,
     creator, create_time, updater, update_time, deleted)
SELECT 30345, '物理资源', '', 2, 3, 30343, 'physical-resource',
       'lucide:hard-drive', 'asset-ops/business-resource/physical/index',
       'AssetOpsPhysicalResource', 0, true, true, true,
       'migration', now(), 'migration', now(), 0
WHERE NOT EXISTS (SELECT 1 FROM system_menu WHERE id = 30345);

-- Roles that could see the old mixed page must see the new physical page too.
INSERT INTO system_role_menu
    (id, role_id, menu_id, creator, create_time, updater, update_time, deleted, tenant_id)
SELECT nextval('system_role_menu_seq'), grant_.role_id, 30345,
       'migration', now(), 'migration', now(), 0, grant_.tenant_id
FROM system_role_menu grant_
WHERE grant_.menu_id = 30252
  AND grant_.deleted = 0
  AND NOT EXISTS (
      SELECT 1 FROM system_role_menu
      WHERE role_id = grant_.role_id AND menu_id = 30345 AND deleted = 0
  );
