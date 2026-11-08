-- 运营流程新增"自动化配置"菜单
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30361, '自动化配置', '', 2, 4, 30344, 'automation-config', 'lucide:settings',
       'asset-ops/automation-config/index', 'AutomationConfig', 0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30361 AND deleted = 0);

-- 超管补授权
INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, 30361, 'system', 'system', 0, 1
FROM public.system_role r
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = 30361 AND rm.deleted = 0
  );
