# 研究任务编排层总计划

> **状态：实现进行中。** 用户已确认范围；阶段 5 为取消/不实施，仅完成审计，不进入生产编排。

## 目标与验收标准

- 由 Rust 编排层强制搜索页到详情页、滚动加载、按需评论、临时网页清理和来源分级。
- 知乎、小红书和 B站均读取具体详情，不以搜索摘要冒充证据。
- B站优先读取页面实际公开的普通字幕和 AI 字幕，统一生成带分段和时间戳的 Transcript artifact。
- B站没有可读取字幕时固定降级为：“当前视频未取得可读取字幕，已跳过音频转写。”
- 只有用户询问画面、面板、属性、伤害、配装或操作时，才抽取有限关键帧并调用现有视觉任务抽象。
- 不实现 ASR、在线语音转写、音频下载、音频抽取或本地语音模型。
- 不修改模型配置、对话界面、信息源页面或其他无关功能。

## 阶段顺序

1. 阶段 1：统一研究模型、adapter、router、executor、Agent 工具和证据门禁。
2. 阶段 2：知乎详情、展开、字段、评论和阻断降级。
3. 阶段 3：小红书详情、轮播图片、按需视觉和隐私边界。
4. 阶段 4：B站详情、当前分P/多分P、普通字幕/AI字幕发现与 Transcript。
5. 阶段 5：取消/不实施；只审计既有 ASR 文件并保证未接入生产研究流程。
6. 阶段 6：B站字幕增强与按需关键帧视觉分析，仅依赖阶段 4 和阶段 3。

阶段 1–3 已有代码仍须按各自 plan 重新验证。阶段 4 通过后才能开始阶段 6；阶段 5 不创建实现目标，也不是任何阶段的依赖。

## 共享约束

- 平台无关模型不得依赖 Windows WebView2、Tauri 窗口或 Android SDK 类型。
- Body、Comments、Transcript、Visual 分开存储，不能相互覆盖。
- 普通字幕与 AI 字幕只有在当前页面或公开响应的实际字段得到验证后才能进入成功路径，不猜测 endpoint 或字段。
- 当前分P默认只读当前分P；用户明确询问全集时才逐P读取，并保留每个分P的标识和时间线边界。
- Cookie、Authorization、签名 URL、短期令牌和本地绝对路径不得进入日志或模型上下文。
- 成功、失败、取消和超时均关闭本轮临时网页并清理本轮关键帧资源，不影响用户已有窗口。
- 保留当前脏工作区，不执行 reset、checkout、强制删除或覆盖他人改动。

## ASR 改动审计

- 保留既有 `apps/desktop/src-tauri/src/media/` 和 `docs/research/asr-provider-matrix.md`，不直接删除或覆盖。
- 审计其模块声明、调用点和依赖，记录哪些只服务于 ASR。
- 从当前目标、Agent 工具、B站 adapter、planner 和生产调用链中移除 ASR/音频引用；既有未启用文件可继续保留。
- 不新增 ASR provider、音频缓存、音频依赖、凭据 UI 或转写设置。

## 分阶段验证

每阶段记录实际修改文件、定向测试、失败路径、未完成项和停止原因。最终统一运行：

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast
npm test --prefix apps/desktop -- --run
npm run build --prefix apps/desktop
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
git diff --check
```

## 完成与停止条件

- 只有阶段 1–4、6 的逐项验收与最终回归都有当前证据，才能完成总目标。
- 阶段 5 只标记“取消/不实施”，不能标记为 ASR 已完成。
- 若普通字幕或 AI 字幕数据未从实际页面验证，停止该成功路径并保留固定降级。
- 若关键帧只能通过绕过登录、风控、DRM 或付费限制取得，停止关键帧流程并明确降级。

## 实际完成状态（2026-08-24）

- 阶段 1–3：已实现并纳入 Rust 全量回归；阶段 3 的在线视觉 executor 已接入当前会话模型，仍按画面意图触发。
- 阶段 4：已完成 B站详情 URL、当前分P/全集字幕读取、普通/AI字幕统一 Transcript、时间戳/长度上限、固定无字幕降级、评论分离和关闭网页。
- 阶段 5：取消/不实施，审计完成。`src/media/` 与 `docs/research/asr-provider-matrix.md` 保留为未启用记录；生产 `research`/B站/Agent 调用链未挂载 ASR 或音频动作。
- 阶段 6：已完成关键帧脚本、内部研究动作、data URL 安全边界、视觉 executor 接线、B站按需视觉和 Visual artifact 分离；普通总结不抽帧、不调用视觉。

## 实际验证记录

- 阶段 4 定向：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::adapters::bilibili --no-fail-fast`，10 passed。
- 阶段 6 定向：B站同上 10 passed；`research::model_tasks` 覆盖 data URL、图片大小、敏感引用和视觉失败边界；关键帧脚本定向 2 passed。
- Rust 全量：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast`，136 passed。
- 前端：`npm test --prefix apps/desktop -- --run`，11 个测试文件、67 passed。
- 生产构建：`npm run build --prefix apps/desktop`，Vite 构建成功。
- 格式与差异：`cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check` 通过；`git diff --check` 通过。
- 当前工作区最终回归已完成；剩余 Rust warning 仅是未启用模型类型、关键帧请求和 B站兼容字段的 dead-code 提示，不影响测试或生产路径。
