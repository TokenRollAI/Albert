# Albert 开放问题清单

这份清单用于收敛你后续希望我持续追问的设计决策，避免问题散落在聊天记录里。

## 1. 品牌与命名

- 项目对外宣传时，是否强调 “tribute to Albert II” 作为主叙事
- README 之外，是否还需要单独的品牌使用说明

## 2. 数据模型

- 原始 OpenAPI / cURL 输入是否保留快照
- Canonical Schema 是否需要保留字段级示例与描述优先级规则

## 3. UI 演进

- 一期界面更偏控制台式工程工具，还是更偏产品化桌面应用
- 是否要在二期引入项目切换、工作区历史和导入记录

## 4. Parser 策略

- OpenAPI 3.0 和 3.1 是否同时支持
- cURL 解析的最小语法边界是什么

## 5. Provider 策略

- OpenAI Chat Completions 之外，Responses API 进入哪一个 phase
- Provider 配置是否允许多环境切换

## 6. Mock 运行时

- 静态样例的匹配规则是否允许后续按 query/header/body 条件分支
- 本地网关要不要支持导出为独立服务进程
