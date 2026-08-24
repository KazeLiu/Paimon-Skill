# 研究编排阶段 6：B站字幕增强与按需关键帧视觉分析实现计划

> **状态：已完成（2026-08-24）。** 本阶段名称保留历史文件名，但业务含义为“B站字幕增强与按需关键帧视觉分析”，不依赖阶段 5。

## 目标与验收标准

- 复用阶段 4 已验证的普通字幕/AI字幕发现与解析，增强多分P和时间线选择。
- 普通字幕和 AI 字幕统一生成 Transcript artifact，保留字幕类型、语言、分P与时间戳。
- 无字幕或不可读取时固定降级：“当前视频未取得可读取字幕，已跳过音频转写。”
- 用户未询问画面、面板、属性、伤害、配装或操作时，关键帧和视觉调用次数为 0。
- 用户需要画面时，依据问题和字幕时间线选择有限关键帧，调用现有视觉 executor，独立生成 Visual artifact。
- Body、Transcript、Visual、Comments 互不覆盖，传给总结模型的内容有界且脱敏。
- 所有路径不下载/抽取音频，不调用 ASR，不创建音频临时文件。
- 成功、失败、取消和超时均关闭本轮网页并清理关键帧临时资源。

## 范围 / 非目标

范围：字幕增强、多分P时间线、画面意图、有限关键帧、现有在线视觉抽象、artifact 分离、隐私和清理。

非目标：ASR、语音转写、音频下载/抽取、本地语音或视觉模型、整视频下载、绕过登录/风控/DRM/付费限制。

## 文件职责

- `research/adapters/bilibili.rs`：字幕增强、分P与视觉决策、报告组装。
- `research/media.rs`：有限关键帧请求和非敏感临时资源句柄；不得含音频类型。
- `research/model_tasks.rs`：复用视觉 executor、图片限制和敏感引用过滤。
- `research/tauri_executor.rs` / `browser/scripts.rs`：通用页面公开数据、关键帧候选和清理动作。
- `diagnostics.rs`：确保敏感 URL、本地路径和凭据不进入导出日志。

## 任务清单

1. 写决策表：普通字幕、AI字幕、无字幕、当前分P/全集、视觉意图、评论意图及组合。
2. 复用阶段 4 Transcript，增强多分P合并上限、时间线和来源 metadata。
3. 定义画面意图和关键帧时间选择；普通总结时整个视觉链路为 0 次调用。
4. 定义关键帧数量、像素/字节、时间范围和总上下文硬上限。
5. 从页面可合法访问的画面/已验证帧候选中生成临时关键帧；不得持久化签名 URL或本地路径。
6. 调用阶段 3 视觉 executor，生成独立 Visual artifact；视觉不可用时保留正文/字幕并 warning。
7. 为成功、关键帧失败、视觉失败、取消和超时实现网页与临时资源清理。
8. 审计所有生产路径，证明不存在 ASR/音频动作。
9. 运行阶段 6 定向测试和最终全量回归，记录实际结果。

## 验证矩阵

- 字幕：普通/AI字幕均产生 Transcript；分段有序有界，metadata 保留字幕类型、语言和分P。
- 分P：普通请求只处理当前分P；全集请求逐P处理并保持来源/时间线边界。
- 无字幕：固定降级且不生成伪 Transcript。
- 视觉条件：普通总结为 0 次；画面类问题才请求有限关键帧和视觉。
- Artifact：Body、Transcript、Visual、Comments 互不覆盖。
- 清理：成功、帧失败、视觉失败、取消、超时后网页关闭且临时关键帧不存在。
- 隐私：日志/模型请求不含 Cookie、Authorization、完整签名 URL或本地绝对路径。
- 音频边界：所有路径无音频请求、文件或 ASR 调用。
- 并发：两个研究任务的标签、取消 token 和临时关键帧相互隔离。

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::adapters::bilibili --no-fail-fast
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib research::model_tasks --no-fail-fast
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib diagnostics:: --no-fail-fast
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast
npm test --prefix apps/desktop -- --run
npm run build --prefix apps/desktop
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
git diff --check
```

## 启动、完成与停止条件

- 启动条件：阶段 4 已提供可验证普通/AI字幕、Transcript 或固定降级；阶段 3 的视觉 executor 契约稳定。
- 完成条件：字幕增强、按需关键帧、Visual 分离、隐私、无音频/ASR和全部清理路径均有自动测试证据。
- 若字幕或关键帧只能通过猜测接口、抓包或绕过限制取得，停止对应成功路径并明确降级。
- 发现任何 ASR/音频实现需求时立即停止，遵循阶段 5 的取消规则。

## 实际完成与验证

- 新增 `ExtractBilibiliKeyframes` 内部研究动作；页面脚本最多 6 个时间点、最大约 960x540、JPEG 内存 data URL，恢复原播放状态，不访问/下载音频。
- B站 adapter 只在 `wantsVisual` 或问题命中画面/面板/属性/伤害/配装/操作语义时抽取关键帧；普通总结的抽帧和视觉调用次数为 0。
- 字幕时间线优先提供关键帧时间点；无字幕时只有明确视觉意图才按视频时长做少量均匀采样，普通总结仍不调用视觉。
- `OpenAiCompatibleVisualTaskExecutor` 复用当前会话 provider/API key，支持受限 JPEG/PNG/WebP data URL；解码后不超过 8 MiB，拒绝敏感引用、私网、凭据和不支持 MIME。
- Visual 与 Body、Transcript、Comments 独立保存；视觉失败保留正文/字幕并 warning。所有 adapter 成功、失败、取消路径关闭研究网页，关键帧不落盘。
- 阶段 6 定向：B站 adapter 10 passed，model_tasks 覆盖视觉/data URL 边界，关键帧脚本 2 passed；Rust 全量 136 passed。
- 前端 67 passed、生产构建、fmt-check 和 diff-check 均通过。

## 未完成项

- 最终回归命令均已执行；只剩未启用代码的 dead-code warning，不新增 ASR 或音频生产动作。
