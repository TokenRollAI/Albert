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
- Tauri 命令 `generate_mock_example` 支持会话级别 API key override、
  `persist=true` 时 `SqliteStore::replace_mock_example` 同步更新样例与快照。
- 前端 ResponsePane 提供「Generate <kind>」按钮和复制负载、note 展示。

待继续：

- 结构化输出校验（按 Canonical Schema 验证生成结果，失败时重试/修复）。
- 多 Provider 切换 + 环境变量自检。
- Responses API 变体。

## Phase 5: Intelligence And Evolution

目标：补齐高阶能力与长期维护机制。

交付：

- Responses API 兼容
- 请求指纹缓存
- AOT / JIT 策略
- 文档 diff 引擎
- 过期样例定向刷新
- 多 Provider 扩展
- 录制实际响应 → 生成真实样例
- 延迟/错误率注入
