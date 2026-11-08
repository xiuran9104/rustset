import type { App } from 'vue';

/**
 * form-create 插件的懒加载门面。
 *
 * setup.ts 静态引入了全量 ant-design-vue、@form-create/antd-designer 与
 * tinymce/上传等自定义组件——过去它在 bootstrap 顶层被 import，导致登录页
 * 也要一次性拉取整个组件库（实测 226 个 JS chunk / ~4.6MB）。冷加载时这些
 * chunk 由模块预加载器并行扇出，多标签同时冷启动会触发
 * net::ERR_INSUFFICIENT_RESOURCES，模块 import 永远挂起，渲染进程表现为
 * “主线程空闲却无响应”（无头 Chromium 卡死缺陷的直接根因）。
 *
 * 因此：登录页绝不加载本模块；登录成功后预取（store/auth.ts），路由守卫
 * 在首次进入受控页面前 await（router/guard.ts），保证 a-* 全局组件在业务
 * 页面渲染前完成注册。
 */

let appInstance: App | null = null;
let setupPromise: Promise<void> | null = null;

/** bootstrap 中记录 app 实例，供延迟安装使用 */
export function setFormCreateApp(app: App) {
  appInstance = app;
}

/** 幂等加载并安装 form-create + 全量 antd；多次调用共享同一次加载 */
export function ensureFormCreate(): Promise<void> {
  setupPromise ??= (async () => {
    const { setupFormCreate } = await import('./setup');
    if (!appInstance) {
      throw new Error('ensureFormCreate called before setFormCreateApp');
    }
    setupFormCreate(appInstance);
  })();
  return setupPromise;
}
