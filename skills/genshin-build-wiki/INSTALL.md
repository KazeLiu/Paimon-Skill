# 可选社区依赖

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

Agent 应告知用户：基础百科查询仍然可用，但社区经验暂不可查。随后询问用户是否安装 OpenCLI。若 OpenCLI 已存在但所需平台能力为 `false`，应指出缺少的平台子命令并询问是否更新或修复。

只有用户明确同意安装、更新或修复后，才执行当前 npm 发布包：

```bash
npm install -g @jackwener/opencli
```

安装后重新运行检测。用户拒绝时直接使用 BWiki + OBC，不反复询问，也不把未查询的社区内容写进回答。

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
