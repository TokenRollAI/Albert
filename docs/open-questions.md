# Albert 开放问题清单

这份清单用于收敛你后续希望我持续追问的设计决策，避免问题散落在聊天记录里。

## 1. 品牌与命名

- 项目对外宣传时，是否强调 “tribute to Albert II” 作为主叙事
- README 之外，是否还需要单独的品牌使用说明

## 2. 数据模型

- ✅ 原始 OpenAPI / cURL 输入是否保留快照：当前存储已保留 `source_snapshot`，
  同时以 Canonical Schema 作为主要运行模型。
- Canonical Schema 是否需要保留字段级示例与描述优先级规则：部分已保留
  `example`、`description` 和常见约束；仍需明确冲突优先级、vendor extension
  规则和 OpenAPI 3.1 edge cases。

## 3. UI 演进

- ✅ 一期界面更偏控制台式工程工具，还是更偏产品化桌面应用：当前方向已经是
  产品化工程控制台，优先支持高频 API mock 工作流，而不是营销式入口页。
- 工作区历史和导入记录已开始 first slice：`api_collections` 记录
  `created_at` / `updated_at`，Sidebar 展示 imported collection 最近更新时间和
  endpoint 数；顶部 Workspace collections 抽屉可汇总当前 imported collections 的
  数量、endpoint 数、来源、更新时间、方法分布，并复用打开、重命名、导出、删除、
  刷新和导入动作；重新导入、重命名和 mock example 编辑会刷新更新时间。完整项目
  切换、跨 workspace 管理和更细的导入版本历史仍待定。
- ✅ 导入后是否要立即暴露接口演进变化：当前重复导入同一 collection id 时会返回并
  展示 endpoint-level diff 摘要（added / changed / removed / unchanged），changed
  忽略 mock examples，只比较接口契约。前端保留最近一次 Import report 抽屉，可查看
  added / changed / removed endpoint 明细，并打开仍存在的 endpoint 或查看 success
  prompt preview；changed 行会展示 metadata / parameters / request body / responses /
  auth 等粗粒度变更原因以及参数/请求体/响应状态码和 schema 的简短明细，且 prompt
  preview 会把这些信息作为 generation context note；changed 行也可一键 AI Refresh
  success mock，报告头部可批量 Refresh 全部可刷新 changed endpoint。完整字段级 schema
  diff、版本历史、自动标记过期 mock 和跨导入批次回滚仍待定。

## 4. Parser 策略

- OpenAPI 3.0 和 3.1 是否同时支持：当前 OpenAPI v3 基础路径可用，3.1
  JSON Schema 差异仍需明确覆盖边界；`contains`、`dependentRequired` 和
  `dependentSchemas`、`if` / `then` / `else` 已进入 Canonical Schema
  解析/校验/prompt hint；object-level `unevaluatedProperties: false` 已有保守
  closed-unknown-fields 支持；`prefixItems` + `unevaluatedItems: false` 已有保守
  tuple closure 支持；布尔 JSON Schema（如 `items: false`）已有保守解析/校验/
  prompt hint 支持。完整 evaluated-set 语义仍待定。
- cURL 解析的最小语法边界是什么：当前常见请求可用，且已保守支持
  `multipart/form-data`、`--data-binary` 文件引用以及重复 header/query 的
  canonical 保留；复杂 shell quoting、真实本地文件读取、嵌套 multipart 和更多
  curl 传输/认证 flag 仍不作为已承诺边界。

## 5. Provider 策略

- ✅ OpenAI Chat Completions 之外，Responses API 进入哪一个 phase：OpenAI
  Responses API 与 Azure OpenAI Responses API 基础路径已经进入 Phase 4/5
  first slice；高级 streaming/tool calling 仍待定。
- ✅ OpenAI Responses reasoning effort 是否进入 Provider profile：当前已支持
  profile 持久化和 Providers 面板选择，并仅在 OpenAI Responses 请求中下发
  `reasoning.effort`；Chat/Azure Chat 的 reasoning 兼容策略仍待定。
- Provider 配置是否允许多环境切换：当前支持 Saved profiles、复制派生 profile、
  session API key override，并已增加 profile 级 `environment` 标签与 Providers
  面板环境过滤；workspace-scoped provider sets、并发 provider routing 和按
  endpoint/collection 自动选 provider 仍待定。
- 单 API endpoint 的 prompt mock 已可用：ResponsePane 可预览并生成单 endpoint 的
  `success / empty / error` mock，Try-it / Import report 可注入上下文；按 endpoint 保存
  自定义 prompt 模板、版本和团队共享策略仍待定。

## 6. Mock 运行时

- ✅ 静态样例的匹配规则是否允许后续按 query/header/body 条件分支：gateway
  runtime 已支持 `conditional_example_rules`，可按简单 query/header/body equality
  条件选择 `success / empty / error` 样例，并通过 config bundle / `/__albert/config`
  往返；桌面端 Mock Server Routes tab 已提供 dedicated editor，支持新增规则、多条件
  AND、顺序调整和应用到运行中网关。复杂表达式、规则模板和更细粒度持久化 UI 仍待定。
- ✅ 本地网关要不要支持导出为独立服务进程：当前已提供 `albert-cli serve`
  headless 路径和 Tauri 内嵌运行时；是否进一步打包为独立长期 daemon 仍待定。
- ✅ 请求指纹缓存是否参与网关请求时自动样例选择：当前已有 opt-in 的 Request
  cache routing。Tauri 在 start/update 时把近期缓存响应注入 gateway 内存，命中
  method/path/query/header/body 指纹时返回缓存响应；后台自动刷新、复杂优先级和
  长期 daemon 策略仍待定。
