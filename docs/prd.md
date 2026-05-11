# Albert 核心模块 PRD

## 1. 产品定位

Albert 是一款 AI-Native API Mock 桌面客户端，服务对象是前端与客户端开发者。产品核心价值不是替代接口文档工具，而是把接口描述快速转换成可消费的 Mock 响应，并为后续 AI 驱动的动态生成能力预留结构化基础。

## 2. 目标用户

- 需要快速联调但后端接口尚未完成的前端开发者
- 需要构建本地沙箱和回归环境的客户端开发者
- 希望从 OpenAPI 或 cURL 快速落地 Mock 资产的小团队

## 3. 核心问题

- 传统 Mock 工具需要手工配置字段和假数据，成本高且枯燥
- API 描述格式分散，缺少统一内部表示
- Mock 资产难以随着接口结构演进而同步维护
- 团队往往没有一套本地优先、AI 可接入的统一 Mock 基础设施

## 4. 产品愿景

Albert 要建立一条从接口描述到结构化 Mock 资产的本地闭环：

1. 导入 OpenAPI 或 cURL
2. 解析为统一的 Canonical API Schema
3. 存入本地项目空间
4. 在桌面端进行查看、配置和样例管理
5. 后续逐步接入 OpenAI 生成能力、本地网关、缓存与版本差异管理

## 5. 品牌与命名

- 产品名为 `Albert`
- 命名灵感来自 Albert II，即第一只进入太空的猴子
- Mock 工具领域常借用 `Monkey` 语义，Albert 则把这种联想转译为更独立的产品身份
- README 需要明确说明这一点，避免命名来源在后续传播中丢失
- 当前不要求全大写或缩写化品牌写法，统一使用 `Albert`

## 6. 一期目标与当前阶段

一期是基础设施期，重点是把开发驱动文档、模块边界和工作区骨架建立起来。
这些目标已经基本交付；当前实现已推进到 Phase 4/5 的 AI-assisted mocking、
本地 Mock Server 和请求指纹缓存切片。

### 6.1 一期必须交付

- 双语 README
- PRD、架构文档、分阶段实施规划
- `llmdoc` 项目知识体系
- `Tauri + React + TypeScript` 桌面控制台基础 UI
- Rust workspace 与核心 crates 边界
- Canonical API Schema 的基础数据结构（已演进为可用 schema/validation 模型）
- OpenAPI / cURL parser 的接口定义与基础实现
- OpenAI Chat Completions provider 的接口适配器与基础运行时
- SQLite 存储层与迁移脚手架、核心实体落库
- Mock 样例模型，至少覆盖 `success`、`empty`、`error`

### 6.2 一期明确不做（历史约束）

- 请求指纹缓存
- AOT 预生成和 JIT 实时生成调度
- 多 Provider 并发支持
- Tool calling、streaming、reasoning control
- Postman Collection、GraphQL、gRPC
- 完整本地网关监听服务
- 完整的 Schema Diff Engine

说明：上面的“不做”是一期边界，不再代表当前仓库状态。当前已经具备本地 HTTP
Mock Server、多 provider 类型基础适配、Responses API 基础路径、请求指纹缓存、
Try-it 录制、手动 AI refresh 和重复导入时的 endpoint-level diff 摘要；尚未完成的是
后台自动录制/JIT、完整 Schema Diff Engine、导入版本历史、高级 provider matrix 和
团队级协作能力。

## 7. 一期功能范围

### 7.1 文档与工作区

- 明确产品边界、术语和模块职责
- 将后续开发拆成多个可执行 phase
- 建立面向长期维护的文档体系

### 7.2 桌面 UI

一期 UI 重点是结构完整，而不是功能闭环。最低覆盖：

- Dashboard / 概览页
- API 导入页
- 接口列表页
- 接口详情页
- Provider 配置页
- Mock Server 状态页

当前说明：

- 当前桌面 UI 已从占位工作台推进为可用控制台，覆盖导入、接口浏览、Mock Server
  配置、Provider profiles、AI 生成、Try-it 请求发送、真实响应保存和请求指纹缓存。
- Sidebar 已展示 imported collection 的最近更新时间和 endpoint 数；顶部
  Workspace collections 抽屉进一步集中展示当前 imported collections 的数量、
  endpoint 数、来源、更新时间、方法分布和常用 collection 操作，作为工作区导入
  记录的第一层可见性。完整项目历史、多项目切换仍属于后续产品形态。
- 它仍不是最终完整产品形态，后续重点是项目/工作区管理、自动刷新策略和更完整的
  AI/Provider 体验。

### 7.3 协议解析

- 支持 OpenAPI/Swagger 输入
- 支持 cURL 输入
- 输出统一 Canonical API Schema
- 解析实现以稳定接口和数据结构为优先，逐步扩大 OpenAPI/cURL 语法覆盖

### 7.4 存储模型

建议一期围绕以下概念建模：

- `projects`
- `api_collections`
- `api_endpoints`
- `api_schemas`
- `mock_examples`
- `provider_configs`
- `gateway_preferences`
- `gateway_scenarios`
- `request_fingerprint_cache`

当前 `api_collections` 还记录 `created_at` / `updated_at`，用于排序和 Sidebar
展示最近导入/更新信息，并驱动 Workspace collections 抽屉的导入记录卡片；
重新导入、重命名和 mock example 编辑会刷新更新时间。

重复导入同一 collection id 时，导入命令会基于旧 `raw_snapshot` 与新解析结果返回
endpoint-level diff 摘要（added / changed / removed / unchanged），changed endpoint
还会附带粗粒度原因（metadata、parameters、request body、responses、auth）和简短
明细（如参数新增/移除/修改、请求体变化、响应状态码/ schema 变化）。前端在导入成功
消息中展示摘要，并提供最近一次 Import report 抽屉查看 added / changed / removed
endpoint 明细。对仍存在的 added / changed endpoint，可直接打开接口或查看 success
prompt preview；changed 行会展示这些原因和明细，并在打开 prompt preview 时把它们
作为生成上下文 note 传入。Changed 行还可一键 Refresh success mock，复用同一变更
上下文并持久化到对应样例；报告头部也可批量 Refresh 全部可刷新 changed endpoint。
当前 diff 不落库，也不追踪完整字段级版本历史。

### 7.5 AI Provider

- 当前支持 OpenAI-compatible Chat Completions、Azure OpenAI Chat Completions、
  OpenAI Responses API 与 Azure OpenAI Responses API 基础路径。
- 当前 endpoint 级 prompt mock 已可用：用户可对单个 API endpoint 预览 prompt，并按
  `success / empty / error` 生成 mock；Try-it 和 Import report 也会把真实响应或导入
  diff 作为上下文传入。按 endpoint 保存自定义 prompt 模板/版本仍未完整实现。
- Provider profile 持久化只保存非 secret 配置；API key 通过后端环境变量或
  session-only override 提供。
- OpenAI / Azure Responses provider 已支持可选 reasoning effort 控制；默认不下发
  该字段，非空时映射为 `reasoning.effort`。
- Provider profile 已支持结构化输出修复重试次数控制；默认 2 次，允许设置
  0–5 次，其中 0 表示只返回首轮生成结果和 schema mismatch note。
- 高级能力（Azure Responses、streaming、tool calling、
  多 provider 并发策略）仍属于后续阶段。

### 7.6 Mock 样例

每个 endpoint 支持固定的样例集合：

- `success`
- `empty`
- `error`

## 8. 核心成功标准

- 新开发者在阅读 README 和 docs 后，能理解项目目标、边界和后续阶段
- 仓库具备明确可扩展的模块结构，而不是单一脚本式堆积
- UI 能作为后续功能承载壳继续演进
- Rust crate 边界能够承接后续解析、网关、存储、AI 接入实现

## 8.1 当前进度快照

- OpenAPI 与 cURL 已具备基础解析能力
- Canonical Schema 已从占位模型进入可用转换能力
- SQLite 持久化已具备最小可用实现
- Tauri 已暴露 parse/import/list/load/export/delete/rename、Mock Server、Provider、
  AI generation、Try-it validation/cache 等命令
- 前端已具备导入预览、落库回看、接口详情、Mock Server 控制台、Provider 配置、
  ResponsePane AI 生成（含当前样例上下文迭代）、Try-it 发送/录制/缓存管理能力，
  并在 Sidebar 和 Workspace collections 抽屉展示 imported collection 的最近更新时间；
  重复导入时还会显示 endpoint-level diff 摘要和最近一次 Import report 明细抽屉，
  changed endpoint 会展示 parameters / responses 等粗粒度变更原因和简短明细
- 本地 HTTP Mock Server 已具备可用运行时：路由匹配、样例选择、运行时 overrides、
  请求日志/指标、chaos、auth gates、schema enforcement、proxy upstream、scenarios
- 请求指纹缓存已具备保存/列表/Replay/Save/Remove/Clear stale/AI refresh/Prompt 预览
  能力；stale 缓存会进入 Try-it Refresh queue，集中批量 AI refresh、首个 stale
  prompt 预览和清理入口；最新 Try-it 响应也可直接作为 AI refresh / Prompt 上下文
- GitHub Actions 与本地验证命令已提供基础持续集成保护

## 9. 风险与约束

- Tauri / Rust / React 是跨技术栈项目，边界不清会迅速增加维护成本
- 过早接入复杂 AI 策略会干扰基础模型设计
- 若直接将 OpenAPI 原样入库，后续支持多格式与 AI 结构化输出会受限
- 若不保留命名来源与文档体系，项目品牌与架构会在后续迭代中失焦
