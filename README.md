<div align="center">
  <img src="logo.png" alt="Paimon-Skill" width="160" />

  # Paimon-Skill

  面向 Claude Code、Codex 与其他兼容 Agent 的原神攻略查询 skill。
</div>

`skills/` 是唯一正文来源，插件元数据只负责发现这份目录。

## 怎么用（最简单）

**在本仓库目录里打开 Agent，直接问原神问题即可**——无需安装、无需链接、无需说「加载 skill」。

任何兼容 Agent（Claude Code、Codex 等）在本目录启动时会读到根目录 [AGENTS.md](AGENTS.md)，其中内联了派蒙人设与完整工作规则，于是自动进入派蒙状态并按规则回答。

```text
# 在 d:\code\Paimon-Skill 里启动 Agent，然后：
胡桃怎么配？
当前卡池是什么？
大家现在怎么给芙宁娜配队？
```

想在任意目录都能用？把这份目录 clone 到本地，随时进来问；或看下方[让 Agent 全局可用](#让-agent-全局可用可选)。

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

## 让 Agent 全局可用（可选）

进本目录已经能直接用。如果想在**任意目录**都能唤起派蒙，把 `skills/genshin-build-wiki` 链接到 Agent 的个人 skills 目录即可：

```text
~/.claude/skills/genshin-build-wiki
~/.codex/skills/genshin-build-wiki
~/.agents/skills/genshin-build-wiki
```

也可以让 Agent 帮你做这步：

```text
帮我把 Paimon-Skill 链接到全局 skills 目录：https://github.com/KazeLiu/Paimon-Skill
```

链接 Agent 必须保留已有目录，并在任何全局依赖安装前征求用户同意。链接与依赖的完整流程见 [INSTALLATION_GUIDE.md](INSTALLATION_GUIDE.md)。

## 可选社区依赖

BWiki 和 OBC 查询不需要 OpenCLI。只有查询小红书、知乎或 B站社区内容时才需要它。**想看玩家实战经验，推荐装上 OpenCLI。**

两种安装方式，任选其一：

- **自己装**：执行下方命令，装完用 `opencli --version` 验证。
- **让派蒙帮你装 / 指导你装**：直接说「帮我装一下 OpenCLI」，Agent 会先检测环境、说明用途，征得你同意后再执行安装并复检；你也可以让它一步步指导你自己完成。

```bash
node skills/genshin-build-wiki/check-dependencies.js          # 只检测，不安装、不写状态文件
node skills/genshin-build-wiki/check-dependencies.js --json
npm install -g @jackwener/opencli                             # 明确同意后再执行
```

若 OpenCLI 已存在但所需平台能力为 `false`，Agent 会说明缺少的子命令；只有你明确同意更新或修复后，才会重新安装或升级。

不安装时，skill 继续使用 BWiki + OBC，并明确没有覆盖社区经验。详见 [skills/genshin-build-wiki/INSTALL.md](skills/genshin-build-wiki/INSTALL.md)。

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
├── AGENTS.md              # 内联派蒙人设+工作规则，进目录即生效
├── CLAUDE.md              # Claude Code 入口，指向 AGENTS.md
├── README.md
├── logo.png
├── INSTALLATION_GUIDE.md  # 全局链接与可选依赖的完整流程
├── package.json
├── .claude-plugin/
├── .codex-plugin/
├── tests/
└── skills/
    └── genshin-build-wiki/
        ├── SKILL.md          # 完整正文（插件发现用）
        ├── INSTALL.md
        ├── check-dependencies.js
        └── references/
            ├── sources.md
            ├── query-cheatsheet.md
            └── social-platforms.md
```

> [AGENTS.md](AGENTS.md) 是 SKILL.md 的内联精简版，两者规则保持一致：进目录即生效靠前者，插件生态发现靠后者。

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
