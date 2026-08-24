# 研究编排阶段 4：B站详情与字幕实现计划

> **状态：已完成（2026-08-24）。** 字幕协议已用真实 B站页面与公开响应验证；实现只读取已验证字段，未知形态固定降级。

## 目标与验收标准

- 仅接受合法 BV/AV 详情 URL；搜索页、首页和非法 URL 不能作为最终证据。
- 提取 BV/AV ID、标题、UP 主、简介、发布时间、互动数据、分P列表和当前分P。
- 从页面实际公开 DOM、播放器状态或已验证公开响应中发现普通字幕和 AI 字幕。
- 普通字幕和 AI 字幕统一为 Transcript artifact；metadata 保留 `subtitleType`、语言、分P索引/标识和来源。
- 字幕段按开始时间排序，校验起止时间，限制总段数、单段字符和总字符。
- 当前分P默认优先；只有用户明确询问全集时才逐P读取，并保留分P边界。
- 无字幕、不可读、登录阻断或解析失败时精确包含固定 warning：“当前视频未取得可读取字幕，已跳过音频转写。”
- Body、Transcript、Comments 独立；成功、失败和取消均关闭本轮临时网页。
- 全部路径不下载/抽取音频，不调用 ASR 或本地语音模型。

## 范围 / 非目标

范围：详情 URL、视频元数据、分P、普通/AI字幕发现与解析、Transcript、评论分离、失败降级和清理。

非目标：ASR、在线语音转写、音频下载/抽取、关键帧和视觉分析（属于阶段 6）、绕过登录/风控/DRM/付费限制。

## 文件职责

- `research/adapters/bilibili.rs`：URL、详情字段、字幕候选、分P策略和报告组装。
- `research/models.rs`：平台无关 Transcript artifact 与必要 metadata。
- `browser/scripts.rs`：有界读取页面公开播放器状态和字幕候选。
- `research/tauri_executor.rs`：通用网页动作；不放 B站业务判断。
- `tools/comments.rs`：按需读取 B站评论，保持通用契约。

## 任务清单

1. 为 BV、AV、短链重定向、搜索、首页和非法 URL 建立测试。
2. 定义视频、分P、字幕轨、字幕类型和字幕分段模型及硬上限。
3. 用真实 B站详情页/公开响应验证普通字幕和 AI 字幕字段、可读取条件及失败形态，保存脱敏 fixture；不得以记忆实现 endpoint。
4. 实现详情字段、当前分P和分P列表提取。
5. 实现字幕候选发现、普通/AI字幕统一解析、时间戳排序与 Transcript artifact。
6. 实现当前分P和“全集请求”多分P策略；单P失败只产生 warning，不污染其他成功分P。
7. 实现无字幕/不可读的固定降级；确认没有音频或 ASR 调用点。
8. 按需提取评论；覆盖登录、验证码、页面变化、字幕解析错误、分P失败、取消和关闭失败。
9. 执行定向测试和全量回归，记录实际结果。

## 验证矩阵

- URL：BV/AV 通过；搜索/首页拒绝；重定向后校验最终 URL。
- 字段：fixture 覆盖 ID、标题、UP 主、简介、时间、互动、分P列表和当前分P。
- 字幕发现：普通字幕和 AI 字幕各有来自真实页面/公开数据的 fixture；未经验证字段不得成功。
- Transcript：分段有序、时间戳有效、长度受限，metadata 保留字幕类型、语言和分P。
- 分P：普通请求只读当前分P；全集请求逐P读取且不混淆来源。
- 降级：缺失、阻断、解析失败均包含固定 warning，不生成伪 Transcript。
- 评论：只在 `wants_comments` 时读取并独立保存。
- 边界：代码与测试证明所有路径都无音频/ASR动作。
- 清理：成功、失败和取消关闭本轮临时网页。

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::adapters::bilibili --no-fail-fast
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast
npm test --prefix apps/desktop -- --run
npm run build --prefix apps/desktop
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
git diff --check
```

## 完成与停止条件

- 详情、分P、普通/AI字幕、Transcript、固定降级、评论分离和清理均有测试证据后才能完成。
- 普通或 AI 字幕协议无法从真实页面验证时，停止对应成功路径，只保留固定降级。
- 发现需要音频、ASR、抓包或绕过限制时立即停止，不扩大范围。

## 实际完成与验证

- `browser/scripts.rs` 在页面上下文读取 `__INITIAL_STATE__.videoData`、当前/全集 `pages`，调用已验证的 `/x/player/v2` 和公开字幕 JSON；不输出 Cookie、字幕签名 URL或本地路径。
- 普通字幕与 AI 字幕按真实 `lan`、`lan_doc`、`type` 字段分类，中文/普通字幕优先；每个分P最多选一条轨道，最多 4000 段/轨、单段 2000 字、Transcript 总计 60000 字。
- 普通请求只读当前分P；查询命中“全集/全部分P/所有分P/每个分P/各分P/整套/全套”时最多读取 12 个分P，并保留 P 序号、标题、字幕类型和时间戳。
- 无字幕、登录阻断、字段变化、响应过大或解析失败均保留正文并追加固定降级：“当前视频未取得可读取字幕，已跳过音频转写。”
- 成功、字幕失败、评论失败和取消均通过 adapter 关闭临时网页。
- 定向测试：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::adapters::bilibili --no-fail-fast`，10 passed；Rust 全量在当前工作区 134 passed。

## 未完成项

- 最终回归已完成：Rust 136 passed、前端 67 passed、生产构建通过、fmt-check 和 diff-check 通过。
