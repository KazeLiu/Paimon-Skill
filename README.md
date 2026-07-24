# Paimon-Skill

面向 Claude Code、Codex 与其他兼容 Agent 的原神攻略查询 skill。`skills/` 是唯一正文来源，插件元数据只负责发现这份目录。

## 能做什么

- 查询角色配装、圣遗物、武器、配队、培养材料与玩法机制
- 查询当前卡池、活动、版本与深境螺旋等时效信息
- 按需补充小红书、知乎和 B站的玩家经验
- 使用轻量派蒙语气回答，并附实际查询过的来源链接

## 数据源

| 层级 | 来源 | 用途 |
|------|------|------|
| 第一方资料 | [米游社原神观测枢](https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header) | 基础档案、材料、数值与官方说明 |
| 社区维护 Wiki | [B站原神 WIKI](https://wiki.biligame.com/ys/首页) | 图鉴、机制与角色攻略 |
| 社区经验 | 小红书、知乎、B站视频 | 实战体验、平替与深渊案例 |

稳定图鉴事实不查询社交平台。配装争议、实战取舍或用户明确询问“大家怎么配”时，才启用社区查询。

## 快速开始

```bash
git clone https://github.com/KazeLiu/Paimon-Skill.git
cd Paimon-Skill
```

然后用 Agent 打开仓库，或将 `skills/genshin-build-wiki` 链接到该 Agent 的个人 skills 目录：

```text
~/.claude/skills/genshin-build-wiki
~/.codex/skills/genshin-build-wiki
~/.agents/skills/genshin-build-wiki
```

也可以让 Agent 执行安装：

```text
帮我安装 Paimon-Skill：https://github.com/KazeLiu/Paimon-Skill
```

安装 Agent 必须保留已有目录，并在任何全局依赖安装前征求用户同意。完整流程见 [INSTALLATION_GUIDE.md](INSTALLATION_GUIDE.md)。

## 可选社区依赖

BWiki 和 OBC 查询不需要 OpenCLI。只有查询小红书、知乎或 B站社区内容时才需要它。

```bash
node skills/genshin-build-wiki/check-dependencies.js
node skills/genshin-build-wiki/check-dependencies.js --json
```

检测脚本只检查当前环境，不安装软件、不写状态文件。未检测到 OpenCLI 时，Agent 应先询问用户；得到明确同意后才可执行：

```bash
npm install -g @jackwener/opencli
```

若 OpenCLI 已存在但所需平台能力为 `false`，Agent 应说明缺少的子命令；只有用户明确同意更新或修复后，才可重新安装或升级 OpenCLI。

用户不同意时，skill 继续使用 BWiki + OBC，并明确没有覆盖社区经验。详见 [skills/genshin-build-wiki/INSTALL.md](skills/genshin-build-wiki/INSTALL.md)。

## 示例

- `绝缘之旗印四件套效果是什么？`
- `胡桃圣遗物和武器怎么配？`
- `那维莱特主词条和副词条怎么选？`
- `当前卡池是什么？查询时间也告诉我。`
- `大家现在怎么给芙宁娜配队？`

## 回答质量

合格回答应：

1. 先给可执行结论，再解释条件与取舍。
2. 把 OBC 标为第一方资料、BWiki 标为社区维护 Wiki、社交内容标为社区经验。
3. “当前/最新”信息重新查询，并附绝对查询时间、时区、适用版本或深渊期数。
4. 只引用实际读取过的页面，给出标题与完整 URL。
5. 部分来源失败时说明覆盖范围；全部失败时不凭记忆编造。

## 仓库结构

```text
Paimon-Skill/
├── AGENTS.md
├── README.md
├── INSTALLATION_GUIDE.md
├── package.json
├── .claude-plugin/
├── .codex-plugin/
├── tests/
└── skills/
    └── genshin-build-wiki/
        ├── SKILL.md
        ├── INSTALL.md
        ├── check-dependencies.js
        └── references/
            ├── sources.md
            ├── query-cheatsheet.md
            └── social-platforms.md
```

## 开发与测试

```bash
npm test
node skills/genshin-build-wiki/check-dependencies.js --json
```

- Skill 正文只维护 `skills/genshin-build-wiki/`。
- 跨 Agent 约定写入 `AGENTS.md`。
- 修改触发或查询流程后，用全新对话测试自动发现和回答行为。
- 禁止整站抓取、提交机器相关状态或把登录凭据写入仓库。

## 许可与归属

本仓库采用 [MIT License](LICENSE)。游戏内容、角色与百科正文版权归其各自权利人和内容作者所有。
