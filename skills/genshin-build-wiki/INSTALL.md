# 可选社区依赖

> skill 本身进目录即用（见根目录 [AGENTS.md](../../AGENTS.md)），本文只关于查询小红书/知乎/B站所需的可选依赖 OpenCLI。

基础的 BWiki 与 OBC 查询只需要 HTTP/网页阅读能力。OpenCLI 仅用于小红书、知乎和 B站社区内容。

## 检测

```bash
node check-dependencies.js
node check-dependencies.js --json
```

脚本检测 `opencli --version`，并分别验证 `xiaohongshu/zhihu/bilibili` 子命令：

- 不安装任何软件
- 不修改全局环境
- 不创建 `.dependency-status.json`
- OpenCLI 缺失时仍返回正常检测结果

## 缺失时的处理

Agent 应告知用户：基础百科查询仍然可用，但社区经验暂不可查，并**主动推荐安装 OpenCLI**，同时给出两种方式——用户自己装，或让 Agent 代装 / 指导安装。若 OpenCLI 已存在但所需平台能力为 `false`，应指出缺少的平台子命令并询问是否更新或修复。

无论哪种方式，只有用户明确同意安装、更新或修复后，Agent 才执行当前 npm 发布包：

```bash
npm install -g @jackwener/opencli
```

- **用户自己装**：给出上面的命令与验证步骤（见下方「验证」），不替用户执行。
- **Agent 代装**：征得同意后执行安装，随后重新运行检测确认结果。
- **指导安装**：分步说明并在每一步等待用户反馈，遇到权限或网络问题给出排查建议。

安装后重新运行检测。用户拒绝或暂不安装时直接使用 BWiki + OBC，不反复询问，也不把未查询的社区内容写进回答。

## 登录与隐私

OpenCLI 某些平台功能复用本机浏览器登录态。需要登录时：

1. 请用户在本机浏览器打开并登录对应平台。
2. 登录完成后重新检测或执行查询。
3. 不要求用户在对话中粘贴 Cookie、令牌或账号密码。
4. 不把凭据写入仓库、缓存文件或日志。

## 验证

```bash
opencli --version
opencli bilibili --help
opencli xiaohongshu --help
```

具体平台命令见 [references/social-platforms.md](references/social-platforms.md)。

## 卸载

只有用户明确要求移除 OpenCLI 时才执行：

```bash
npm uninstall -g @jackwener/opencli
```

卸载 OpenCLI 不影响 BWiki 与 OBC 查询。
