# OpenCLI 安装指南（可选社区依赖）

> skill 本身进目录即用（见根目录 [AGENTS.md](../../AGENTS.md)）。本文只关于查询**小红书 / 知乎 / B站**社区经验所需的可选依赖 **OpenCLI**。不装它，BWiki + OBC 的官方百科查询照常可用。

## OpenCLI 是什么，为什么要装两部分

OpenCLI 不是单纯的命令行工具，也不是单纯的浏览器插件，而是两者的组合：

```
opencli 命令 → 本地守护进程(localhost:19825) → WebSocket → Chrome 扩展 → 你已登录的浏览器
```

- **命令行部分（npm 包）**：你在终端敲 `opencli xiaohongshu search ...` 的那个 CLI。
- **浏览器扩展部分（Browser Bridge）**：真正的认证来源。它**复用你 Chrome 里已登录的账号会话**读取页面，不需要 API Key，也不需要你粘贴 Cookie。

**两者缺一不可**：只装 npm 包、不装扩展，命令连不上浏览器，社区平台查不了。守护进程在首次浏览器命令时自动启动，无需手动配置。

> ⚠️ **无头服务器不支持**：扩展就是认证源，没有桌面浏览器会话的服务器环境无法使用社区查询。此时只用 BWiki + OBC。

---

## 第一步：安装命令行（npm）

需要 Node.js 18+。用户明确同意后执行：

```bash
npm install -g @jackwener/opencli
opencli --version   # 验证
```

## 第二步：安装浏览器扩展（Browser Bridge）

任选一种。**推荐 A（应用商店，自动更新）**。

### A. Chrome 应用商店（推荐）

打开并添加官方扩展 **OpenCLI Browser Bridge**：

```
https://chromewebstore.google.com/detail/opencli-browser-bridge/ildkmabpimmkaediidaifkhjpohdnifk
```

> 商店里有个别开发者上架的同名 “Browser Bridge”，请认准上面这个 ID（`ildkmabpimmkaediidaifkhjpohdnifk`），别装错。

### B. 离线 / 手动加载（无法访问商店时）

1. 到 GitHub Releases 下载最新的 `opencli-extension-v{版本}.zip`：`https://github.com/jackwener/opencli/releases`
2. 解压到一个固定目录。
3. Chrome 打开 `chrome://extensions`，右上角开启「开发者模式」。
4. 点「加载已解压的扩展程序」，选中第 2 步解压出的文件夹。

## 第三步：登录目标平台

在**同一个 Chrome** 里打开并登录小红书 / 知乎 / B站（查哪个登哪个）。OpenCLI 复用这个登录态。

- 登录只在你本机浏览器完成。
- **不要**把 Cookie、令牌或账号密码粘贴到对话里，Agent 也不会索取。
- 凭据不写入仓库、缓存文件或日志。

---

## 检测

skill 通过检测脚本判断当前环境是否具备社区查询能力：

```bash
node check-dependencies.js
node check-dependencies.js --json
```

脚本检测 `opencli --version` 并分别验证 `xiaohongshu/zhihu/bilibili` 子命令，且：

- 不安装任何软件
- 不修改全局环境
- 不创建 `.dependency-status.json`
- OpenCLI 缺失时仍返回正常检测结果

> 注意：检测脚本只验证 **CLI 侧**。即使命令可用，若浏览器扩展未安装或未登录，实际查询仍会失败——遇到「未登录/连接不上浏览器」时，回到第二、三步。

## 缺失时的处理（给 Agent）

告知用户：基础百科查询仍可用，但社区经验暂不可查，并**主动推荐安装 OpenCLI**，给出两种方式——用户自己装，或让 Agent 代装 / 指导安装。若 OpenCLI 已存在但所需平台能力为 `false`，指出缺少的平台子命令并询问是否更新或修复。

无论哪种方式，**只有用户明确同意后**才执行安装 / 更新 / 修复：

- **用户自己装**：给出上面三步的命令与链接，不替用户执行。
- **Agent 代装**：征得同意后执行 `npm install -g @jackwener/opencli`，再引导用户装扩展并登录（扩展与登录必须由用户在本机浏览器完成），随后重新检测。
- **指导安装**：分步说明并在每步等待反馈，遇到权限 / 网络 / 商店无法访问时改走离线加载（第二步 B）。

用户拒绝或暂不安装时直接使用 BWiki + OBC，不反复询问，也不把未查询的社区内容写进回答。

## 验证

```bash
opencli --version
opencli bilibili --help
opencli xiaohongshu --help
```

装好扩展并登录后，可用一条真实查询确认全链路通：

```bash
opencli bilibili search "原神" --limit 3 -f json
```

具体平台命令见 [references/social-platforms.md](references/social-platforms.md)。

## 卸载

只有用户明确要求移除 OpenCLI 时才执行：

```bash
npm uninstall -g @jackwener/opencli
```

浏览器扩展在 `chrome://extensions` 里手动移除。卸载 OpenCLI 不影响 BWiki 与 OBC 查询。
