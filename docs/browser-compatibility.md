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

## 已验证项（随合同测试与端到端用例持续更新）

- 登录、菜单导航、表格、表单、文件上传下载：全部核心用例基于标准 Web API。
- 流式响应（SSE）：要求网关关闭代理缓冲（部署模板已按此配置 `proxy_buffering off`）。
- 大文件上传：浏览器直传 RustFS 走分片 S3 协议，不依赖插件。

## 验收要求（交付前必须完成并留存证据）

1. 每个支持浏览器以实际版本执行核心用例截图回归（登录/菜单/表格/图表/表单/打印/上传下载/流式响应）。
2. WPS 打开导出的 Excel/CSV/压缩包，验证中文、日期、数字、公式、样式与大文件名。
3. 国密 HTTPS 场景与国密浏览器互操作（证书、混合内容、跨域、流式）。
4. 所有用例留存版本号、截图/日志与结论，作为验收证据（§4.5.5）。
