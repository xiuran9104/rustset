import type { RouteRecordRaw, Router } from 'vue-router';

import { LOGIN_PATH } from '@vben/constants';
import { $t } from '@vben/locales';
import { preferences } from '@vben/preferences';
import { useAccessStore, useDictStore, useUserStore } from '@vben/stores';
import { startProgress, stopProgress } from '@vben/utils';

import { message } from 'ant-design-vue';

import { getSimpleDictDataList } from '#/api/system/dict/data';
import { accessRoutes, coreRouteNames } from '#/router/routes';
import { ensureFormCreate } from '#/plugins/form-create';
import { useAuthStore } from '#/store';

import { generateAccess } from './access';

/**
 * 通用守卫配置
 * @param router
 */
function setupCommonGuard(router: Router) {
  // 记录已经加载的页面
  const loadedPaths = new Set<string>();

  router.beforeEach((to) => {
    to.meta.loaded = loadedPaths.has(to.path);

    // 页面加载进度条
    if (!to.meta.loaded && preferences.transition.progress) {
      startProgress();
    }
    return true;
  });

  router.afterEach((to) => {
    // 记录页面是否加载,如果已经加载，后续的页面切换动画等效果不在重复执行

    loadedPaths.add(to.path);

    // 关闭页面加载进度条
    if (preferences.transition.progress) {
      stopProgress();
    }
  });
}

/**
 * 权限访问守卫配置
 * @param router
 */
function setupAccessGuard(router: Router) {
  router.beforeEach(async (to, from) => {
    const accessStore = useAccessStore();
    const userStore = useUserStore();
    const authStore = useAuthStore();
    const dictStore = useDictStore();

    // 基本路由，这些路由不需要进入权限拦截
    if (coreRouteNames.includes(to.name as string)) {
      if (to.path === LOGIN_PATH && accessStore.accessToken) {
        return decodeRedirectPath(
          (to.query?.redirect as string) ||
            userStore.userInfo?.homePath ||
            preferences.app.defaultHomePath,
        );
      }
      return true;
    }

    // accessToken 检查
    if (!accessStore.accessToken) {
      // 明确声明忽略权限访问权限，则可以访问
      if (to.meta.ignoreAccess) {
        return true;
      }

      // 没有访问权限，跳转登录页面
      if (to.fullPath !== LOGIN_PATH) {
        return {
          path: LOGIN_PATH,
          // 如不需要，直接删除 query
          query:
            to.fullPath === preferences.app.defaultHomePath
              ? {}
              : { redirect: to.fullPath },
          // 携带当前跳转的页面，登录后重新跳转该页面
          replace: true,
        };
      }
      return to;
    }

    // 是否已经生成过动态路由
    if (accessStore.isAccessChecked) {
      if (areAccessRoutesRegistered(accessStore.accessRoutes, router)) {
        // 受控页面大量使用全局注册的 a-* 标签（全量 antd 由 form-create
        // 插件安装）；首次进入前确保注册完成，后续调用为已缓存 promise。
        await ensureFormCreate();
        return true;
      }
      accessStore.setIsAccessChecked(false);
    }
    // 加载字典数据（不阻塞加载）
    dictStore.setDictCacheByApi(getSimpleDictDataList);

    // 生成路由表
    // 当前登录用户拥有的角色标识列表
    let userInfo = userStore.userInfo;
    if (!userInfo || accessStore.backendAccessMenus.length === 0) {
      // fetchUserInfo 已统一加载用户与权限信息，再按权限生成动态菜单。
      const loading = message.loading({
        content: `${$t('common.loadingMenu')}...`,
      });
      try {
        const authPermissionInfo = await authStore.fetchUserInfo();
        if (authPermissionInfo) {
          userInfo = authPermissionInfo.user;
        }
      } finally {
        loading();
      }
    }
    const userRoles = userStore.userRoles ?? [];

    // 生成菜单和路由
    const { accessibleMenus, accessibleRoutes } = await generateAccess({
      roles: userRoles,
      router,
      // 则会在菜单中显示，但是访问会被重定向到403
      routes: accessRoutes,
    });

    // 保存菜单信息和路由信息
    accessStore.setAccessMenus(accessibleMenus);
    accessStore.setAccessRoutes(accessibleRoutes);
    accessStore.setIsAccessChecked(true);
    userStore.setUserRoles(userRoles);
    const redirectPath = (from.query.redirect ??
      (to.path === preferences.app.defaultHomePath
        ? userInfo?.homePath || preferences.app.defaultHomePath
        : to.fullPath)) as string;

    return {
      ...router.resolve(decodeRedirectPath(redirectPath)),
      replace: true,
    };
  });
}

function areAccessRoutesRegistered(
  routes: RouteRecordRaw[],
  router: Router,
): boolean {
  return routes.every((route) => {
    const currentRegistered = !route.name || router.hasRoute(route.name);
    return (
      currentRegistered &&
      areAccessRoutesRegistered(route.children ?? [], router)
    );
  });
}

/**
 * Vue Router already encodes query values. Decode legacy redirects at most
 * twice so URLs stored by the previous double-encoding behavior still work.
 */
function decodeRedirectPath(path: string): string {
  let decoded = path;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      const next = decodeURIComponent(decoded);
      if (next === decoded) break;
      decoded = next;
    } catch {
      break;
    }
  }
  return decoded;
}

/**
 * 项目守卫配置
 * @param router
 */
function createRouterGuard(router: Router) {
  /** 通用 */
  setupCommonGuard(router);
  /** 权限访问 */
  setupAccessGuard(router);
}

export { createRouterGuard };
