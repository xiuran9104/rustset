-- 补齐流程定义的 update/delete 按钮权限（0062 只建了 query/create，
-- 前端编辑/删除按钮因权限码缺失不渲染）。
-- 注意 30370 已被 0060 MCP 菜单占用，delete 用 30381。

LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30369, '定义更新', 'infra:flow-definition:update', 3, 3, 30362, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30369 AND deleted = 0);

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30381, '定义删除', 'infra:flow-definition:delete', 3, 4, 30362, '', '', '', NULL,
       0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30381 AND deleted = 0);

-- 超管补授权
INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, m.menu_id, 'system', 'system', 0, 1
FROM public.system_role r
CROSS JOIN (VALUES (30369), (30381)) AS m(menu_id)
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = m.menu_id AND rm.deleted = 0
  );
