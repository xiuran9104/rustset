# 浏览器与办公软件兼容声明

依据：docs/xinchuang-adaptation-plan.md §4.5。前端为 Vue 3 + Vben（Ant Design Vue）纯静态产物（vite target `es2015`），无 ActiveX / 浏览器插件依赖；能力判断不依赖 UA 嗅探。基线冻结于 `apps/web/.browserslistrc`（Chromium 内核 ≥ 88、Firefox ESR），基线变更需按 §4.5.1 重新认证。

## 支持矩阵

| 浏览器 / 办公软件 | 内核 / 版本基线 | 状态 |
| --- | --- | --- |
| 银河麒麟自带浏览器 | Chromium 内核 ≥ 88 | 支持（待真机回归，§6.1 第三层） |
| 统信 UOS 浏览器 | Chromium 内核 ≥ 88 | 支持（待真机回归） |
| 360 安全浏览器 / 奇安信浏览器 | Chromium 内核 ≥ 88 | 支持（待真机回归） |
| Firefox | ESR（≥ 91） | 支持 |
| Chrome / Edge | ≥ 88 | 支持 |
| 国密浏览器（密信/奇安信等 TLCP 场景） | Chromium 内核 + 国密 TLS 模块 | 随 §4.4.3 #1/#7 一并验证 |
| WPS Office | 打开导出的 Excel/CSV/压缩包 | 支持（calamine/zip 导入导出路径，待专项验证 §4.5.4） |

## 已发现缺陷（2026-11-08，容器 Chromium 124 实测）

**D-1：登录表单在 Chromium 124 不渲染**——zod v4（catalog ^4.6.7，当前最新）的
`_zod` 惰性代理模式在 Chromium 124 触发 Proxy 不变量 TypeError，登录表单组件
挂载失败（页面文本渲染、输入框数量 0）。dev/生产构建同复现 → 引擎版本相关。
证据：`docs/evidence/browser-chromium124/`。影响：browserslist 声明下限
（Chromium ≥ 88）与实际可行下限不一致；处置：目标环境实测真实下限 +
跟踪 zod 上游修复。其余页面待登录打通后逐一验证。

**D-3：无头浏览器"卡死"（2026-11-09 定位，测试容器缺陷 + 应用侧放大因素）**
——症状：chrome-interop 容器（zenika/alpine-chrome，Chromium 124 headless +
swiftshader 软渲染）内停留/新开标签数分钟后渲染进程"假死"：主线程空闲但
Runtime.evaluate 超时，新开同站标签同样卡死，仅重启容器可复位。定位过程
（CDP tracing + 网络域探针）：
- 直接诱因：登录页冷加载一次性并行扇出 200+ JS/CSS chunk 请求（实测 239 个
  资源 / 4.6MB），部分请求 `net::ERR_INSUFFICIENT_RESOURCES`、其余永久
  pending，模块 import 挂起 → 渲染进程阻塞。
- 容器侧根因：Docker 默认 `--shm-size=64m`，空闲时已用 51%；Chromium 网络
  缓冲/mojo IPC 走 /dev/shm，多标签并发冷加载即耗尽。容器重建为
  `--shm-size=1g` 后同一复现脚本 8/8 通过（此前 4 次中 3 次卡死）。
- 应用侧修复（已提交）：`plugins/form-create`（全量 antd + FcDesigner +
  tinymce/上传组件）过去在 bootstrap 顶层静态引入，登录页也拉整个组件库。
  现拆分为懒加载 `ensureFormCreate()`：登录成功后预取、路由守卫首次进入
  受控页面前 await。登录页 239 资源 / 4.6MB → 158 资源 / 2.26MB。
- 残余：偶发 Compositor `trap int3` 崩溃（dmesg）为该镜像 headless 软渲染
  的老毛病，随标签数/负载加重；测试回合间重启容器即可规避。真实浏览器
  （硬渲染、正常 shm）未复现任何卡死。
- 回归结论：错误密码提示 / 登录 / 工作台 / vxe 表格页 / 表单设计器均正常，
  无组件未解析警告（a-* 全局组件由懒加载安装覆盖）。

## 已验证项（随合同测试与端到端用例持续更新）

- 登录、菜单导航、表格、表单、文件上传下载：全部核心用例基于标准 Web API。
- 流式响应（SSE）：要求网关关闭代理缓冲（部署模板已按此配置 `proxy_buffering off`）。
- 大文件上传：浏览器直传 RustFS 走分片 S3 协议，不依赖插件。

## 验收要求（交付前必须完成并留存证据）

1. 每个支持浏览器以实际版本执行核心用例截图回归（登录/菜单/表格/图表/表单/打印/上传下载/流式响应）。
2. WPS 打开导出的 Excel/CSV/压缩包，验证中文、日期、数字、公式、样式与大文件名。
3. 国密 HTTPS 场景与国密浏览器互操作（证书、混合内容、跨域、流式）。
4. 所有用例留存版本号、截图/日志与结论，作为验收证据（§4.5.5）。
