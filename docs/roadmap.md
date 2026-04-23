# Albert 实施路线图

## Phase 1: Foundation

目标：建立项目边界、文档系统和工作区骨架。

交付：

- 双语 README
- PRD、架构文档、路线图
- `llmdoc` 初始化
- Tauri + React + TypeScript UI 壳
- Rust workspace 与核心 crate 占位
- Canonical API Schema 基础模型
- OpenAPI / cURL / OpenAI / Storage / Gateway 的占位接口

验收标准：

- 仓库目录稳定
- 新成员可从文档进入项目
- UI 可以承载后续功能
- 各模块 extension point 已清晰暴露

## Phase 2: Parsing And Persistence

目标：打通从输入到存储的主链路。

交付：

- OpenAPI 基础解析
- cURL 基础解析
- Canonical API Schema 转换
- SQLite schema 与迁移
- 导入后在 UI 查看接口列表与详情

当前进度：

- 已完成：OpenAPI JSON/YAML 基础解析
- 已完成：常见 cURL 请求解析
- 已完成：Canonical API Schema 转换基础能力
- 已完成：SQLite 迁移、保存 collection、列出 collections/endpoints
- 已完成：Tauri parse/import/list command 接线
- 已完成：Parser 与 Storage 单元测试
- 已完成：GitHub Actions 基线校验
- 当前说明：桌面 UI 仅为占位工作台，不作为最终产品界面承诺
- 待继续：前端工作台重构与产品化交互设计
- 待继续：更完整的 OpenAPI 兼容范围

验收标准：

- 能把示例 OpenAPI / cURL 转成项目内可查询资产
- UI 能展示 endpoint 结构和样例槽位
- CI 能稳定覆盖 Rust 格式、Rust 测试、workspace 构建、前端构建和品牌资源校验

## Phase 3: Static Mock Runtime

目标：让 Albert 成为可用的静态 Mock 工具。

交付：

- 本地 Mock 网关基础监听
- RESTful 路由匹配
- `success / empty / error` 样例选择和返回
- CORS 处理
- 基础状态页与运行控制

验收标准：

- 本地端口可启动
- 典型接口请求可命中静态样例
- 浏览器环境调用正常

## Phase 4: AI-Assisted Mocking

目标：引入 OpenAI 生成能力，但保持架构可控。

交付：

- OpenAI Chat Completions 真实接线
- Prompt 构造器
- 结构化输出约束
- 基础错误处理和失败兜底

验收标准：

- 能基于 Canonical Schema 生成结构化 Mock 数据
- 失败场景具备可理解的错误反馈

## Phase 5: Intelligence And Evolution

目标：补齐高阶能力与长期维护机制。

交付：

- Responses API 兼容
- 请求指纹缓存
- AOT / JIT 策略
- 文档 diff 引擎
- 过期样例定向刷新
- 多 Provider 扩展

验收标准：

- 项目具备可持续演进的 AI 驱动 Mock 能力
- 样例和接口结构可以长期同步维护
