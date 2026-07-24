# Paimon-Skill — Agent 指引

本仓库是 **Agent 无关** 的 skill 包（`skills/` 为唯一正文来源）。

## 发现 skill

- 权威路径：`skills/<skill-name>/SKILL.md`
- Codex 插件声明：`.codex-plugin/plugin.json` → `"skills": "./skills/"`
- Claude 插件元数据：`.claude-plugin/plugin.json`、`.claude-plugin/marketplace.json`

在本仓库根目录打开 Claude Code / Codex / 其他兼容 Agent 时，应加载 `skills/` 下的 skill。

## 当前 skills

| 名称 | 路径 | 用途 |
|------|------|------|
| `genshin-build-wiki` | `skills/genshin-build-wiki/` | 原神圣遗物 / 武器 / 配装 / 配队按需查询 |

## 使用约定

1. 命中原神配装/图鉴类问题时，先读对应 `SKILL.md` 再回答。
2. 默认事实来源仅：米游社原神观测枢（第一方百科）+ B站原神 WIKI（社区维护 Wiki）。
3. 社交平台只按意图补充玩家经验，不作为稳定事实来源；用户未授权时不扩展到其他攻略站。
4. 禁止整站爬取；回答必须带实际读取过的来源链接，“当前/最新”信息还要标注查询时间与适用版本或期数。
5. 依赖检测不得安装软件或写入机器状态；缺少可选依赖时先询问用户，得到明确同意后再安装。
6. 工具实现用当前 Agent 可用的 Shell/HTTP/网页阅读能力，不绑定单一产品。
