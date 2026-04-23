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

## 6. 一期目标

一期是基础设施期，重点是把开发驱动文档、模块边界和工作区骨架建立起来。

### 6.1 一期必须交付

- 双语 README
- PRD、架构文档、分阶段实施规划
- `llmdoc` 项目知识体系
- `Tauri + React + TypeScript` 桌面控制台基础 UI
- Rust workspace 与核心 crates 边界
- Canonical API Schema 的基础数据结构
- OpenAPI / cURL parser 的接口定义与占位实现
- OpenAI Chat Completions provider 的接口适配器占位
- SQLite 存储层与迁移脚手架占位
- Mock 样例模型，至少覆盖 `success`、`empty`、`error`

### 6.2 一期明确不做

- 请求指纹缓存
- AOT 预生成和 JIT 实时生成调度
- 多 Provider 并发支持
- Tool calling、streaming、reasoning control
- Postman Collection、GraphQL、gRPC
- 完整本地网关监听服务
- 完整的 Schema Diff Engine

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

### 7.3 协议解析

- 支持 OpenAPI/Swagger 输入
- 支持 cURL 输入
- 输出统一 Canonical API Schema
- 解析实现可保留为占位或基础骨架，但接口和数据结构必须稳定

### 7.4 存储模型

建议一期围绕以下概念建模：

- `projects`
- `api_collections`
- `api_endpoints`
- `api_schemas`
- `mock_examples`
- `provider_configs`

### 7.5 AI Provider

- 一期仅支持 OpenAI
- 优先支持 `Chat Completions`
- `Responses API` 在文档中保留演进位，但代码可先不接
- 一期不接复杂高级能力，只定义统一适配接口

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

## 9. 风险与约束

- Tauri / Rust / React 是跨技术栈项目，边界不清会迅速增加维护成本
- 过早接入复杂 AI 策略会干扰基础模型设计
- 若直接将 OpenAPI 原样入库，后续支持多格式与 AI 结构化输出会受限
- 若不保留命名来源与文档体系，项目品牌与架构会在后续迭代中失焦
