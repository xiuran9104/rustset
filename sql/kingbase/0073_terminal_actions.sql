-- 终端设备操作列对齐资产页三件套（工单/编辑/删除）：
-- 工单=终端变更申请（流程），编辑=低风险字段直改，删除=误入账清理。
-- 补编辑/删除按钮权限并授超管。

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT m.id, m.name, m.permission, 3, m.sort, 30386, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
FROM (VALUES
    (30388, '终端编辑', 'infra:terminal:update', 1),
    (30389, '终端删除', 'infra:terminal:delete', 2)
) AS m(id, name, permission, sort)
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = m.id AND deleted = 0);

INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30388),(30389)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu
      WHERE role_id = r.id AND menu_id = m.menu_id AND deleted = 0
  );
