# 研究编排阶段 5：取消在线 ASR（不实施）

> **状态：取消/不实施。** 本文件只作为审计记录；不创建阶段实现目标，不新增依赖，不接入生产编排。

## 取消结论

当前 EXE 研究功能不实现 ASR、在线语音转写、音频下载、音频抽取、Whisper、faster-whisper 或其他本地语音模型。B站无字幕时统一返回：“当前视频未取得可读取字幕，已跳过音频转写。”

## 既有改动审计范围

- 保留 `apps/desktop/src-tauri/src/media/` 和 `docs/research/asr-provider-matrix.md`，不直接删除或覆盖。
- 盘点模块声明、调用点、依赖和测试，记录哪些仅服务于 ASR。
- 从当前研究 Agent、B站 adapter、planner、设置和生产调用链中移除 ASR/音频引用。
- 未被生产引用的既有文件可保留为未启用代码；不得继续扩展 provider、签名、凭据、UI 或测试转写。

## 任务清单（只审计）

1. 搜索 ASR/音频模块的生产引用和 Cargo 依赖。
2. 确认阶段 4/6 的有字幕和无字幕路径都不引用这些模块。
3. 若存在生产引用，做最小解耦；不删除文件、不修改无关 UI。
4. 在阶段 4/6 执行记录中写明审计结果和未启用状态。

## 验收与测试

- `research`、B站 adapter、Agent 工具和生产命令中不存在 ASR/音频调用链。
- 无字幕测试精确验证固定降级语句且不生成 Transcript。
- Rust 全量、前端测试、生产构建、fmt 和 diff-check 通过。

```powershell
rg -n "TranscriptionProvider|transcribe|AudioInput|audio" apps/desktop/src-tauri/src
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast
npm test --prefix apps/desktop -- --run
npm run build --prefix apps/desktop
cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check
git diff --check
```

## 停止条件

- 阶段 5 永远不标记为“ASR 实现完成”，只标记“取消/不实施，审计完成”。
- 任何新增 ASR、音频或厂商接口代码都属于范围违规，应停止并撤销该新增部分，但不得回滚他人无关改动。
- 如果 ASR 文件与非 ASR 功能强耦合且无法安全解耦，记录具体调用链并暂停该点，不扩张到模型配置或对话 UI。

## 实际审计结果（2026-08-24）

- `apps/desktop/src-tauri/src/media/` 及 `docs/research/asr-provider-matrix.md` 仅包含未启用的 ASR 契约、reserved provider 和文档记录，按要求保留，未删除、未扩展。
- `lib.rs` 未声明 `mod media`；`research`、B站 adapter、Agent 工具、planner、`run_chat` 和生产 Cargo 依赖没有引用 ASR provider、音频下载、音频文件或转写调用。
- `research/media.rs` 原有 `AudioSource` 已移除，只保留非音频的 `KeyFrameRequest`；阶段 4/6 关键帧脚本只从页面 `video` 画布生成内存 data URL。
- 无字幕固定降级测试与关键帧脚本边界测试证明不会转入音频或 ASR 路径。
- 阶段 5 结论：取消/不实施，审计完成；不得将其标记为“ASR 已完成”。

## 验证记录

- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib --no-fail-fast`：136 passed。
- 前端 67 passed、生产构建、fmt-check 和 diff-check 均通过。
- `rg` 审计确认生产研究路径无 `transcribe`、`TranscriptionProvider`、`AudioInput` 或 ASR provider 调用；仅保留诊断脱敏测试和未挂载 `src/media/`。
