# Albert 架构文档

## 1. 总体原则

- 文档优先于实现，先固化边界再铺功能
- 使用 Canonical API Schema 作为系统内部统一表示
- UI、领域模型、存储、网关、AI Provider 之间保持明确边界
- 一期优先搭骨架，未实现能力必须显式暴露

## 2. 系统分层

### 2.1 桌面应用层

`apps/desktop`

职责：

- 提供项目概览、导入、接口浏览、Provider 配置、Mock Server 状态面板
- 承载后续项目管理和交互式配置
- 通过 Tauri command 与 Rust 核心能力通信
- 当前 UI 已从临时工作台推进为可用的产品化控制台：支持导入、接口浏览、
  Mock Server 运行配置、Provider profile、AI mock 生成、Try-it 请求发送、
  响应录制和请求指纹缓存管理

### 2.2 领域核心层

`crates/albert-core`

职责：

- 定义 Canonical API Schema
- 定义请求、响应、样例、Provider 配置等共享模型
- 为 parser、gateway、storage、provider 提供统一契约

### 2.3 协议解析层

`crates/albert-parser`

职责：

- 接收 OpenAPI / cURL 输入
- 解析为内部标准结构
- 为未来扩展 Postman / GraphQL 保留统一 parser 抽象

### 2.4 存储层

`crates/albert-storage`

职责：

- 管理 SQLite schema 与迁移
- 提供仓储接口
- 保存项目、接口定义、标准化 schema、Mock 样例、Provider 配置、Mock
  Server 场景/偏好、请求指纹缓存

### 2.5 网关层

`crates/albert-gateway`

职责：

- 承接本地 Mock Server 的请求匹配与调度
- 运行真实本地 HTTP Mock Server（axum + tokio），支持路由匹配、样例选择、
  运行时配置热更新、请求日志/指标、延迟/错误注入、鉴权 gates、请求 body
  schema enforcement、proxy upstream、OpenAPI/status/config 辅助路由

### 2.6 AI Provider 层

`crates/albert-openai`

职责：

- 提供 OpenAI-compatible Chat Completions、Azure OpenAI Chat Completions、
  OpenAI Responses API 和 Azure OpenAI Responses API 适配
- 构造基于 Canonical Schema 的 prompt/schema hint，并支持 JSON-object
  生成、响应解析、schema 校验和可配置 bounded repair retry
- 使用 Provider profile 中的生成控制参数：`temperature` 默认 0.7 并限制在
  0 到 2；`max_output_tokens` 为空时不下发，Chat/Azure Chat 请求映射为
  `max_tokens`，OpenAI/Azure Responses 请求映射为 `max_output_tokens`；
  可选 `reasoning_effort` 会在 Responses 请求中映射为
  `reasoning.effort`，Chat/Azure Chat 当前不下发该参数；可选
  `schema_repair_attempts` 控制 schema 校验失败后的修复重试次数，默认 2，
  范围 0–5，0 表示禁用修复重试
- 接收可选 `generation_context`，让最新 Try-it 响应或 Try-it 缓存中的真实
  请求/响应上下文参与单次 AI mock 刷新

## 3. Canonical API Schema

内部数据模型不直接保存原始 OpenAPI 结构，而是做一层规范化表达。原因如下：

- cURL、OpenAPI、未来的 Postman / GraphQL 需要统一落点
- AI Prompt 和结构化输出更适合围绕 JSON Schema 风格数据组织
- 存储层不应与任意上游协议格式强耦合
- 当前 Canonical Schema 已覆盖常见 JSON Schema 约束，以及
  `contains` / `dependentRequired` / `dependentSchemas` / `if` / `then` /
  `else` / `prefixItems` / object-level `unevaluatedProperties: false` /
  `unevaluatedItems: false` / 布尔 JSON Schema 等用于提高 AI 生成和修复重试准确度的高级约束

推荐结构：

- `CanonicalApiCollection`
- `CanonicalEndpoint`
- `CanonicalParameter`
- `CanonicalRequestBody`
- `CanonicalResponse`
- `SchemaNode`
- `MockExample`

## 4. 当前运行流

### 4.1 导入流（当前已实现）

1. 用户在 UI 选择 OpenAPI 文件或粘贴 cURL
2. 前端调用 Tauri command
3. Rust parser 层识别输入类型
4. 解析为 Canonical API Schema
5. 写入存储层
6. UI 展示接口、schema、默认样例、导入/更新时间和可编辑 mock 资产
7. 重复导入同一 collection id 时，Tauri 导入命令会在覆盖保存前比较旧 snapshot
   和新解析结果，返回 endpoint-level diff 摘要（added / removed / changed /
   unchanged）；当前 changed 比较接口契约并忽略 mock examples，同时附带粗粒度
   变更原因（metadata、parameters、request body、responses、auth）和简短明细
   （参数、请求体、响应状态码/content type/schema 等）。前端在导入成功的
   status/toast 中展示摘要，并保存最近一次导入报告供 UI 抽屉查看明细和原因

当前命令面：

- `bootstrap_summary`
- `parse_api_description`
- `import_api_description`
- `import_bundle`
- `list_imported_collections`
- `list_imported_endpoints`
- `load_collection_snapshot`
- `export_collection_json`
- `export_all_collections_json`
- `delete_collection`
- `rename_collection`

### 4.2 资产查看流（当前已实现）

1. 用户进入接口详情页
2. UI 读取 Canonical Endpoint 与 MockExample
3. 根据 `success / empty / error` 切换展示
4. ResponsePane 支持复制、编辑、保存和 AI 生成/批量生成样例
5. Try-it 面板可向运行中的 Mock Server 发送请求、保存真实响应为样例、查看最近
   请求历史和请求指纹缓存

### 4.3 本地持久化流（当前已实现）

1. Tauri command 创建或复用 SQLite 数据库
2. 执行 migration
3. 保存 `api_collections`
4. 保存 `api_endpoints`
5. 保存请求/响应 schema 到 `api_schemas`
6. 保存默认 `success / empty / error` mock examples
7. `api_collections` 记录 `created_at` / `updated_at`；重复导入、重命名和 mock
   example 资产编辑会刷新 `updated_at`，`list_imported_collections` 返回这些
   metadata 并按最近更新倒序排列，Sidebar 用它显示最近导入/更新时间
8. 后续编辑/AI 生成/录制响应通过 `replace_mock_example` / `save_mock_example`
   同步更新样例表和 collection snapshot JSON
9. 导入差异摘要不新增数据库表，直接使用即将覆盖的 `raw_snapshot` 与新
   `CanonicalApiCollection` 比较；完整 Schema Diff Engine 和版本历史仍是后续演进

### 4.4 Mock Server 运行流（当前已实现）

1. 前端 Mock Server 面板调用 `start_mock_server`
2. Tauri 从 SQLite 解析 collection 快照并交给 `MockGateway`
3. `albert-gateway` 生成 `RouteTable` 并绑定本地 HTTP 监听
4. 请求按 method + path template 匹配，选择 query override、运行时 override 或
   endpoint 默认样例；Mock Server Routes tab 还可配置 per-route
   `conditional_example_rules`，按 query/header/body equality 条件选择
   `success / empty / error` 样例
5. 运行期间可通过 `update_mock_server` 热更新 chaos、headers、auth gates、rate
   limits、status overrides、schema enforcement、proxy upstream 等配置
6. 请求日志和指标通过 `mock_server_requests` / `mock_server_metrics` 返回 UI

### 4.5 AI 与请求缓存流（当前已实现）

1. ResponsePane 调用 `generate_mock_example` 生成并可持久化样例；目标 mock 槽位
   已有样例时，单个生成、批量生成和 prompt 预览会把该样例作为
   `generation_context`
2. Providers 面板管理非 secret provider profiles，并支持 session-only API key
   override、连接测试、temperature、max output tokens 与 reasoning effort
   生成参数
3. Try-it 成功请求会 best-effort 写入 `request_fingerprint_cache`
4. 最新 Try-it 响应可直接作为 AI refresh / Prompt preview 上下文；缓存行也可
   Replay、Save as mock、Remove、Clear stale、Prompt preview、AI refresh；
   stale 缓存存在时，Try-it 显示 Refresh queue，把批量 AI refresh、首个 stale
   prompt 预览和清理操作集中在缓存列表之前
5. Mock Server 的 Request cache routing 开关可在启动/更新时把近期缓存响应注入
   gateway 内存；Runtime 面板会显示已注入条目数，并可通过 Reload request cache
   在不重启监听器的情况下重新注入新录制缓存；Try-it 在成功保存新指纹后也会在
   routing 已开启时提供内联 reload 入口。`albert-cli serve --use-request-cache`
   也可在 headless 模式启动时注入缓存。运行时命中相同请求指纹会直接返回缓存
   响应。Gateway 不在请求时访问 SQLite，后台自动刷新仍未实现

## 5. 数据持久化建议

一期推荐的 SQLite 逻辑实体：

- `projects`
- `api_collections`
- `api_endpoints`
- `api_schemas`
- `mock_examples`
- `provider_configs`
- `gateway_preferences`
- `gateway_scenarios`
- `request_fingerprint_cache`

建议策略：

- 原始输入可选保留一份快照，便于重新解析
- 存库主体应是标准化后的结构
- 样例与 endpoint 分离，便于后续扩展状态与版本
- 当前实现已支持 collection、endpoint、schema、example、provider config、
  gateway scenario/preference、request fingerprint cache 的基础落库；collection
  summary 已包含创建/更新时间，用于工作区导入记录的第一层可见性

## 6. UI 信息架构

- `TopBar`: 导入入口、全局工作区标题、Workspace collections 入口、主题切换
- `ImportReportPanel`: 最近一次导入报告抽屉，展示 added / changed / removed /
  unchanged 计数和 endpoint 列表；added/changed 可直接打开新 snapshot 中仍存在的
  endpoint，也可直接打开该 endpoint 的 success prompt preview；changed 行展示粗粒度
  变更原因和简短明细，并把这些信息作为 prompt preview / AI Refresh success mock 的
  generation context note；报告头部可批量 Refresh 全部可刷新 changed endpoint，removed
  只展示
- `WorkspacePanel`: 右侧工作区抽屉，汇总当前 imported collections 的数量、
  endpoint 数、SQLite/Preview 来源、最近更新时间、方法分布，并复用打开、
  重命名、导出、删除、刷新和导入动作；当前仍是导入记录 first slice，不承担完整
  多项目切换
- `Sidebar`: collection/endpoint 列表、搜索、标签过滤、collection 管理和最近更新
  metadata
- `Endpoint Detail`: 请求参数/headers/body/responses/schema/AI mock tabs
- `ResponsePane`: mock 样例查看、编辑、AI 生成、prompt 预览
- `Try-it`: 请求构造、发送、响应录制、历史、请求指纹缓存和 AI refresh
- `Providers`: provider profile、API key override、环境变量状态、连接测试
- `Mock Server`: runtime、routes、requests、scenarios、chaos、auth/schema/proxy 配置

当前说明：

- 当前桌面界面已是可用控制台，但仍可继续向更完整的项目/工作区管理演进
- UI 仍不应反向决定领域模型；复杂行为继续通过 canonical types 和 Tauri command
  边界进入 Rust crates

## 7. 模块边界与依赖方向

- `albert-core` 不依赖业务实现 crate
- `albert-parser` 依赖 `albert-core`
- `albert-storage` 依赖 `albert-core`
- `albert-gateway` 依赖 `albert-core`
- `albert-openai` 依赖 `albert-core`
- `apps/desktop/src-tauri` 依赖所有核心 crates，并对前端暴露统一 command

依赖方向必须保持单向，避免形成“UI 反向决定领域模型”的结构。

## 8. 命名规范

- 产品名统一写作 `Albert`
- 代码级模块使用 `albert-*`
- Canonical Schema 相关类型统一前缀 `Canonical`
- UI 页面与状态使用清晰英文命名
- 文档中第一次出现关键术语时采用“中文 + 英文”并列写法

## 9. 后续演进位

- Azure Responses、streaming/tool calling、reasoning control 等高级 Provider 变体
- 静态样例与动态生成结合
- 文档 diff / Schema Diff Engine 与样例失效刷新；当前仅有重复导入时的
  endpoint-level diff 摘要
- 请求指纹缓存驱动的后台自动录制、过期样例刷新和网关侧自动样例选择
- 更完整的多环境/provider matrix
- 工作区历史、项目切换、导入记录与团队协作边界
