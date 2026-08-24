# 研究编排阶段 2：知乎适配器实现计划

> **计划状态：待用户审阅。** 执行阶段必须先完整读取本文件；每项任务完成后记录测试命令、结果和未完成原因。

## 执行目标

将知乎从 Generic 包装器升级为字段级适配器，由代码确保只研究具体问题、回答或文章详情，展开被折叠的正文，并把正文和评论独立保存；登录、验证码或风控必须明确降级。

## 目标与验收标准

- 搜索页、首页和非详情 URL 只能用于发现候选，不能生成成功报告。
- 支持知乎问题、回答和专栏文章详情 URL；规范化时不丢失决定具体回答的路径信息。
- 使用语义 DOM 查找并点击“阅读全文/展开”等只读控件，禁止副作用操作。
- 结构化提取标题、正文、作者、发布时间/更新时间和可见互动数据；缺失字段以 warning 表示，不猜测。
- 用户需要评论、讨论、口碑或反馈时读取评论；正文和评论分别写入 artifact。
- 页面含重要图片、表格或图表时只记录视觉任务候选，本阶段不接通在线视觉模型。
- 登录、验证码、空正文、页面结构变化和取消都返回可区分错误，并关闭本轮窗口。

## 范围 / 非目标

范围：知乎 URL 分类、展开动作、字段提取、评论分离、阻断降级和测试夹具。

非目标：绕过登录/验证码、写操作、知乎全站爬取、通用 OCR/视觉模型、小红书和 B站流程。

## 现状与关键约束

- `ZhihuAdapter` 当前只是 `GenericAdapter` 的薄包装。
- 已有语义 DOM、`browser_find_context`、`browser_click_element`、滚动和评论提取能力。
- `elementId` 只在当前快照代次有效，滚动/导航后必须重新定位。
- 适配规则应位于 Rust adapter/planner/extractor，不写回系统提示词。

## 文件和模块职责

- `research/adapters/zhihu.rs`：知乎编排与报告组装。
- `research/planner.rs` 或 `research/adapters/zhihu/*`：知乎动作计划和详情 URL 校验。
- `research/models.rs`：仅在确有共享价值时增加结构化 metadata，不加入平台 SDK 类型。
- `browser/scripts.rs`：必要的只读 DOM 字段提取脚本；脚本输出必须有长度上限。
- `browser/agent_tools.rs` / `research/tauri_executor.rs`：增加通用查找、点击或结构化提取动作，不写知乎业务判断。
- `tools/comments.rs`：只在现有知乎评论适配器确有缺口时局部增强。

## 任务清单

1. 为问题、回答、专栏和搜索/首页 URL 写分类测试，再实现详情 URL 校验。
2. 定义知乎正文 metadata 契约：作者、发布时间、更新时间、互动数据和页面类型。
3. 在 adapter 内实现“读取快照 → 查找展开控件 → 安全点击 → 重新读取 → 有限滚动 → 提取正文”。
4. 将评论作为独立 `Comments` artifact；只在 `wants_comments` 时执行。
5. 将重要图片/表格/图表标为视觉候选 warning/metadata，不在本阶段调用视觉模型。
6. 覆盖登录、验证码、空正文、控件失效、结构变化、取消和关闭失败路径。
7. 运行定向测试、Rust 全量测试、前端测试、构建和格式检查。

## 验证矩阵

- URL：问题、具体回答、专栏文章通过；搜索、首页、非法 URL 被拒绝或标为 discovery。
- 展开：有“阅读全文”时点击后重新提取；没有时不点击；旧 `elementId` 失败后重新扫描一次而非循环。
- 字段：标题、正文、作者、时间、互动数据从固定 HTML/语义 DOM fixture 提取。
- 分离：正文不含评论正文，评论 artifact 不覆盖正文 artifact。
- 条件：`wants_comments=false` 不调用评论动作；`true` 时调用一次并处理阻断。
- 安全：登录/验证码返回 `blocked`；不点击关注、赞同、发布等副作用控件。
- 清理：成功、失败、取消均关闭本轮 `tabId`。
- 回归命令：
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::adapters::zhihu --no-fail-fast`
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast`
  - `npm test --prefix apps/desktop -- --run`
  - `npm run build --prefix apps/desktop`
  - `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`
  - `git diff --check`

## 完成标准和停止条件

- 只有 fixture 和真实运行契约均证明具体详情、展开、字段提取、评论分离和阻断降级后才能完成阶段 2。
- 若真实 DOM 无法稳定区分某字段，保留缺失 warning，不使用脆弱的序号/位置猜测。
- 若必须扩大到登录自动化或视觉模型，停止并留给对应后续阶段。

## 当前实现备注

当前工作区已有知乎适配器和定向测试实现，但尚未完成本阶段要求的全量 Rust、前端测试和生产构建。因此在目标执行时仍需按验证矩阵重新验收，不能仅依据已有定向测试标记阶段完成。

## 风险、失败处理与回退

- 知乎 DOM/A-B 测试会变化：优先语义标签和结构，多选择器仅作受测 fallback。
- 展开操作可能触发登录：记录阻断并降级，不能反复点击。
- 可回退到 Generic adapter，但报告必须注明未取得知乎结构化字段。

## 需要用户决定的问题

计划审阅阶段无必须决定项；视觉候选是否默认触发由阶段 3 的共享视觉契约统一确定。
