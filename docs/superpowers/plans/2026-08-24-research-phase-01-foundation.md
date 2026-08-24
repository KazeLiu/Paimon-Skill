# 研究编排阶段 1：统一基础层实现计划

> **计划状态：待用户审阅。** 执行阶段必须先完整读取本文件，再按任务顺序实现并在“执行记录”中补充命令和结果；本文件本身不授权提交、推送或发布。

## 执行目标

建立平台无关的研究请求、结果、适配器、路由、计划和执行器，用 Generic adapter 复用现有浏览器能力，并保证聊天和网页研究不回归。该阶段已经完成；后续阶段开始前应读取本文件，把这里的契约视为不可破坏的基线。

## 目标与验收标准

- `ResearchRequest`、`ResearchContext`、`ResearchArtifact`、`ResearchReport` 和 `ResearchError` 不依赖 Windows/Tauri 类型。
- `ResearchAdapter` 与 `ResearchToolExecutor` 可由桌面端和后续 Android 端分别实现。
- URL 或 `source_id` 可路由至 Generic、知乎、小红书和 B 站适配器。
- Generic adapter 执行打开、滚动、正文提取、可选评论提取和关闭。
- 正文与评论分别保存；搜索摘要不能冒充详情页证据。
- 同一研究任务显式传递 `tabId`，成功、失败和取消路径清理本轮临时窗口，不影响用户已有窗口。
- 统一 `research_source` 工具可由现有 Agent 调用。

## 范围 / 非目标

范围：研究基础模型、trait、路由、executor、Generic adapter、Tauri 桥接和 Agent 工具入口。

非目标：知乎/小红书/B站字段级提取、字幕、图片视觉、ASR、音频和 Android 平台实现。

## 文件和模块职责

- `apps/desktop/src-tauri/src/research/models.rs`：平台无关数据模型。
- `apps/desktop/src-tauri/src/research/router.rs`：URL/source 路由。
- `apps/desktop/src-tauri/src/research/planner.rs`：通用动作计划。
- `apps/desktop/src-tauri/src/research/executor.rs`：取消、错误传播、标签传递和清理契约。
- `apps/desktop/src-tauri/src/research/adapters/generic.rs`：通用网页研究。
- `apps/desktop/src-tauri/src/research/tauri_executor.rs`：桌面浏览器工具桥接。
- `apps/desktop/src-tauri/src/research/agent_tools.rs`：统一模型工具。
- `apps/desktop/src-tauri/src/agent/research.rs`：最终回答前的证据门禁。

## 任务清单

1. [x] 建立平台无关研究模型、适配器 trait 和统一入口。
2. [x] 实现 URL/source 路由和 Generic adapter。
3. [x] 用 Tauri executor 包装现有浏览器工具。
4. [x] 显式传递 `tabId`，覆盖成功、失败、取消清理。
5. [x] 接入 `research_source` 和 Agent 最终回答门禁。
6. [x] 收敛系统提示词中的平台流程描述，由代码负责强制步骤。

## 验证矩阵

- 路由：知乎、小红书、B站、Generic 和非法 URL。
- 计划：正文与评论为独立动作。
- Executor：成功关闭、失败关闭、取消关闭、同一 `tabId` 贯穿后续动作。
- 安全：只清理本轮新增临时标签；登录/验证码阻断不作为成功。
- Agent：搜索页要求详情页、详情页要求滚动、评论意图要求评论读取或明确阻断。
- 全量：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast`。
- 前端：`npm test --prefix apps/desktop -- --run`。
- 构建：`npm run build --prefix apps/desktop`。
- 格式：`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` 和 `git diff --check`。

## 当前验证基线

- Rust：97 项通过。
- 前端：67 项通过。
- 前端生产构建、Rust 格式和 diff 检查通过。
- 未提交、未推送。

## 阶段门禁

阶段 1 只有在 Rust、前端、构建、格式和 diff 检查全部通过，并确认临时标签清理不会影响用户标签后，才能作为阶段 2 的依赖。若后续阶段修改共享研究模型，必须重新执行本阶段全量回归，不得沿用旧结果。

## 风险、失败处理与回退

- 后续阶段不得把 Windows WebView2、Credential Manager 或文件路径类型写入通用研究模型。
- 扩展 `ResearchArtifact` 时必须保持正文、评论、字幕/转写和视觉结果分离。
- 平台适配失败时回退为明确 warning/error，不能静默用搜索摘要完成回答。

## 需要用户决定的问题

无。后续开发从阶段 2 开始。
