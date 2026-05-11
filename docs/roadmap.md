# Albert 实施路线图

## Phase 1: Foundation ✅

目标：建立项目边界、文档系统和工作区骨架。

交付：

- 双语 README
- PRD、架构文档、路线图
- `llmdoc` 初始化
- Tauri + React + TypeScript UI 壳
- Rust workspace 与核心 crate 占位
- Canonical API Schema 基础模型
- OpenAPI / cURL / OpenAI / Storage / Gateway 的占位接口

## Phase 2: Parsing And Persistence ✅

目标：打通从输入到存储的主链路。

交付：

- OpenAPI JSON/YAML 基础解析
- cURL 常见请求解析
- Canonical API Schema 转换
- SQLite 迁移、`save_collection` / `load_collection` / `list_*`
- Tauri `parse_api_description` / `import_api_description` / `list_*` 命令
- Parser + Storage 单元测试

## Phase 3: Static Mock Runtime ✅ (minimum viable)

目标：让 Albert 成为可用的静态 Mock 工具。

交付：

- `MockGateway`：axum + hyper + tokio 的真实 HTTP 服务，支持 graceful shutdown。
- `RouteTable`：方法 + 路径模板匹配（字面量段优先 `{param}` 通配）。
- `success / empty / error` 样例选择，支持查询参数 `?__albert_mock=...` 和运行时 overrides。
- `CORS permissive` 层、特殊 `/__albert/status` 路由、`x-albert-mock-kind` 响应头。
- Tauri 命令：`start_mock_server` / `stop_mock_server` / `mock_server_status`，共享 `AppServices` 状态。
- 前端 Mock Server 面板：启动/停止、端口配置、路由列表、复制 URL。

状态：可用。后续增强项包含基于 Schema 的合成数据、请求录制、延迟模拟。

## Phase 4: AI-Assisted Mocking 🚧 (first slice delivered)

目标：引入 OpenAI 生成能力，但保持架构可控。

已交付：

- `OpenAiChatAdapter`：`reqwest` + `response_format: json_object`，可对接任意
  OpenAI 兼容端点（OpenAI、Azure OpenAI、Qwen 兼容代理等）。
- `PromptBundle` 构造：基于 Canonical Schema 生成 system/user prompt 与 JSON
  schema hint。
- 三种意图（success/empty/error）与 Markdown 代码围栏清洗。
- 结构化输出校验：按匹配的 Canonical response schema 校验生成结果，并在失败时
  追加错误明细做 bounded repair retry；Provider profile 可配置
  `schema_repair_attempts`（默认 2，范围 0–5，0 表示禁用修复重试）。
- Canonical `SchemaNode` 保留并校验 OpenAPI 的 `format`、`pattern`、
  length、numeric min/max、array min/max 约束；OpenAI prompt schema hint 同步输出这些约束。
- Canonical `SchemaNode` 进一步保留并校验 `multipleOf`、`uniqueItems`、
  `additionalProperties`（含 closed object 与 typed map）约束。
- Canonical `SchemaNode` 保留并校验对象 `minProperties` / `maxProperties`
  约束，OpenAI prompt schema hint 同步输出。
- Canonical `SchemaNode` 保留并校验数组 `contains` / `minContains` /
  `maxContains` 以及对象 `dependentRequired` / `dependentSchemas` 约束；
  OpenAI prompt schema hint 同步输出这些约束，让 AI 生成与修复重试能看到更完整的
  JSON Schema 合同。
- Canonical `SchemaNode` 保留并校验条件 schema `if` / `then` / `else`；
  OpenAPI raw overlay 可捕获这些 OpenAPI 3.1/JSON Schema 关键字，OpenAI prompt
  schema hint 同步输出。
- Canonical `SchemaNode` 保守支持 object-level `unevaluatedProperties: false`：
  OpenAPI raw overlay 可捕获该关键字，validator 会拒绝未被 `properties` 或 typed
  `additionalProperties` 覆盖的字段，OpenAI prompt schema hint 同步输出。
- Canonical `SchemaNode` 保守支持 array tuple 约束 `prefixItems` 与
  `unevaluatedItems: false`：validator 会先校验固定位置 tuple，再在没有 tail
  `items` schema 时拒绝额外数组项，OpenAI prompt schema hint 同步输出。
- Tauri 命令 `generate_mock_example` 支持会话级别 API key override、
  `persist=true` 时 `SqliteStore::replace_mock_example` 同步更新样例与快照。
- Tauri 命令 `provider_env_status` 与 Providers 面板可区分会话 override、
  后端环境变量存在、缺失和静态浏览器不可检查状态。
- Tauri 命令 `list_provider_configs` / `save_provider_config` /
  `delete_provider_config` 与 Providers 面板的 Saved profiles 区域打通
  SQLite `provider_configs`，支持保存、载入、复制派生、删除 Provider 配置
  （不保存 API key）。
- Provider-specific 参数已覆盖 OpenAI-compatible、Azure OpenAI 与 Azure
  Responses：
  `api_type` 决定请求路径和鉴权 header，Azure profile 可配置
  `azure_deployment` 与 `azure_api_version`。
- `openai_responses` provider 类型已覆盖 OpenAI Responses API 基础路径：
  请求发送到 `/v1/responses`，使用 `instructions` / `input` /
  `text.format: json_object`，并从 `output_text` 或 `output[].content[].text`
  解析 JSON 负载。
- `azure_openai_responses` provider 类型已覆盖 Azure OpenAI Responses API
  基础路径：请求发送到 Azure resource root 的 `/openai/v1/responses`，使用
  `api-key` 鉴权，body 的 `model` 使用 Azure deployment 名称，并复用
  Responses API 的 JSON object 输出解析。
- Provider profile 已支持生成参数控制：temperature 默认 0.7 并 clamp 到
  0–2；max output tokens 为空时使用 provider 默认，OpenAI-compatible/Azure
  Chat 请求映射为 `max_tokens`，OpenAI Responses 请求映射为
  `max_output_tokens`。
- Provider profile 已支持 OpenAI Responses reasoning effort 控制：
  Providers 面板可保存 Default/None/Minimal/Low/Medium/High/Xhigh，
  Default 不下发请求字段；非空值在 OpenAI Responses 请求中映射为
  `reasoning.effort`。Chat/Azure Chat 当前仍不发送 reasoning 参数。
- Provider profile 已支持 schema repair retry 控制：Providers 面板可保存
  `schema_repair_attempts`，OpenAI adapter 会按该值限制结构化输出校验失败后的
  修复请求次数，范围 0–5，默认保持 2 次。
- Provider profile 已支持环境标签：SQLite/Tauri/Providers 面板会保存可选
  `environment`，Saved profiles 可按 local/staging/prod 等标签过滤，形成多环境
  provider 管理的第一层分组；当前 environment 仍是 profile 元数据，不参与自动
  provider routing。
- 前端 ResponsePane 提供「Generate <kind>」/「Generate all」按钮和复制负载、
  note 展示；当目标 mock 槽位已有样例时，单个生成、批量生成和 prompt 预览会
  把该样例作为 `generation_context`，让模型基于上一版样例迭代而不是完全重写。
- Try-it 面板可将最近一次 JSON 响应保存为当前 endpoint 的 mock
  example，形成“实际响应 → 可编辑样例”的第一段录制闭环；保存前会用当前
  response schema 做 mismatch 提示（Tauri 中复用 canonical Rust validator，
  静态预览 fallback 到轻量前端校验），但仍允许保留真实响应，并可手动选择保存到
  `success / empty / error` 槽位。最近一次 Try-it 响应也可直接作为
  `generation_context` 做 **AI refresh latest** 或 prompt 预览，不必先从缓存列表
  找回该请求。
- Try-it 成功请求会按 normalized request fingerprint 写入 SQLite
  `request_fingerprint_cache`，记录请求快照与响应快照；重复请求会命中同一
  指纹并递增 `hit_count`，UI 显示 `cached` / `cache hit ×N`；Try-it 还可
  查看当前 endpoint 最近缓存指纹，并把缓存响应保存回
  `success / empty / error` mock 槽位。缓存行超过 24 小时会标记为 stale，
  可 Replay 回填请求草稿以手动刷新该指纹，也可单条 Remove 或按 endpoint
  Clear stale。缓存行还可通过 **AI refresh** 把该请求/响应快照作为
  `generation_context` 传入 OpenAI 生成路径，定向刷新所选
  `success / empty / error` mock 槽位；当 stale 指纹存在时，Try-it 会显示
  **Refresh queue**，集中展示 stale/refreshable 数量，并可对 stale 且响应体
  可持久化的指纹做手动批量 AI refresh 或预览首个 stale prompt。Mock
  Server 可开启 **Request cache routing**，启动/更新时把近期缓存响应注入网关
  内存；Runtime 面板显示已注入条目数，并可 Reload 新录制的缓存响应；Try-it
  成功保存新指纹后也会在 routing 已开启时提供内联 reload 入口；`albert-cli
  serve --use-request-cache` 可在 headless 模式启动时注入缓存。匹配
  method/path/query/header/body 指纹的请求会返回缓存响应并标记
  `x-albert-mock-source: cache`。当前仍不做后台自动刷新。
- Mock Gateway 已支持 runtime 条件样例规则：`conditional_example_rules` 可按
  query/header/body equality 条件为 route 选择 `success / empty / error`，优先级
  低于显式 query/per-route override、高于 request cache routing，并在命中时返回
  `x-albert-mock-source: conditional`；当前规则可通过 config bundle 和
  `/__albert/config` 往返，桌面端 Mock Server Routes tab 已提供规则编辑器，
  支持 query/header/body 条件、多条件 AND、规则顺序调整和运行时 Apply。
- cURL parser 的边界进一步扩展：重复 query/header 会作为多个 canonical
  parameter 保留，`--data-binary @file` / `<file` 以 `format: binary` 表达，
  `-F/--form` 与 `--form-string` 会生成 `multipart/form-data` request body
  object schema；暂不读取本地文件内容，也不承诺完整 curl flag 兼容。
- 工作区/导入记录 first slice：SQLite `api_collections` 已记录
  `created_at` / `updated_at`，重复导入保留创建时间并刷新更新时间，重命名和
  mock example 编辑也会刷新 `updated_at`；collection summary 按最近更新排序，
  Sidebar 对 imported collection 显示最近更新时间和 endpoint 数，让用户能看清
  当前工作区最近导入/更新的资产。顶部工作区标题旁已增加 Workspace collections
  抽屉入口（也可通过 `Mod+Shift+W` 或命令面板打开），集中展示 collection 数、
  endpoint 数、来源、更新时间、方法分布，并复用打开、重命名、导出、删除、刷新和
  导入动作；多项目切换和跨 workspace provider set 仍未进入该切片。
- 导入演进可见性 first slice：`import_api_description` / `import_bundle` 在覆盖保存
  同一 collection id 前，会比较旧 canonical snapshot 和新解析结果，返回
  endpoint-level diff 摘要（added / changed / removed / unchanged）。前端导入成功
  status/toast 会展示这份摘要；App 还保留最近一次导入报告，可从命令面板或
  `Mod+Shift+I` 打开 Import report 抽屉，按 added/changed/removed 分组查看
  endpoint 列表，并直接打开仍存在的 added/changed endpoint 或查看该 endpoint 的
  success prompt preview。Changed endpoint 还会展示粗粒度变更原因（metadata、
  parameters、request body、responses、auth）和简短明细（参数、请求体、响应状态码/
  schema 等）；从 changed 行打开 prompt preview 时，这些信息会作为 generation
  context note 传入 prompt，也可一键 AI Refresh success mock 并持久化。当前 changed
  比较接口契约并忽略 mock examples，Import report 也可批量 Refresh 全部可刷新
  changed endpoint；这还不是完整 Schema Diff Engine 或版本历史。

待继续：

- 更完整的 JSON Schema 关键字覆盖（如完整 evaluated-set 语义等）与更细粒度的修复策略。
- 更完整的多环境策略、Provider-specific 参数矩阵和 streaming/tool calling 等
  高级生成控制。
- streaming/tool calling 等 Responses API 高级变体。
- 请求指纹缓存驱动的后台自动录制和自动过期样例刷新策略。
- 更完整的项目/工作区历史页面、多项目切换和跨 workspace provider set。

## Phase 5: Intelligence And Evolution

目标：补齐高阶能力与长期维护机制。

交付：

- Responses API 兼容
- 请求指纹缓存
- AOT / JIT 策略
- 文档 diff / Schema Diff Engine（当前仅有重复导入 endpoint-level 摘要）
- 过期样例定向刷新
- 工作区历史 / 导入记录
- 多 Provider 扩展
- 录制实际响应 → 生成真实样例（Try-it 单次保存已具备，自动录制待做）
- 延迟/错误率注入
