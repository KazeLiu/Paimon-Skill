# Tauri 2 桌面端交接报告

更新时间：2026-08-19  
当前分支：`codex/tauri-desktop-phase1`  
远端状态：已推送，`HEAD` 与 `origin/codex/tauri-desktop-phase1` 均为 `6b12bf2`

## 一、已完成

### 桌面应用基础

- 建立 `apps/desktop/` Tauri 2 + Vue 3 + Rust 桌面应用。
- 首页支持新建对话、历史对话、连续聊天、取消生成、模型切换。
- 模型配置支持 OpenAI-Compatible 接口，包括 DeepSeek、中转站、Ollama、LM Studio 等。
- 支持基础请求、流式输出、Tool Calling 能力测试。
- API Key 通过 Windows 系统凭据库存储，不写入 SQLite、JSON、日志或模型上下文。

### 提示词与规则

- 系统提示词由资源文件和设置组合，不硬编码到 Agent 逻辑。
- 支持编辑基础提示词、角色人设、工具策略、原神规则和自定义后缀。
- 现有 `skills/genshin-build-wiki/SKILL.md` 会在构建前同步为桌面端默认规则资源。

### Agent 与会话

- SQLite 持久化对话、消息、工具执行记录和来源记录。
- 多轮 Tool Calling，最多 4 轮；多个工具调用按顺序执行。
- 工具失败会结构化回传模型，不会直接丢失整轮回答。
- 支持模型请求和工具执行中的取消。
- 流式消息、完成、失败、取消事件按 `conversationId/runId` 过滤，避免切换对话串数据。
- 历史对话切换时恢复消息、来源和工具活动。
- 上下文窗口保留系统提示，单条消息最多 12,000 字符，总窗口最多 48,000 字符；旧历史按消息边界省略。
- 大模型输出使用安全 Markdown 渲染和危险链接过滤。

### 网页与评论

- 使用应用独立的 WebView2 Profile，不复用系统 Chrome/Edge 登录态。
- 支持打开 HTTP/HTTPS 页面、前进后退、刷新、正文提取、有限滚动和当前网页询问。
- Agent 工具：
  - `browser_open_page`
  - `browser_extract_page`
  - `browser_scroll_page`
  - `browser_click_text`
  - `browser_extract_comments`
- 只读点击会拒绝登录、发布、点赞、删除、购买、支付、下载和表单提交等副作用动作。
- 评论提取支持通用 HTML、B 站、小红书、知乎选择器，返回作者、内容、点赞和时间。
- 评论 DOM 只取可见候选节点，单条评论和总 DOM 有字符上限，遇到登录/验证码会停止。
- 浏览器工具结果会自动记录来源 URL、平台、Profile、工具参数、结果和错误。

### 隐私与发布

- 设置页增加「隐私与数据」入口。
- 清理网页登录态前需要二次确认。
- 清理命令会关闭应用内 WebView、清空内存标签，并只删除应用自己的 `app_local_data_dir/browser-profiles/default`。
- Profile ID 禁止路径穿越，删除前进行 canonical path 校验，防止 junction/symlink 越界。
- 已生成 Windows release EXE 和 NSIS 安装器。

## 二、关键提交

```text
5137dc3 docs: 补充 Tauri 桌面端目标与实施计划
02da3e5 feat: add tauri desktop assistant foundation
687b0b7 feat: persist tool runs and sources
8ceb004 feat: expose safe browser agent tools
f71ad9a feat: add bounded comment extraction adapters
e3536d9 feat: add privacy controls and release guide
6b12bf2 feat: bound long conversation context
```

所有关键步骤都在当前分支单独提交并推送，未创建额外工作树。

## 三、验证与产物

最近一次完整验证结果：

```text
npm test                         21/21 通过
npm run desktop:test             21/21 通过
cargo test                       57/57 通过
npm --prefix apps/desktop run build   通过
cargo fmt -- --check             通过
git diff --check                 通过
release EXE 启动冒烟             Responding=True
```

最新打包命令：

```powershell
npm --prefix apps/desktop run tauri build
```

产物：

- `apps/desktop/src-tauri/target/release/paimon-desktop.exe`
- `apps/desktop/src-tauri/target/release/bundle/nsis/派蒙助手_0.1.0_x64-setup.exe`

构建目录已加入 `.gitignore`，不会提交 `dist/`、`target/` 或安装器。

## 四、尚未完成与已知限制

- 尚未用真实的 B 站、小红书、知乎账号完成完整人工冒烟；当前只有本地 fixture、单元测试、构建和 EXE 启动验证。
- 社区网站的分页、评论“查看全部”自动展开、持续滚动和 selector 变化适配仍较基础。
- 尚未实现网站专用的完整搜索/打开/评论工作流，只提供当前 WebView 页面上的受限工具。
- 尚未覆盖所有网站的外部窗口、OAuth 跳转和复杂登录流程。
- 尚未提供明确的 `wait_for_stable`、任务级取消 UI 和动态页面持久化 fixture。
- 浏览器 Profile 元数据尚未进入 SQLite；当前 Profile 目录由固定 `default` 规则管理。
- providers/prompt settings 仍使用 JSON + 系统凭据库，没有统一迁移到 SQLite。
- Agent 事件目前已有消息、工具、来源和运行结束事件，但还没有完整的 `tool_progress/tool_result` 细粒度协议。
- NSIS 安装器已生成但未执行安装；当前只做了 release EXE 启动冒烟。
- Rust 仍有已知 `dead_code` 警告（Agent 返回的内部 messages/tool_runs、provider raw 响应），不影响构建。

## 五、下一台电脑继续开发的建议顺序

1. 先阅读本报告和 `docs/tauri-desktop-implementation-roadmap.md`，确认当前分支为 `codex/tauri-desktop-phase1`。
2. 运行 `npm test`、`npm run desktop:test`、`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`，确认环境基线。
3. 增加动态页面本地 fixture，覆盖懒加载、评论展开、登录挑战、取消和 selector 失效降级。
4. 将浏览器工具从“当前页面操作”扩展为可控的站点工作流，并保持每个站点适配器独立、可禁用、可测试。
5. 增加完整的来源/工具活动面板，支持查看参数、失败原因和截断提示。
6. 增加 Profile 清理、SQLite 清理、配置导入导出和脱敏诊断功能的测试。
7. 真实登录后进行 B 站、小红书、知乎人工冒烟，再重新构建 NSIS。

## 六、工作区注意事项

当前 `git status` 中仍有若干桌面端之外的既有修改，包括插件元数据、根 `AGENTS.md`、README、skill 文档和根测试。它们没有被本阶段提交，不要使用 `git reset --hard` 或整文件回滚。继续开发时只暂存明确属于桌面端任务的文件。
