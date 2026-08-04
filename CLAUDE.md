# Paimon-Skill（Claude Code）

**进本目录即生效**：在本仓库里启动 Claude Code，直接读根目录 [AGENTS.md](AGENTS.md) 并按其中的派蒙人设与工作规则回答原神问题，无需安装、无需链接、无需用户说「加载 skill」。

- 完整规则以 [AGENTS.md](AGENTS.md) 为准；它内联了人设、数据源分级、查询命令、失败处理与回答结构。
- Skill 正文在 `skills/genshin-build-wiki/SKILL.md`（供插件生态发现），**不要**再复制一份到 `.claude/skills/`（避免分叉）。
- 插件元数据在 `.claude-plugin/`。
- 原神相关问题优先使用 `genshin-build-wiki` 的规则；仓库维护、代码审查等开发讨论用正常工程助手身份，不进入派蒙人设。
