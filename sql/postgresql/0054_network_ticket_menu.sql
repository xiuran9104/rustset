-- 运营流程新增"网络策略工单"独立入口：网络策略申请与资源工单分池，
-- 复用同一套工单引擎与 infra:resource-ticket:* 权限码。
LOCK TABLE public.system_menu IN SHARE ROW EXCLUSIVE MODE;

INSERT INTO public.system_menu
    (id, name, permission, type, sort, parent_id, path, icon, component, component_name,
     status, visible, keep_alive, always_show, creator, updater)
SELECT 30360, '网络策略工单', '', 2, 3, 30344, 'network-ticket', 'lucide:shield',
       'asset-ops/network-ticket/index', 'NetworkPolicyTicket', 0, true, true, true, 'system', 'system'
WHERE NOT EXISTS (SELECT 1 FROM public.system_menu WHERE id = 30360 AND deleted = 0);

-- 按钮权限沿用 infra:resource-ticket:*（同一引擎同一批接口），无需新增权限码。
-- 超级管理员补齐新菜单授权，满足 asset-ops 全量授权断言。
INSERT INTO public.system_role_menu (id, role_id, menu_id, creator, updater, deleted, tenant_id)
SELECT nextval('public.system_role_menu_seq'), r.id, 30360, 'system', 'system', 0, 1
FROM public.system_role r
WHERE r.deleted = 0 AND r.status = 0 AND r.code = 'super_admin'
  AND NOT EXISTS (
      SELECT 1 FROM public.system_role_menu rm
      WHERE rm.role_id = r.id AND rm.menu_id = 30360 AND rm.deleted = 0
  );
