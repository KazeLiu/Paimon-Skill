# 查询命令速查

命令应按当前 Shell 适配。Windows PowerShell 使用 `curl.exe` 可避免 `curl` 别名差异；其他环境可使用 `curl` 或等价 HTTP 客户端。

## BWiki：搜索标题

```bash
curl -sS -G "https://wiki.biligame.com/ys/api.php" --data-urlencode "action=opensearch" --data-urlencode "search=绝缘之旗印" --data-urlencode "limit=8" --data-urlencode "format=json"
```

返回通常为：`[查询词, [标题...], [描述...], [URL...]]`。

## BWiki：解析页面

```bash
curl -sS -G "https://wiki.biligame.com/ys/api.php" --data-urlencode "action=parse" --data-urlencode "page=绝缘之旗印" --data-urlencode "prop=text|sections|displaytitle" --data-urlencode "format=json"
```

只摘要相关章节，不整页复制。`parse.text.*` 是 HTML，`sections` 可帮助定位章节。

## BWiki：读取 wikitext

```bash
curl -sS -G "https://wiki.biligame.com/ys/api.php" --data-urlencode "action=query" --data-urlencode "prop=revisions" --data-urlencode "rvprop=content" --data-urlencode "rvslots=main" --data-urlencode "titles=胡桃" --data-urlencode "format=json"
```

## 网页正文阅读

API 难解析或需要阅读 OBC 条目时，可用当前 Agent 已有的网页阅读能力。若环境提供 Jina Reader：

```bash
curl -sS "https://r.jina.ai/https://wiki.biligame.com/ys/%E8%83%A1%E6%A1%83"
curl -sS "https://r.jina.ai/<OBC条目URL>"
```

不要把 OBC 首页响应当作具体条目内容；先从导航定位条目直链。

## 角色配装最小请求集

问题：`胡桃圣遗物和武器怎么选`

1. BWiki `opensearch`：定位胡桃精确页面。
2. BWiki `parse`：读取角色攻略相关章节。
3. OBC：读取胡桃条目的基础信息。
4. 攻略未给出完整效果时，仅补查被推荐的套装/武器页面。
5. 用户问实战取舍时，再按 [social-platforms.md](social-platforms.md) 查询社区。

## 多语言名称与考据

英文 Genshin Impact Wiki 网页受验证页阻挡时，可用公开 MediaWiki API。先搜索标题，再读取页面章节或 wikitext：

```bash
curl -sS -G "https://genshin-impact.fandom.com/api.php" --data-urlencode "action=query" --data-urlencode "list=search" --data-urlencode "srsearch=Narzissenkreuz" --data-urlencode "srlimit=10" --data-urlencode "format=json" --data-urlencode "formatversion=2"
curl -sS -G "https://genshin-impact.fandom.com/api.php" --data-urlencode "action=parse" --data-urlencode "page=Narzissenkreuz Ordo" --data-urlencode "prop=wikitext|sections|displaytitle" --data-urlencode "format=json" --data-urlencode "formatversion=2"
```

俄语 Wiki 使用 `/ru/api.php`：

```bash
curl -sS -G "https://genshin-impact.fandom.com/ru/api.php" --data-urlencode "action=query" --data-urlencode "list=search" --data-urlencode "srsearch=Нарциссенкрейц" --data-urlencode "srlimit=10" --data-urlencode "format=json" --data-urlencode "formatversion=2"
```

Project Amber 用于浏览多语言游戏文本：

```text
https://ambr.top/en/archive
https://ambr.top/ru/archive
```

具体证据分级与知乎 / Reddit 考据线索规则见 [story-research.md](story-research.md)。

## 回退规则

| 失败 | 动作 |
|------|------|
| `opensearch` 无结果 | 换简称、全名或同义词再搜 |
| `parse` 页不存在 | 使用搜索返回的精确标题 |
| 网页阅读正文为空 | 尝试具体条目 URL 或 BWiki API |
| OBC 无法解析 | 仅用 BWiki，并声明 OBC 本页未核对 |
| 两源均缺字段 | 写“未在指定来源查到”，不要编造 |
