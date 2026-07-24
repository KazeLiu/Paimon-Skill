# 查询速查

## BWiki：搜索

```bash
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=opensearch" \
  --data-urlencode "search=绝缘之旗印" \
  --data-urlencode "limit=8" \
  --data-urlencode "format=json"
```

返回一般为：`[查询词, [标题...], [描述...], [URL...]]`。

## BWiki：解析页面 HTML

```bash
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=parse" \
  --data-urlencode "page=绝缘之旗印" \
  --data-urlencode "prop=text|sections|displaytitle" \
  --data-urlencode "format=json"
```

从 JSON 的 `parse.text.*` 取 HTML 后再摘要；`sections` 用于只读相关章节。

## BWiki：取 wikitext

```bash
curl -sG "https://wiki.biligame.com/ys/api.php" \
  --data-urlencode "action=query" \
  --data-urlencode "prop=revisions" \
  --data-urlencode "rvprop=content" \
  --data-urlencode "rvslots=main" \
  --data-urlencode "titles=胡桃" \
  --data-urlencode "format=json"
```

## 网页正文阅读（按 agent-reach 优先级）

**优先级**：Jina Reader（零配置首选） → web-reader MCP（若已安装）

```bash
# 1. Jina Reader（零配置，首选）
curl -s "https://r.jina.ai/https://wiki.biligame.com/ys/%E8%83%A1%E6%A1%83"
curl -s "https://r.jina.ai/https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header"

# 2. web-reader MCP（若已安装）
mcporter call 'web-reader.webReader(url: "https://wiki.biligame.com/ys/胡桃")'
mcporter call 'web-reader.webReader(url: "https://baike.mihoyo.com/ys/obc/...")'
```

**环境检测**：首次使用 web-reader MCP 失败时，询问用户是否安装。

对 OBC 具体条目，把 URL 换成条目直链效果更好。

## 角色配装最小请求集示例

问题：`胡桃 圣遗物和武器怎么选`

1. BWiki opensearch：`胡桃`
2. BWiki parse：角色页（攻略段）
3. OBC：胡桃条目（若能定位）
4. 若攻略只写套装名未写效果：再 parse 套装页 / 武器页 **仅补缺**

## 失败回退

| 失败 | 动作 |
|------|------|
| opensearch 无结果 | 换同义词/简称/全名再搜 |
| parse 页不存在 | 用搜索返回的精确标题 |
| Jina Reader 空正文 | 尝试 web-reader MCP；若都失败则用 API 或声明失败 |
| web-reader MCP 不可用 | 询问用户是否安装；用户拒绝或跳过则继续用 Jina Reader |
| OBC 阅读失败 | 只用 BWiki 并声明「观测枢本页未能解析」 |
| 两源都缺某字段 | 回答写「未在指定数据源查到」，不要编 |
