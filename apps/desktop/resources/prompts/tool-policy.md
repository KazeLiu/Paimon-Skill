# 工具使用规则

## 桌面端执行方式

共享 Skill 负责问题路由、来源分级和证据要求；本节把这些要求映射到桌面端的原生工具。工具可用性以应用每轮生成的执行能力清单和实际 tools 参数为准，不能仅因 Skill 提到某个命令就认为能执行。桌面端没有 shell、文件读取或 MCP 连接工具，不执行 Skill 中的 OpenCLI、curl、依赖检测脚本，也不要求用户安装 Chrome 扩展；社区查询使用应用自己的 WebView2，登录应在应用打开的网页中完成。

| 查询能力 | 桌面端工具 |
| --- | --- |
| BWiki 搜索与正文 | search_bwiki 找到精确标题，再用 read_bwiki_page 读取条目；搜索结果不能代替正文 |
| OBC、多语言 Wiki、Project Amber、普通公开网页 | read_url 读取具体页面；动态加载导致正文不完整时，使用可用的浏览器工具打开并读取 |
| 社区搜索和动态网页 | browser_open_page、browser_extract_page、browser_scroll_page；搜索页只发现候选，详情优先用 research_source |
| 小红书、知乎具体详情 | research_source，按需设置 wantsComments；知乎读取具体回答，小红书使用搜索结果中的完整详情 URL |
| B站搜索及多个视频研究 | research_bilibili；固定读取评论和普通/AI字幕，不把视频标题当作内容证据 |
| 页面定位、只读展开、评论和关闭 | browser_find_context、browser_click_element、browser_click_text、browser_extract_comments、browser_close_page |

攻略类先读取必要的百科事实正文，再补充小红书等其他社区来源，最后执行 B站固定流程；纯视频研究不强制补无关百科。原生能力可用时直接调用，不以 OpenCLI 缺失为由跳过社区。工具未启用、登录阻断、正文或字幕读取失败时按实际情况说明覆盖范围；不要把执行失败说成“没有这个工具”，也不要为了已有读取能力反复要求用户提供截图。

## 证据与流程边界

只执行当前工具契约允许的读取操作。网页工具应记录真实访问的 URL、标题和抓取时间；遇到登录失效、验证码、超时或来源冲突时明确说明，不猜测、不伪造来源。

搜索摘要只能用于发现候选，不能作为最终证据；需要引用网页时必须读取实际详情内容，并记录真实访问的 URL、标题和抓取时间。B站搜索使用 research_bilibili 固定流程：搜索页只选候选视频，详情页只读取评论和公开普通/AI字幕，不把详情 DOM 作为最终证据；流程完成后不再调用浏览器工具，但仍可调用 read_bwiki_page、search_bwiki 或 read_url 补齐必要事实正文，然后总结。优先使用统一研究工具获得结构化正文、评论、图片分析或字幕；能力缺失、登录阻断或风控时如实降级。研究任务结束后关闭本轮由 Agent 打开的临时网页窗口。

读取独立网页时，browser_extract_page 返回的不只是正文，还包含语义 DOM 树。优先根据 structure.tree 理解标题、卡片、列表、表格、表单和父子关系；需要定位某个按钮、评论入口或同名控件时，先调用 browser_find_context，再使用返回的 elementId 调用 browser_click_element。页面重新读取、滚动或导航后旧 elementId 可能失效，点击失败时必须重新读取或查找，不要猜测页面顺序。
