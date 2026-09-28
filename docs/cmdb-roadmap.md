# RustSet CMDB 与云平台适配路线图

日期：2026-09-25。参考：[veops/cmdb](https://github.com/veops/cmdb)（自定义模型/属性/实例/关系/多视图/权限）。

本文中的 CMDB“模型”用于定义配置项字段和关系，不是 AI 推理模型。AI 供应商、API Key 和模型参数统一在“AI 大模型 → 模型管理”中维护。

## 已落地（本轮，迁移 0009）

- **模型（cmdb_model）**：名称/编码/唯一键/图标/排序；删除前校验实例清空。
- **属性（cmdb_attribute）**：12 种类型（text/textarea/number/float/bool/date/datetime/select/multi_select/link/json/password）、必填、选项、默认值、列表显示；同模型内编码唯一。
- **实例（cmdb_instance）**：JSONB 动态载荷 + GIN 索引；写入按模型定义严格校验（未知属性拒绝、类型/必填/选项校验、默认值填充）；模型唯一键查重；分页 + 关键词（attributes::text ILIKE）。
- **关系（cmdb_relation）**：实例间有向关系绑定/解绑/双向查询；实例删除时级联软删关系。
- **权限**：`cmdb:model:*`、`cmdb:attribute:*`、`cmdb:instance:*`，经 CurrentUser 提取器 + require 强制（与 system 模块同模式），菜单"配置管理"下挂模型管理/实例管理。
- **前端**：模型管理页（含属性编辑器抽屉）、实例管理页（动态表格 + 动态表单）。

## 写入一致性（2026-09-26）

- CMDB 实例创建、更新和 Excel 导入共用 `instance_service`；HTTP 层负责权限和类型化请求，服务层负责校验及事务。导入仍按行提交并返回失败明细。
- 实例创建、更新先锁定所属模型；更新再锁定实例，读取最新属性后合并补丁。模型唯一键变更使用同一模型锁，并检查既有数据是否重复。数字等 JSON 值也参与唯一键检查。
- Infra 工单回写遵循相同的模型锁协议；模型创建与工单自动建模按模型编码获取事务 advisory lock，避免当前非唯一索引下重复建模。
- 单条与批量删除在一个事务内软删实例及关联关系；批量 ID 任一非法则拒绝整个请求。
- 迁移 `0025` 新增唯一值登记表，以 JSONB 唯一约束兜底（`0026` 后范围为 `(tenant_id, model_id, value)`）。数据库触发器覆盖直接 SQL、工单回写、实例软删/恢复和模型唯一键变更；缺失、null、空白字符串及空数组不登记，数字按 JSONB 等值比较。登记与业务写入同事务提交。唯一值受 PostgreSQL B-tree 索引项大小限制，不适合作为任意大 JSON 文档的索引键。
- 迁移在锁定模型和实例表后回填登记；历史重复值会使整个迁移失败，须人工核对并修复后重试，不自动丢弃数据。实例删除也先锁模型再锁实例，保持应用写入的锁顺序。直接 SQL 多行写入仍可能遭遇数据库死锁回滚，调用方应重试整个事务。
- 列表、详情和导入结果 DTO 放在 `cmdb-api`，保留现有 ID、时间和分页字段格式，动态属性仍为 JSON。
- 迁移 `0026` 已对实例、关系和作为开通来源的工单实施租户隔离；模型/属性保持共享目录。历史记录归属为 `NULL` 并隔离，等待逐项确认；详见 [CMDB 租户边界](cmdb-tenant-isolation.md)。
- 扫描任务批量删除按字符串 ID 校验，单条 SQL 原子更新；任一数据库错误导致整批失败，缺失或已删除的 ID 按幂等空操作处理。

验证：`cargo test -p rustset-cmdb-api -p rustset-cmdb-server -p rustset-infra-server`。设置 `TEST_DATABASE_URL` 后加 `-- --include-ignored`，在独立临时 schema 验证并发查重、补丁合并、工单回写、唯一键变更、删除恢复及批量失败回滚。`bash script/test-database-migrations.sh` 验证全部 28 个迁移和基线；无 Docker 时可提供指向空的 UTF-8 临时数据库的 `TEST_DATABASE_URL`。参考快照由全链路迁移后的干净数据库导出，运行时不依赖它。

## 属性演进兼容（2026-09-28）

- 属性新增、修改和删除与实例写入共用模型锁，避免实例按旧定义写入、属性定义却已切换的竞态。
- 新增必填属性时，已有实例必须能通过合法默认值补齐；修改类型、选项或必填约束前会校验全部现存实例，不兼容则整个事务回滚。默认值及选项定义本身也会按属性类型校验。
- 删除属性会在同一事务中移除实例 JSONB 中的对应字段；若属性仍被模型唯一键引用则拒绝删除，必须先调整唯一键。
- Excel 导入/导出、批量删除、批量字段修改和整批原子导入已经落地；默认导入仍按行提交并返回失败明细。

## 二期（CMDB 深化）

- 数值计算属性已支持：`+ - * /`、括号、常量和 `${attribute_code}` 引用；实例写入时计算，公式变更时事务内重算。
- 属性字体颜色已支持十六进制颜色并应用到实例列表；属性触发器已支持“条件属性等于指定值时写入目标属性固定值”，并在实例写入事务内执行。

## 三期（资源自动发现 v1）

- 已开始：`POST /infra/asset/discover` 接受 1–256 个明确 IP 和 1–64 个 TCP 端口，在后台探测后幂等写回 `infra_asset`，保留扫描时间、来源和设备类型。
- 云资源台账发现已开始：`POST /infra/cloud-asset/discover` 将现有云资源台账幂等汇总到 `infra_cloud_asset`，保留 Provider、区域、实例、规格和 IP 信息。
- 后续接入云厂商、虚拟化平台和存量资产采集器，统一复用该资产台账与 CMDB 同步链路。
- 视图：机房层级树已加入物理机房管理页，按服务商、机房类型和机房三级聚合，并跟随搜索结果过滤。

## 关系拓扑视图（2026-09-28）

- 实例列表提供关系拓扑入口，按模型着色展示有向关系、关系名称、实例标识及与根实例的距离，节点可点击查看详情。
- `GET /cmdb/relation/topology` 支持 1–4 层展开，默认限制 80 个节点，最多 200 个节点及 800 条边；达到容量限制时返回 `truncated` 提示，避免高关系度数据拖垮页面。
- 后端以双向广度优先遍历发现节点，但保留 `sourceId → targetId` 原始方向；软删除的关系或端点不会进入拓扑。

## 实例批量字段修改（2026-09-28）

- 实例列表支持选择最多 500 条同模型实例，一次修改一个或多个动态属性；未选择的属性保持不变，显式留空可清除可选属性。
- `PUT /cmdb/instance/update-list` 复用 `cmdb:instance:update` 权限。服务端按模型锁 → 实例 ID 顺序行锁执行，在同一事务内完成类型、必填、选项和唯一键校验。
- 任一实例不存在、跨模型、字段不兼容或唯一键冲突时整批回滚；响应区分实际更新与内容未变化的数量。

## 资产台账映射（2026-09-28）

- 资产管理页支持一键将有效 `infra_asset` 台账同步到专用 CMDB 模型 `infra_asset_inventory`，覆盖原扫描字段、采集表 43 个字段及网段归属字段。
- `asset_id` 对应 `infra_asset.id` 并作为模型唯一键；重复同步执行幂等更新，返回新增、更新、未变化及源资产已失效的数量。
- 模型创建、属性补齐和实例写入在同一事务内完成，并复用 CMDB 模型编码 advisory lock → 模型行锁协议；固定字段定义冲突、重复模型或重复实例会拒绝整批同步。
- 同步只覆盖映射字段，保留模型扩展属性。源资产软删除不会自动删除 CMDB 实例，避免破坏已经建立的关系和运维流程；结果中的 `stale` 数量用于后续人工治理。
- 接口 `POST /infra/asset/sync-cmdb` 复用 `infra:asset:update` 权限，不新增数据库迁移。

## 云平台适配层（OpenTofu）——已落地 v1（迁移 0010）

### 公有云开通补齐（2026-09-25）

- 新增腾讯云 CVM、华为云 ECS 模板，补齐阿里云 ECS 的镜像、可用区、子网、安全组和系统盘容量。腾讯云使用 `tencentcloudstack/tencentcloud`。
- 开通弹窗的凭据选择、镜像与规格现在实际传入执行器；凭据必须属于工单的平台，支持 `active` / `enabled` 状态。多个凭据时必须明确选择；厂商以所选凭据为准，区域优先工单、其次凭据中的区域名称。
- 连接测试通过只读数据源执行 `tofu plan` 查询区域/可用区，不创建云资源。成功表示查询 API 可用，不保证账号有开通权限。
- 只有显式 `cloudCategory=demo` 才运行模拟模板。真实云缺少配置、镜像、网络参数时拒绝开通。
- 开通参数保存在现有 `target_config` 中；同工单使用数据库 advisory lock 防止并发执行，失败重试保留工作区，禁止切换厂商或凭据配置。超时终止 tofu 子进程，outputs 读取失败不会推进到待交付。
- 自动开通可通过工单 `targetConfig` JSON 提供 `configId`、`imageId`、`flavor`、`availabilityZone`、`subnetId`、`securityGroups`，腾讯云另需 `vpcId`；数量沿用审批后的工单值。不要放入密码或密钥。
- 三家模板使用真实 Provider 做 `tofu validate`（含连接测试数据源），不使用真实云凭据。实云 `plan/apply` 和故障恢复仍需目标环境验收。
- 尚未实现：存量资源自动发现、vCenter 采集、Proxmox/OpenStack/vSphere 开通、真实云端停机/销毁/变配。当前只支持三家公有云 AK/SK 开通，不代表界面列出的其他认证方式已接通。

验证命令：`cargo test -p rustset-framework-tofu --test compute -- --include-ignored`（需要 tofu 和 Provider 下载网络）。部署时将 `TOFU_WORKSPACE_ROOT` 指向持久化目录并保留 state；默认临时目录不适合作为生产状态存储。

v1 交付（已 E2E 验证：规则命中 → 建单自动审批 → tofu init/apply → outputs 回读 → CMDB 回写）：

- `rustset-framework-tofu`：模板渲染（demo/null 与 aliyun 参考模板）、子进程执行（超时/代理透传/环境变量注入凭据）、`tofu output -json` 解析；6 个单元测试。
- 工单真实 provision：`POST /infra/resource-ticket/{id}/provision` 读取云凭据（infra*cloud_provider_config）→ 渲染工作区 → init/apply → `apply_status/apply_log/tf_outputs/tofu_workspace` 落库 → 成功后写入 CMDB（`cloud*<resource_type>`模型，键`ticket_id`）并推进到待交付；失败记录原因且工单留在待配置。
- 自动审批：`infra_approval_rule`（资源类型 + CPU/内存/数量阈值 + auto_provision），建单时命中即自动审批，auto_provision 时立即开通（失败不阻断建单）。规则经 `/infra/approval-rule/*` CRUD 管理（infra:approval-rule:\* 权限码）。
- 环境要求：安装 OpenTofu（≥1.6），可用 `TOFU_BINARY/TOFU_WORKSPACE_ROOT/TOFU_TIMEOUT_SECONDS` 调节；网关进程 PATH 需含 tofu。

目标：以 [OpenTofu](https://opentofu.org/)（Terraform 开源分支）+ 各家 Provider 作为统一云资源适配与开通执行层。

对接锚点（现有代码）：

- `infra_cloud_provider_config` / `infra_cloud_platform` / `infra_cloud_zone`：云凭据与平台台账 —— 作为 Provider 凭证来源（敏感字段脱敏沿用现有约定）。
- `infra_resource_ticket`：工单状态机（approve → provision → deliver，0005 已补工作流字段）—— provision 阶段触发 OpenTofu apply。
- `infra_task`（trigger-scan 已有执行语义）：任务编排与执行记录。

设计草案：

1. 新增 `cmdb` 侧"云资源模型"（如云主机/云盘/EIP），OpenTofu state 导入生成实例 —— CMDB 成为云资源的统一台账。
2. 后端新增 `tofu-executor`：按工单渲染 `.tfvars` 模板 → `tofu plan/apply`（独立子进程、超时与日志审计）→ state 回读生成/更新 CMDB 实例。
3. 凭据经云凭据表注入 Provider 环境变量，不落盘到仓库。

## 自动审批与 Agent ——自动审批已落地（0010），Agent v1 已落地（0011）

- **自动审批流（已落地）**：`infra_approval_rule` 阈值规则引擎，建单命中即自动 approve，auto_provision 时直接触发 OpenTofu 开通。
- **Agent v1（已落地）**：复用 `ai-server` 工具调用框架，注册 5 个 Rust 执行工具——`cmdb_model_list` / `cmdb_instance_query` / `asset_query` / `ticket_query` / `ticket_create`（建单走同一套自动审批规则；开通执行仍在工单流）。预置公共聊天角色“运维助理”（系统提示词 + 5 工具绑定），在 AI 聊天页选择该角色即可对话使用。实测链路已通至真实模型 API（需在“AI 模型管理”配置有效 API key 后即可对话）。
- **Agent 二期**：工具沙箱与配额、`tofu plan` 预览工具（只读）、Agent 建单后自动跟踪开通结果、审批链人工闸门参数化。
