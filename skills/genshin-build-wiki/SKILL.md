---
name: genshin-build-wiki
description: Use when the user asks about Genshin Impact 原神 builds, artifact/圣遗物 sets and substats, weapons/武器 choices, character推荐配装, team/配队, or needs facts from B站原神WIKI or 米游社原神观测枢.
---

# 原神配装 / 图鉴查询（BWiki + 观测枢）

## Overview

按需查询公开资料，回答原神**圣遗物、武器、角色配装、配队**问题。  
**禁止整站爬取**；每次只拉与问题直接相关的最小页面集合。

数据源仅限用户指定的两个站：

1. **B站原神 WIKI（BWiki）**：https://wiki.biligame.com/ys/首页  
2. **米游社 · 原神观测枢（OBC）**：https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header

## When to Use

- 用户问：某角色怎么配、圣遗物套装/主副词条、武器毕业/过渡、配队、词条机制
- 用户要对照图鉴数值、套装效果、武器特效
- 用户明确提到 BWiki / 观测枢 / biligame / mihoyo baike / 原神 / 原神某角色

**不要用本 skill：**

- 非原神游戏问题
- 要求破解、修改客户端、自动化外挂
- 用户指定了其他唯一数据源且排除上述两站

## 核心原则

1. **先意图，再取页**：判断问题类型后再选入口，不要从首页盲爬。
2. **按需拉取**：优先 MediaWiki API / 单页阅读；禁止镜像整站。
3. **双源对照**：关键推荐尽量 BWiki 与 OBC 都查；冲突时并列说明并给链接。
4. **可追溯**：回答必须带来源标题 + URL。
5. **区分事实与推荐**：套装数值/武器特效是事实；「毕业装」「最优武器」是攻略观点，标注时效。
6. **合规**：摘要回答，不整页搬运；尊重站点 ToS / 版权声明。

## 意图路由

| 用户意图 | 优先查什么 | 次优先 |
|---------|-----------|--------|
| 某角色配装 | 角色攻略 / 角色词条页 | 圣遗物套装页、武器页 |
| 圣遗物套装效果 / 适用角色 | 圣遗物图鉴或套装专页 | 角色页中的推荐段落 |
| 主词条 / 副词条优先级 | 角色攻略 + 圣遗物属性说明 | 观测枢角色培养 |
| 武器选择 / 对比 | 武器图鉴或武器专页 + 角色攻略武器栏 | 观测枢武器条目 |
| 配队 | 角色攻略配队段 | 相关角色页互相印证 |
| 机制（元素反应、充能等） | BWiki 机制/词条页 | 观测枢对应百科 |

无法归类时：用 BWiki `opensearch` + OBC 站内搜索（或首页导航定位）找最接近词条。

## 数据源用法

### A. B站原神 WIKI（主：结构化查询）

**站点根路径**

- 文章：`https://wiki.biligame.com/ys/<页面名>`
- API：`https://wiki.biligame.com/ys/api.php`
- 首页：`https://wiki.biligame.com/ys/首页`

**推荐 API（优先于硬爬 HTML）**

```bash
# 1) 搜索页面标题
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=opensearch" \
  --data-urlencode "search=<关键词>" \
  --data-urlencode "limit=10" \
  --data-urlencode "format=json"

# 2) 取页面 HTML 正文（适合再摘要）
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=parse" \
  --data-urlencode "page=<精确页面名>" \
  --data-urlencode "prop=text|sections|displaytitle" \
  --data-urlencode "format=json"

# 3) 取 wikitext（模板多时便于定位字段）
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=query" \
  --data-urlencode "prop=revisions" \
  --data-urlencode "rvprop=content" \
  --data-urlencode "titles=<精确页面名>" \
  --data-urlencode "format=json"
```

**常用入口页面名（可随站点改名；找不到就 opensearch）**

| 主题 | 常见页面名 |
|------|-----------|
| 圣遗物总览 | `圣遗物` / `圣遗物一览` / `圣遗物图鉴` |
| 圣遗物属性 | `圣遗物属性` |
| 武器 | `武器图鉴` / `武器一览` |
| 角色攻略索引 | `角色攻略筛选` / `角色图鉴` |
| 单角色 | 角色中文名，如 `胡桃`、`那维莱特`（以搜索结果为准） |
| 单套装 | 套装中文名，如 `绝缘之旗印` |
| 单武器 | 武器中文名 |

**页面阅读兜底**（API 解析困难 / 需要可读文本时）：

- 使用 agent-reach 网页通道：`curl -sL "https://r.jina.ai/https://wiki.biligame.com/ys/<页面名 URL 编码>"`
- 或项目内已配置的等价网页阅读工具

### B. 米游社 · 原神观测枢（主：官方向百科/培养）

**入口**

- 首页：https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header

**访问方式**

1. 先打开首页，从导航定位：角色 / 武器 / 圣遗物 / 敌人等分类。  
2. 对具体条目 URL 使用网页阅读（Jina Reader 或等价工具）提取正文。  
3. 若页面强依赖前端渲染导致正文为空：  
   - 换条目直链重试  
   - 或仅用 BWiki 回答，并明确「观测枢本页未能解析」  
   - **不要**为此改去未授权的第三方站（本 skill 数据源锁定两站）

**查询提示**

- 角色培养、基础数值、材料、官方向说明：优先 OBC  
- 社区向配装推荐、深渊向取舍：优先 BWiki 角色攻略，再用 OBC 核对基础信息

## 标准执行流程

```
1. 解析问题
   - 角色 / 圣遗物 / 武器 / 配队 / 机制
   - 是否指定版本、主C/副C、深渊或大世界

2. 列最小页面清单（通常 1–4 个）
   - 例：「胡桃配装」→ 胡桃(BWiki) + 胡桃(OBC) + 涉及的主推套装/武器页（若攻略未写清）

3. 拉取
   - BWiki：opensearch → parse/query
   - OBC：条目页阅读
   - 并行拉取互不依赖的页面

4. 抽取字段
   - 圣遗物：2/4 件套效果、主词条建议、副词条优先级、毕业线（若有）
   - 武器：基础攻击/副属性、特效摘要、精炼相关注意
   - 角色：推荐套装排序、武器排序、天赋/命座注意点、配队示例
   - 明确标注「攻略推荐」vs「图鉴数值」

5. 冲突处理
   - 数值不一致：以能核对的图鉴字段为准，并贴两侧链接
   - 推荐不一致：并列「BWiki 倾向 / 观测枢倾向」，说明可能因版本或定位不同

6. 输出（见下）
```

## 回答模板

默认使用以下结构（可按问题裁剪，但来源不可省略）：

```markdown
## 结论（先给可用答案）
- 推荐套装：…
- 主词条：花/羽固定；沙/杯/冠：…
- 副词条优先级：…
- 武器：1) … 2) … 3) …
- 配队（若问到）：…

## 依据摘要
- 套装效果 / 武器特效关键句（简述，不整页粘贴）
- 适用场景与取舍（充能、精通、治疗等）

## 注意事项
- 版本或玩法前提（若资料有写）
- 资料未覆盖的部分如实说「未查到」

## 来源
- BWiki：<标题> <URL>
- 观测枢：<标题> <URL>
```

## 缓存策略（可选，轻量）

允许在项目或用户缓存目录写短缓存，**不是**全站库：

| 内容类型 | 建议 TTL |
|---------|----------|
| 套装 2/4 件文字效果、武器基础特效 | 24–72h |
| 角色推荐配装段落 | 6–24h |
| 搜索结果列表 | ≤6h |

缓存键建议：`source + page_title + retrieved_date`。  
用户说「刷新/最新/当前版本」时强制绕过缓存。

## 禁止事项

- 禁止把 BWiki / OBC 整站或大批量页面 dump 进仓库
- 禁止编造未在来源中出现的套装数值、武器特效、推荐结论
- 禁止把攻略观点写成「游戏官方唯一正确答案」
- 禁止扩展到本 skill 未授权的数据源（除非用户当次明确要求）
- 禁止输出外挂、内存修改、自动化脚本用于破坏公平性

## 工具选择（Claude / Codex / 其他 Agent 通用）

本 skill **不绑定** 单一 Agent 产品。用当前环境里可用的 HTTP / 网页阅读能力即可：

| 步骤 | 工具（按可用性选用） |
|------|----------------------|
| BWiki API | Shell `curl` / 任意 HTTP 客户端 |
| 页面正文可读化 | `https://r.jina.ai/<URL>`、agent-reach web 通道、或其他网页阅读工具 |
| 多页互不依赖 | 可并行请求 |
| 复杂长文对比 | 先抽章节标题，再只读相关 section |

若环境里有 `agent-reach`：读网页前按其 web 通道规范执行，并声明使用了该通道。  
没有也不影响：直接用 `curl` + Jina Reader / 等价方案。

## 自检清单（回答前）

- [ ] 是否只查了与问题相关的页面？
- [ ] 是否至少尝试了指定数据源中的合理入口？
- [ ] 数值/推荐是否能在来源中对应？
- [ ] 是否附了可点击/可复制的来源链接？
- [ ] 是否避免了整页复制粘贴？

## 参考文件

- 入口与页面映射补充：[references/sources.md](references/sources.md)
- 查询命令速查：[references/query-cheatsheet.md](references/query-cheatsheet.md)
