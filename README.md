# Paimon-Skill

派蒙技能包：给 **Claude Code / Codex / 其他兼容 Agent** 用的原神配装与图鉴查询 skills。  
**`skills/` 为唯一 skill 正文**，各 Agent 用插件元数据指向同一目录，而不是绑死在 `.claude` 里。

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
        └── references/
            ├── sources.md
            └── query-cheatsheet.md
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
| 数据源 | [B站原神 WIKI](https://wiki.biligame.com/ys/首页) + [原神观测枢](https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header) |
| 策略 | 按需拉页，禁止整站爬取；回答必须带来源链接 |
| 运行时 | Shell `curl` / HTTP + 网页阅读（Jina 等），**不绑定** 单一 Agent |

## 如何使用

### 方式 A：以本仓库为工作区（推荐联调）

1. 用 Claude Code、Codex 或兼容 IDE **打开** `D:\code\Paimon-Skill`（或你的克隆路径）为项目根。  
2. 新开对话，直接问原神问题，或手动点名：

```text
使用 genshin-build-wiki skill 查询：胡桃圣遗物和武器怎么配？
```

### 方式 B：装进个人 skills 目录（全局）

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

### 方式 C：插件元数据

- Codex：见 `.codex-plugin/plugin.json` 的 `"skills": "./skills/"`  
- Claude：见 `.claude-plugin/plugin.json` 与 `marketplace.json`  

具体安装命令随 Agent 版本变化，以各产品文档为准；**skill 正文始终只维护 `skills/` 一份**。

## 示例问题

- 胡桃圣遗物和武器怎么配？
- 绝缘之旗印适合哪些角色？
- 那维莱特主词条和副词条优先级？
- 芙宁娜配队推荐？

## 合格回答长什么样

1. 会实际查询 BWiki / 观测枢（不是纯背答案）  
2. 结构接近：结论 → 依据摘要 → 注意事项 → 来源  
3. 带来源标题 + URL  
4. 区分「图鉴数值」与「攻略推荐」  
5. 查不到就写未查到，不编造  

## 环境建议

| 能力 | 用途 |
|------|------|
| 终端 `curl` 或等价 HTTP | BWiki MediaWiki API |
| 网页阅读（Jina Reader 等） | 观测枢或难解析页面 |
| 可访问外网 | 两站均为在线公开站 |

无需把 wiki 整站下载到本仓库。

## 本地开发

- 改 skill：只编辑 `skills/genshin-build-wiki/SKILL.md` 与 `references/`  
- 改跨端约定：`AGENTS.md`  
- 改完后**新开对话**再测自动发现更干净  

## 合规

- 仅摘要引用，不整页搬运  
- 尊重 BWiki / 米哈游站点条款与版权声明  
- 不提供外挂、修改客户端等违规用途指引  

## 许可与归属

本仓库 skill 文档由维护者编写，用于个人/协作辅助查询。  
游戏内容与百科正文版权归原作者及对应站点所有。
