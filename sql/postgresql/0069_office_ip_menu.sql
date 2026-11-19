-- 办公网IP申报独立页（普通员工自服务入口）：运营流程下新增菜单，
-- 页面提供申报表单与全生命周期页签（全部/待我审批/待配置/待交付/已完成），
-- 与资源工单页同构。发起动作复用既有 infra:flow-instance:start 按钮权限。

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30385, '办公网IP申报', '', 2, 0, 30344, 'office-ip', 'lucide:network',
       'asset-ops/office-ip/index', 'OfficeIpApply',
       0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (
    SELECT 1 FROM public.system_menu WHERE id = 30385 AND deleted = 0
);

INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, 30385, 'system', 'system', 0, 1
FROM public.system_role r
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu
      WHERE role_id = r.id AND menu_id = 30385 AND deleted = 0
  );
