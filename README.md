# Paimon-Skill

派蒙技能包：给 **Claude Code / Codex / 其他兼容 Agent** 用的原神配装与图鉴查询 skills。  
**`skills/` 为唯一 skill 正文**，各 Agent 用插件元数据指向同一目录，而不是绑死在 `.claude` 里。

## 快速安装（推荐）

让 AI 一键安装本 skill 及其依赖：

```
帮我安装 Paimon-Skill：https://github.com/KazeLiu/Paimon-Skill
```

AI 会自动：
1. 克隆或下载本仓库
2. 检测并安装依赖（opencli + bili-cli）
3. 链接到你的 Agent skills 目录
4. 验证安装是否成功

安装完成后，直接问原神问题或呼叫派蒙即可使用！

## 仓库结构

```text
Paimon-Skill/
├── README.md
├── AGENTS.md                 # 跨 Agent 使用约定
├── CLAUDE.md                 # Claude Code 入口提示
├── package.json
├── .claude-plugin/           # Claude 插件 / marketplace 元数据
│   ├── plugin.json
│   └── marketplace.json
├── .codex-plugin/            # Codex 插件元数据（skills → ./skills/）
│   └── plugin.json
└── skills/                   # ★ 权威 skill 目录（Claude / Codex 共用）
    └── genshin-build-wiki/
        ├── SKILL.md
        ├── INSTALL.md            # 依赖安装指南
        ├── check-dependencies.js # 自动依赖检测脚本
        └── references/
            ├── sources.md
            ├── query-cheatsheet.md
            └── social-platforms.md
```

| 路径 | 作用 |
|------|------|
| `skills/` | skill 正文（改这里即可） |
| `.codex-plugin/` | Codex 发现 `./skills/` |
| `.claude-plugin/` | Claude 插件/marketplace 元数据 |
| `AGENTS.md` | 任意 Agent 打开本仓库时的约定 |

## Skill：`genshin-build-wiki`

| 项 | 说明 |
|----|------|
| 名称 | `genshin-build-wiki` |
| 用途 | 原神圣遗物 / 武器 / 角色配装 / 配队查询与摘要 |
| 数据源 | **官方**: [B站原神 WIKI](https://wiki.biligame.com/ys/首页) + [原神观测枢](https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header)<br>**社区**: 小红书 + 知乎 + B站视频 |
| 策略 | 官方数据为主，社区实战为辅，派蒙综合建议；按需拉页，禁止整站爬取；回答必须带来源链接 |
| 运行时 | Shell `curl` / HTTP + 网页阅读（Jina 等） + OpenCLI + bili-cli |

### 功能特性

✅ **官方 Wiki 查询**（BWiki + 观测枢）  
✅ **社区讨论综合**（小红书/知乎实战反馈）  
✅ **视频攻略推荐**（B站视频）  
✅ **派蒙语气回答**（可爱、热心、有梗）  
✅ **自动依赖检测与安装**（opencli + bili-cli）  
✅ **降级模式**（依赖失败时仍可使用官方数据）

## 如何使用

### 方式 A：AI 一键安装（推荐）

复制这句话给你的 AI：

```
帮我安装 Paimon-Skill：https://github.com/KazeLiu/Paimon-Skill
```

AI 会自动完成所有安装步骤。

### 方式 B：以本仓库为工作区（推荐联调）

1. 用 Claude Code、Codex 或兼容 IDE **打开** `D:\code\Paimon-Skill`（或你的克隆路径）为项目根。  
2. 新开对话，直接问原神问题，或手动点名：

```text
使用 genshin-build-wiki skill 查询：胡桃圣遗物和武器怎么配？
```

### 方式 C：手动装进个人 skills 目录（全局）

把 `skills/genshin-build-wiki` **复制或链接**到你当前 Agent 的 skills 目录，例如：

```text
# Claude Code（示例）
~/.claude/skills/genshin-build-wiki

# Codex（示例）
~/.codex/skills/genshin-build-wiki

# 通用 agents 路径（若使用）
~/.agents/skills/genshin-build-wiki
```

Windows 可用目录联接，避免维护多份副本：

```powershell
# 示例：Claude
New-Item -ItemType Junction `
  -Path "$env:USERPROFILE\.claude\skills\genshin-build-wiki" `
  -Target "D:\code\Paimon-Skill\skills\genshin-build-wiki"

# 示例：Codex
New-Item -ItemType Junction `
  -Path "$env:USERPROFILE\.codex\skills\genshin-build-wiki" `
  -Target "D:\code\Paimon-Skill\skills\genshin-build-wiki"
```

**首次使用前，运行依赖检测：**

```bash
cd skills/genshin-build-wiki
node check-dependencies.js
```

### 方式 D：插件元数据

- Codex：见 `.codex-plugin/plugin.json` 的 `"skills": "./skills/"`  
- Claude：见 `.claude-plugin/plugin.json` 与 `marketplace.json`  

具体安装命令随 Agent 版本变化，以各产品文档为准；**skill 正文始终只维护 `skills/` 一份**。

## 依赖说明

本 skill 需要以下依赖来获取社区讨论（首次使用时自动安装）：

| 依赖 | 用途 | 安装命令 | 失败后影响 |
|------|------|---------|-----------|
| **opencli** | 小红书/知乎/B站查询 | `npm install -g @panniantong/opencli` | 降级到仅官方数据 |
| Jina Reader | 网页阅读 | 无需安装（公开接口） | - |
| web-reader MCP | 网页阅读（可选） | 按需提示安装 | 使用 Jina 兜底 |

**降级模式**：即使依赖安装失败，skill 仍可正常工作（仅使用官方 Wiki 数据）。

**注意**：OpenCLI 自带 `bilibili` 子命令，无需单独安装 bili-cli。

详见 [`skills/genshin-build-wiki/INSTALL.md`](skills/genshin-build-wiki/INSTALL.md)。

## 示例问题

- 派蒙！
- 胡桃圣遗物和武器怎么配？
- 绝缘之旗印适合哪些角色？
- 那维莱特主词条和副词条优先级？
- 芙宁娜配队推荐？
- 大家都怎么给钟离配装？（会查社区讨论）

## 合格回答长什么样

1. 用派蒙语气回答，称呼用户为「旅行者」  
2. 综合官方数据（BWiki + OBC）与社区讨论（小红书/知乎）  
3. 会实际查询数据源（不是纯背答案）  
4. 结构接近：官方推荐 → 社区反馈 → 派蒙综合建议 → 来源（官方 + 社区）  
5. 带来源标题 + URL + 数据（点赞数/播放量/时间）  
6. 区分「图鉴数值」与「攻略推荐」  
7. 查不到就写未查到，不编造  

## 环境建议

| 能力 | 用途 |
|------|------|
| **Node.js >= 16** | 运行依赖检测脚本 + opencli + bili-cli |
| **npm >= 7** | 安装全局依赖 |
| 终端 `curl` 或等价 HTTP | BWiki MediaWiki API |
| 网页阅读（Jina Reader 等） | 观测枢或难解析页面 |
| 可访问外网 | 两站均为在线公开站 |

无需把 wiki 整站下载到本仓库。

## 本地开发

- 改 skill：只编辑 `skills/genshin-build-wiki/SKILL.md` 与 `references/`  
- 改跨端约定：`AGENTS.md`  
- 改完后**新开对话**再测自动发现更干净  
- 测试依赖检测：`node skills/genshin-build-wiki/check-dependencies.js`

## 合规

- 仅摘要引用，不整页搬运  
- 尊重 BWiki / 米哈游站点条款与版权声明  
- 不提供外挂、修改客户端等违规用途指引  

## 许可与归属

本仓库 skill 文档由维护者编写，用于个人/协作辅助查询。  
游戏内容与百科正文版权归原作者及对应站点所有。

MIT License - 详见 LICENSE 文件。
