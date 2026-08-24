# 在线 ASR 供应商官方文档核对矩阵

查询日期：2026-08-24（Asia/Shanghai）

本表只记录厂商官方文档。能力列中的“已确认”表示该页面明确写出能力；“未确认”表示本次核对没有足够证据，不代表厂商一定不支持。文档中的示例凭据、签名值和带令牌 URL 均未复制到仓库。

## 总览

| Provider | 官方接口（文档标题） | 普通话 | 长音频/异步 | 上传/来源 | 格式、大小、时长 | 时间戳/热词 | 鉴权 | 免费额度 | Android / 端侧 | 自有服务端签名结论 | 当前实现状态 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 腾讯云 | [录音文件识别请求](https://cloud.tencent.com/document/product/1093/37823)；[录音文件识别结果查询](https://cloud.tencent.com/document/product/1093/37822) | 已确认：`16k_zh` 普通话通用引擎 | 已确认：异步任务；支持回调或轮询，URL 音频最长 5 小时 | 已确认：公网 URL 或 POST 本地文件 | 已确认：支持 wav/mp3/m4a/flv/mp4/wma/3gp/amr/aac/ogg-opus/flac；URL <= 1 GB、<= 5 小时；本地文件 <= 5 MB | 已确认：`ResTextFormat` 提供句/词级时间戳；`HotwordId` / `HotwordList` | 已确认：腾讯云 API 3.0 签名（TC3-HMAC-SHA256），`SecretId`/`SecretKey` 等公共参数 | 已确认存在按月免费资源包；具体额度随产品/账户和计费页面变化 | [语音识别 SDK 概览](https://cloud.tencent.com/document/product/1093/37138) 列出 Android 实时/一句话/极速版 SDK；本次未确认录音文件异步接口的 Android 直连安全建议 | 官方公共参数要求 v3 签名；长期 `SecretKey` 不应进入 APK，桌面直连也需凭据隔离 | `reserved`：统一模型已接入，TC3 实现待审计依赖与 mock contract |
| 阿里云 | [录音文件识别](https://help.aliyun.com/zh/isi/developer-reference/recording-file-recognition) | 已确认：项目模型按场景配置，录音文件识别支持中文场景 | 已确认：轮询或回调；文件最长 12 小时 | 已确认：`file_link` 远程文件 URL | 已确认：WAV/MP3/MP4/M4A/WMA/AAC/OGG/AMR/FLAC；音频 <= 512 MB、视频 <= 2 GB、总时长 <= 12 小时 | 已确认：词信息、时间戳校准、泛/类热词；`enable_words` 与 4.0 版本相关 | 已确认：AppKey + Token；Token 可由 SDK 或公共 API 获取；官方明确不建议移动端保存 AccessKey Secret | 已确认：新用户试用期 3 个月，每 24 小时免费识别 2 小时文件（页面当前说明） | [Android SDK](https://help.aliyun.com/zh/isi/developer-reference/android-sdk)；[通过 SDK 获取 Token](https://help.aliyun.com/zh/isi/getting-started/obtain-an-access-token) 明确建议移动端通过自有服务端下发 Token | 官方明确建议 AccessKey ID/Secret 放服务端，移动端使用服务端 Token；当前不在客户端实现 AK/SK 签名 | `reserved`：已确认能力与安全边界，POP/Token 调用链未实现 |
| 火山引擎 | [录音文件识别标准版 HTTP](https://www.volcengine.com/docs/6561/1354868) | 已确认：默认支持中文普通话，语言可指定 `zh-CN` | 已确认：`submit` + `query` 两阶段异步流程 | 已确认：提交音频 URL；官方页面未确认本地 multipart 上传 | 已确认：raw/wav/mp3/ogg；错误码页面确认音频大小限制 `<512M`；完整时长上限本次未确认 | 已确认：utterance/分句时间信息；`context` 可传热词，亦有 boosting table | 已确认：新版 `X-Api-Key`；旧版 `X-Api-App-Key` + `X-Api-Access-Key`，以及 Resource/Request/Sequence headers | 本次官方计费页只确认按时长/token/QPM 等计费，未确认该异步接口的固定免费额度 | 本次未找到足以证明该录音文件 HTTP 接口 Android 直连安全边界的官方页面；应走自有服务端或短期凭据 | 官方协议使用 API Key/Access Key；长期凭据不应写入 APK，移动端应由自有服务端代理或发放短期凭据 | `reserved`：已确认 HTTP 字段与两阶段流程，配置/结果适配未实现 |
| 自有中转 | 项目内部统一契约（无厂商文档） | 由中转后端决定 | 由中转后端决定 | 当前契约只接受无签名 HTTPS 公网 URL | 由中转后端决定；客户端响应上限 2 MiB | 统一结果模型支持时间戳、可选置信度、热词字段 | 仅保存 keyring 引用；请求使用 Bearer，但令牌不进入日志/模型上下文 | 由部署方决定 | 可作为 Android 默认边界；APK 不保存长期厂商密钥 | endpoint 必须 HTTPS（仅 loopback 开发例外），凭据由自有服务端持有 | `available` 仅表示契约客户端已实现；未配置 endpoint 时仍返回明确错误 |

## 腾讯云 TC3 实现门禁

腾讯云官方页面已经足以确定协议形状：

1. `CreateRecTask` 以公共 API 3.0 参数提交，接口域名为 `asr.tencentcloudapi.com`，查询接口使用任务 ID。
2. 公共参数页面规定 TC3-HMAC-SHA256；签名需要规范请求、规范请求哈希、待签名字符串、派生签名密钥和 `Authorization` 头。
3. `ResTextFormat=3` 适合字幕场景，返回词粒度时间戳；`HotwordId` / `HotwordList` 支持热词。
4. 轮询结果有等待、执行中、成功、失败状态；查询页明确要求用 `TaskId` 查询。

本阶段没有在 `Cargo.toml` 增加密码学依赖，也没有把签名算法手写进业务代码。因此腾讯 provider 保持 `reserved`，避免出现“能编译但签名不符合官方规范”的假实现。真正接通前必须：

- 由主线确认并审计 `sha2` + `hmac`（或等价成熟库）依赖；
- 使用官方固定样例在本地 mock server 覆盖 canonical request、签名、提交、轮询、错误映射、超时和取消；
- 只从 keyring 读取 `SecretId` / `SecretKey`，不让它们进入配置 JSON、诊断日志、研究 artifact 或模型上下文；
- 对返回的 `RequestId` / `TaskId` 做长度与字符白名单限制。

## 证据明细

### 腾讯云

- [语音识别 录音文件识别请求](https://cloud.tencent.com/document/product/1093/37823)：接口域名、异步流程、格式、URL/本地提交、大小/时长、`ResTextFormat`、热词和 v3 签名引用。
- [语音识别 录音文件识别结果查询](https://cloud.tencent.com/document/product/1093/37822)：轮询状态、`TaskId`、结果中的句/词时间信息及错误状态。
- [语音识别 SDK 概览](https://cloud.tencent.com/document/product/1093/37138)：SDK 平台列表，包含 Android 的实时/一句话/极速版入口；不据此推断录音文件异步接口可安全端侧直连。
- [语音识别计费概述](https://cloud.tencent.com/document/product/1093/35686)：免费资源包、扣费顺序和后付费开关说明。

### 阿里云

- [录音文件识别](https://help.aliyun.com/zh/isi/developer-reference/recording-file-recognition)：格式、大小、时长、轮询/回调、试用额度、词信息、时间戳校准和热词。
- [Android SDK](https://help.aliyun.com/zh/isi/developer-reference/android-sdk)：Android SDK 的音频格式、Token 与时间戳回调信息。
- [通过 SDK 获取 Token](https://help.aliyun.com/zh/isi/getting-started/obtain-an-access-token)：Token 获取方式、有效期和“移动端不要保存 AccessKey ID/Secret，应由自有服务端下发 Token”的安全建议。

### 火山引擎

- [录音文件识别标准版 HTTP](https://www.volcengine.com/docs/6561/1354868)：submit/query endpoint、header 鉴权、音频字段、格式、语言、分句/时间信息、热词上下文和错误码。
- [豆包语音计费说明](https://www.volcengine.com/docs/6561/1359370)：按时长/token、QPM/TPM 等计费说明；本次没有把未明确的免费额度写成确定值。

## 代码对应关系

- `media/models.rs`：平台无关音频输入、请求、结果、分段和错误模型；只保存 `CredentialRef`，不保存明文密钥。
- `media/transcription.rs`：provider trait、描述信息和 registry 路由。
- `media/providers/tencent.rs`：腾讯协议的明确 `reserved` 门禁，等待成熟密码学依赖和 mock contract。
- `media/providers/aliyun.rs`、`volcengine.rs`：有证据的 `reserved` 状态，拒绝假成功。
- `media/providers/relay.rs`：自有中转统一 JSON 契约、HTTPS/loopback endpoint 校验、大小上限、结果校验和取消。
