# 数据源与入口

## 锁定数据源

| 名称 | 入口 | 角色 |
|------|------|------|
| B站原神 WIKI | https://wiki.biligame.com/ys/首页 | 图鉴、机制、角色攻略、筛选工具入口 |
| 原神观测枢 | https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header | 官方向百科、培养与基础信息 |

本 skill **默认不得**改用其他攻略站。用户当次明确点名其他来源时，以用户当次要求为准。

## BWiki 导航印象（首页级）

首页常见分区（名称可能微调，以线上为准）：

- 图鉴：角色、武器、圣遗物等
- 攻略/筛选：角色攻略筛选、培养总览
- 小工具：伤害相关、圣遗物强化模拟等
- 玩法：深渊等（配装语境可能相关）

### 配装相关高频路径模式

```
https://wiki.biligame.com/ys/<页面名>
```

示例（页面名需 URL 编码）：

- `/ys/圣遗物图鉴`
- `/ys/圣遗物一览`
- `/ys/武器图鉴`
- `/ys/角色攻略筛选`
- `/ys/<角色名>`
- `/ys/<套装名>`
- `/ys/<武器名>`

### API

```
https://wiki.biligame.com/ys/api.php
```

MediaWiki 实例；优先 `opensearch` + `parse` / `query`。

### 网页阅读（按 agent-reach 优先级）

当 API 解析困难或需要可读文本时：

1. **Jina Reader**（零配置，首选）
   ```bash
   curl -s "https://r.jina.ai/https://wiki.biligame.com/ys/<页面名 URL 编码>"
   ```

2. **web-reader MCP**（若已安装）
   ```bash
   mcporter call 'web-reader.webReader(url: "https://wiki.biligame.com/ys/<页面名>")'
   ```

**环境检测**：首次使用 web-reader MCP 失败时，询问用户是否安装。

## 观测枢

- 桌面入口带参数：`visit_device=pc&bbs_presentation_style=no_header`
- 内容偏官方百科结构；条目 URL 以站点内跳转为准
- **网页访问优先级**（按 agent-reach 规范）：
  1. Jina Reader（`curl -s "https://r.jina.ai/<条目URL>"`）
  2. web-reader MCP（若已安装）
- 若 SPA 导致阅读器拿不到正文：记录失败，回退 BWiki，不擅自换源

## 双源分工建议

| 问题 | 更先看 |
|------|--------|
| 套装/武器数值与特效原文 | 两边都可；冲突时并列 |
| 角色推荐圣遗物/武器序 | BWiki 角色攻略 |
| 突破材料、基础档案 | 观测枢 |
| 社区向取舍、深渊向说明 | BWiki |

## 归因

回答中的「来源」一节至少包含实际使用过的页面标题与完整 URL。  
BWiki 内容常见 CC 类声明，以页面页脚为准；做摘要引用，避免整页转载。
