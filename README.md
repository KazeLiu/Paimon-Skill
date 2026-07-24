# Paimon-Skill

派蒙技能包：给 Claude Code 用的**原神配装 / 图鉴查询** skill。  
按需查询公开资料，回答圣遗物、武器、角色配装、配队等问题。

## 仓库结构

```text
Paimon-Skill/
├── README.md
└── .claude/
    └── skills/
        └── genshin-build-wiki/
            ├── SKILL.md
            └── references/
                ├── sources.md
                └── query-cheatsheet.md
```

Claude Code 会从项目内 `.claude/skills/` 发现 skill。

## Skill：`genshin-build-wiki`

| 项 | 说明 |
|----|------|
| 名称 | `genshin-build-wiki` |
| 用途 | 原神圣遗物 / 武器 / 角色配装 / 配队查询与摘要 |
| 数据源 | 仅 B站原神 WIKI + 米游社原神观测枢 |
| 策略 | 按需拉页，禁止整站爬取；回答必须带来源链接 |

### 数据源

1. [B站原神 WIKI](https://wiki.biligame.com/ys/首页)  
   - API：`https://wiki.biligame.com/ys/api.php`（优先 `opensearch` / `parse`）
2. [米游社 · 原神观测枢](https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header)

## 如何使用

### 1. 打开本仓库作为工作区

在 Cursor / VS Code 中打开：

```text
D:\code\Paimon-Skill
```

（或你本机的克隆路径。）确保 Claude Code 以**该文件夹为项目根**启动，这样会加载：

```text
.claude/skills/genshin-build-wiki/SKILL.md
```

### 2. 自动触发

直接问原神相关问题即可，例如：

- 胡桃圣遗物和武器怎么配？
- 绝缘之旗印适合哪些角色？
- 那维莱特主词条和副词条优先级？
- 芙宁娜配队推荐？

命中 `description` 中的触发条件时，Claude 应读取本 skill 并按流程查询。

### 3. 手动触发（更稳）

在对话里明确写出：

```text
使用 genshin-build-wiki skill 查询：……
```

或在支持 slash 的环境下尝试：

```text
/genshin-build-wiki
```

（具体是否显示 slash 取决于 Claude Code / 扩展版本。）

### 4. 合格回答长什么样

1. 会实际查询 BWiki / 观测枢（不是纯背答案）  
2. 结构接近：结论 → 依据摘要 → 注意事项 → 来源  
3. 带来源标题 + URL  
4. 区分「图鉴数值」与「攻略推荐」  
5. 查不到就写未查到，不编造  

## 环境建议

| 能力 | 用途 |
|------|------|
| 终端 `curl` | 调 BWiki MediaWiki API |
| 网页阅读（如 Jina Reader / agent-reach web） | 读观测枢或 API 难解析的页面 |
| 可访问外网 | 两站均为在线公开站 |

无需把 wiki 整站下载到本仓库。

## 本地开发说明

- 改 skill：编辑 `.claude/skills/genshin-build-wiki/SKILL.md`  
- 补数据源说明：`references/sources.md`  
- 补命令示例：`references/query-cheatsheet.md`  
- 改完后**新开对话**再测自动发现更干净；同对话也可手动点名 skill 名联调  

## 合规

- 仅摘要引用，不整页搬运  
- 尊重 BWiki / 米哈游站点条款与版权声明  
- 不提供外挂、修改客户端等违规用途指引  

## 许可与归属

本仓库 skill 文档由维护者编写，用于个人/协作辅助查询。  
游戏内容与百科正文版权归原作者及对应站点所有。
