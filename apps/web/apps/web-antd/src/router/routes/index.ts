import type { RouteRecordRaw } from 'vue-router';

import { traverseTreeValues } from '@vben/utils';

import { coreRoutes, fallbackNotFoundRoute } from './core';

// 有需要可以自行打开注释，并创建文件夹
// const externalRouteFiles = import.meta.glob('./external/**/*.ts', { eager: true });
// const staticRouteFiles = import.meta.glob('./static/**/*.ts', { eager: true });

/** 外部路由列表，访问这些页面可以不需要Layout，可能用于内嵌在别的系统(不会显示在菜单中) */
// const externalRoutes: RouteRecordRaw[] = mergeRouteModules(externalRouteFiles);
// const staticRoutes: RouteRecordRaw[] = mergeRouteModules(staticRouteFiles);
// Keep historical Dioxus bookmarks working. These routes still pass through
// the login guard, and their dynamic targets remain permission-controlled.
const legacyAssetRedirects: RouteRecordRaw[] = [
  { path: '/asset', redirect: '/asset-center/assets', name: 'LegacyAssetRoot' },
  { path: '/asset/list', redirect: '/asset-center/assets', name: 'LegacyAssetList' },
  { path: '/asset/provider', redirect: '/infra-center/service-provider', name: 'LegacyAssetProvider' },
  { path: '/provider', redirect: '/infra-center/service-provider', name: 'LegacyProvider' },
  { path: '/asset/room', redirect: '/infra-center/machine-room', name: 'LegacyAssetRoom' },
  { path: '/room', redirect: '/infra-center/machine-room', name: 'LegacyRoom' },
  { path: '/asset/cloud', redirect: '/infra-center/cloud-platform', name: 'LegacyAssetCloud' },
  { path: '/cloud', redirect: '/infra-center/cloud-platform', name: 'LegacyCloud' },
  { path: '/asset/zone', redirect: '/infra-center/zone', name: 'LegacyAssetZone' },
  { path: '/zone', redirect: '/infra-center/zone', name: 'LegacyZone' },
  { path: '/asset/security', redirect: '/infra-center/security-product', name: 'LegacyAssetSecurity' },
  { path: '/security', redirect: '/infra-center/security-product', name: 'LegacySecurity' },
  { path: '/asset/business-app', redirect: '/biz-center/business-application', name: 'LegacyAssetBusinessApp' },
  { path: '/business', redirect: '/biz-center/business-application', name: 'LegacyBusiness' },
  { path: '/asset/business-resource', redirect: '/asset-center/cloud-resource', name: 'LegacyAssetBusinessResource' },
  // 0020 拆分前存在过的旧地址，兼容历史标签页/收藏。
  { path: '/biz-center/business-resource', redirect: '/asset-center/cloud-resource', name: 'LegacyBizBusinessResource' },
  { path: '/asset/ticket', redirect: '/ops-center/resource-ticket', name: 'LegacyAssetTicket' },
  { path: '/ticket', redirect: '/ops-center/resource-ticket', name: 'LegacyTicket' },
  { path: '/asset/task', redirect: '/asset-center/task', name: 'LegacyAssetTask' },
  { path: '/task', redirect: '/asset-center/task', name: 'LegacyTask' },
  { path: '/asset/risk', redirect: '/asset-center/risk', name: 'LegacyAssetRisk' },
  { path: '/risk', redirect: '/asset-center/risk', name: 'LegacyRisk' },
  // 0024 将 API 文档页从 swagger 更名为 api-docs，兼容历史标签页/收藏。
  { path: '/infra/swagger', redirect: '/infra/api-docs', name: 'LegacyInfraSwagger' },
];
const staticRoutes: RouteRecordRaw[] = legacyAssetRedirects;
const externalRoutes: RouteRecordRaw[] = [];

/** 路由列表，由基本路由、外部路由、静态重定向和404兜底路由组成
 *  无需走权限验证（会一直显示在菜单中） */
const routes: RouteRecordRaw[] = [
  ...coreRoutes,
  ...externalRoutes,
  // 纯重定向的旧地址与 system_menu 路径零冲突；后端路由生成器不合并
  // 本地路由，必须在创建 router 时直接注册，否则历史地址会 404。
  ...staticRoutes,
  fallbackNotFoundRoute,
];

/** 基本路由列表，这些路由不需要进入权限拦截 */
const coreRouteNames = traverseTreeValues(coreRoutes, (route) => route.name);

/**
 * 业务路由以 system_menu 为唯一来源。本地只保留核心、静态和外部路由，
 * 避免与后端菜单生成同名或同路径的第二套路由。
 */
const accessRoutes = [...staticRoutes];

// 源自 vue-vben-admin playground：https://github.com/vbenjs/vue-vben-admin/blob/main/playground/src/router/routes/index.ts#L38-L45
const componentKeys: string[] = Object.keys(
  import.meta.glob('../../views/**/*.vue'),
)
  .filter((item) => !item.includes('/modules/'))
  .map((v) => {
    const path = v.replace('../../views/', '/');
    return path.endsWith('.vue') ? path.slice(0, -4) : path;
  });
export { accessRoutes, componentKeys, coreRouteNames, routes };
