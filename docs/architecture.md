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
- 当前 UI 仅作为临时工作台，用于承接 Phase 2 的真实导入链路

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
- 保存项目、接口定义、标准化 schema、Mock 样例、Provider 配置

### 2.5 网关层

`crates/albert-gateway`

职责：

- 承接本地 Mock Server 的请求匹配与调度
- 一期只定义边界，不启动真实 HTTP 监听

### 2.6 AI Provider 层

`crates/albert-openai`

职责：

- 提供 OpenAI Chat Completions 接口适配器
- 为后续结构化输出和多 Provider 抽象保留兼容位

## 3. Canonical API Schema

内部数据模型不直接保存原始 OpenAPI 结构，而是做一层规范化表达。原因如下：

- cURL、OpenAPI、未来的 Postman / GraphQL 需要统一落点
- AI Prompt 和结构化输出更适合围绕 JSON Schema 风格数据组织
- 存储层不应与任意上游协议格式强耦合

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
6. UI 展示接口与样例占位

当前命令面：

- `bootstrap_summary`
- `parse_api_description`
- `import_api_description`
- `list_imported_collections`
- `list_imported_endpoints`

### 4.2 资产查看流（当前已实现）

1. 用户进入接口详情页
2. UI 读取 Canonical Endpoint 与 MockExample
3. 根据 `success / empty / error` 切换展示
4. 后续 phase 再接 AI 生成与网关回放

### 4.3 本地持久化流（当前已实现）

1. Tauri command 创建或复用 SQLite 数据库
2. 执行 migration
3. 保存 `api_collections`
4. 保存 `api_endpoints`
5. 保存请求/响应 schema 到 `api_schemas`
6. 保存默认 `success / empty / error` mock examples

## 5. 数据持久化建议

一期推荐的 SQLite 逻辑实体：

- `projects`
- `api_collections`
- `api_endpoints`
- `api_schemas`
- `mock_examples`
- `provider_configs`

建议策略：

- 原始输入可选保留一份快照，便于重新解析
- 存库主体应是标准化后的结构
- 样例与 endpoint 分离，便于后续扩展状态与版本
- 当前实现已支持 collection、endpoint、schema、example、provider config 的基础落库

## 6. UI 信息架构

- `Overview`: 当前项目状态、实现进度、后续 phase
- `Import`: OpenAPI / cURL 导入入口与规则说明
- `Endpoints`: 接口清单、方法、路径、状态
- `Endpoint Detail`: 请求结构、响应结构、Mock 样例
- `Providers`: OpenAI 配置入口
- `Server`: Mock Server 规划状态和未来运行参数

当前说明：

- 当前桌面界面应按“占位工具工作台”理解
- 现阶段更关注命令与数据链路是否成立，而不是最终 UI 形态
- 后续可以重新设计窗口结构，而不影响 parser、storage、gateway 的边界

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

- OpenAI Responses API 适配
- AI 结构化输出与修复重试
- 静态样例与动态生成结合
- 本地 HTTP 网关运行时
- 文档 diff 与样例失效刷新
- 多 Provider 兼容层
- 前端导入体验、collection 切换、详情页细化
