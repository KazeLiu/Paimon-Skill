use scraper::{Html, Selector};
use std::collections::BTreeMap;
use url::Url;

const MAX_BODY_CHARS: usize = 18_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZhihuPageKind {
    Question,
    Answer,
    Article,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZhihuDetailUrl {
    pub(crate) safe_url: String,
    pub(crate) kind: ZhihuPageKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ZhihuFields {
    pub(crate) title: Option<String>,
    pub(crate) body: Option<String>,
    pub(crate) author: Option<String>,
    pub(crate) published_at: Option<String>,
    pub(crate) updated_at: Option<String>,
    pub(crate) interactions: BTreeMap<String, String>,
    pub(crate) visual_candidate: bool,
}

pub(crate) fn validate_zhihu_detail_url(raw: &str) -> Result<ZhihuDetailUrl, String> {
    let parsed = Url::parse(raw).map_err(|_| "知乎 URL 不是有效链接".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("知乎 URL 必须使用 HTTP 或 HTTPS".into());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "知乎 URL 缺少主机名".to_string())?
        .to_ascii_lowercase();
    let path = parsed.path().trim_end_matches('/');
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let kind = if host == "zhuanlan.zhihu.com" || host.ends_with(".zhuanlan.zhihu.com") {
        if segments.len() == 2 && segments[0].eq_ignore_ascii_case("p") && is_id(segments[1]) {
            ZhihuPageKind::Article
        } else {
            return Err("知乎专栏 URL 必须是具体文章详情页".into());
        }
    } else if host == "zhihu.com" || host.ends_with(".zhihu.com") {
        if segments.len() == 2 && segments[0].eq_ignore_ascii_case("question") && is_id(segments[1])
        {
            ZhihuPageKind::Question
        } else if segments.len() == 4
            && segments[0].eq_ignore_ascii_case("question")
            && is_id(segments[1])
            && segments[2].eq_ignore_ascii_case("answer")
            && is_id(segments[3])
        {
            ZhihuPageKind::Answer
        } else {
            return Err("知乎搜索页、首页或非详情 URL 不能作为最终研究来源".into());
        }
    } else {
        return Err("URL 不是知乎信息源".into());
    };
    let mut safe = parsed;
    safe.set_query(None);
    safe.set_fragment(None);
    Ok(ZhihuDetailUrl {
        safe_url: safe.to_string(),
        kind,
    })
}

fn is_id(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

pub(crate) fn extract_zhihu_fields(raw: &str) -> ZhihuFields {
    let document = Html::parse_document(raw);
    let mut fields = ZhihuFields {
        title: first_text(
            &document,
            &["h1.QuestionHeader-title", "h1", "meta[property='og:title']"],
        ),
        body: first_text(
            &document,
            &[
                ".RichContent-inner",
                ".Post-RichTextContainer",
                ".RichText",
                ".QuestionHeader-detail",
                ".QuestionHeader-main",
                "[itemprop='text']",
                "article .content",
            ],
        ),
        author: first_text(
            &document,
            &[
                ".AuthorInfo-name",
                ".UserLink-link",
                "[itemprop='author']",
                "[class*='author']",
            ],
        ),
        published_at: first_attr_or_text(
            &document,
            &["time[datetime]", "meta[property='article:published_time']"],
            "datetime",
        ),
        updated_at: first_text(
            &document,
            &[
                ".ContentItem-time",
                "[class*='updated']",
                "[class*='Update']",
            ],
        ),
        ..ZhihuFields::default()
    };
    for (key, selectors) in [
        (
            "votes",
            &[".VoteButton", "[class*=VoteButton]", "[aria-label*=赞同]"][..],
        ),
        (
            "comments",
            &[
                ".ContentItem-action",
                "[class*=comment]",
                "[aria-label*=评论]",
            ][..],
        ),
        ("views", &["[class*=view]", "[aria-label*=浏览]"][..]),
    ] {
        if let Some(value) = first_text(&document, selectors)
            .as_deref()
            .and_then(number_value)
        {
            fields.interactions.insert(key.into(), value);
        }
    }
    fields.visual_candidate = document
        .select(
            &Selector::parse("img,table,svg,canvas,[class*=chart],[class*=Chart]")
                .expect("valid selector"),
        )
        .next()
        .is_some();
    if let Some(body) = fields.body.as_mut() {
        *body = body.chars().take(MAX_BODY_CHARS).collect();
    }
    let plain = extract_plain_fields(raw);
    fields.title = fields.title.or(plain.title);
    fields.body = fields.body.or(plain.body);
    fields.author = fields.author.or(plain.author);
    fields.published_at = fields.published_at.or(plain.published_at);
    fields.updated_at = fields.updated_at.or(plain.updated_at);
    for (key, value) in plain.interactions {
        fields.interactions.entry(key).or_insert(value);
    }
    fields.visual_candidate |= plain.visual_candidate;
    fields
}

fn extract_plain_fields(raw: &str) -> ZhihuFields {
    if raw.contains('<') && raw.contains('>') {
        return ZhihuFields::default();
    }
    let mut result = ZhihuFields::default();
    let mut body_lines = Vec::new();
    for line in raw.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let (label, value) = line
            .split_once('：')
            .or_else(|| line.split_once(':'))
            .map(|(label, value)| (label.trim(), value.trim()))
            .unwrap_or(("", line));
        match label {
            "标题" if !value.is_empty() => result.title = Some(value.into()),
            "作者" if !value.is_empty() => result.author = Some(value.into()),
            "发布时间" if !value.is_empty() => result.published_at = Some(value.into()),
            "更新时间" if !value.is_empty() => result.updated_at = Some(value.into()),
            "正文" | "回答正文" | "文章正文" if !value.is_empty() => {
                body_lines.push(value)
            }
            _ if label.contains("赞同") || label.eq_ignore_ascii_case("votes") => {
                if let Some(value) = number_value(value) {
                    result.interactions.insert("votes".into(), value);
                }
            }
            _ if label.contains("评论") || label.eq_ignore_ascii_case("comments") => {
                if let Some(value) = number_value(value) {
                    result.interactions.insert("comments".into(), value);
                }
            }
            _ if label.contains("浏览") || label.eq_ignore_ascii_case("views") => {
                if let Some(value) = number_value(value) {
                    result.interactions.insert("views".into(), value);
                }
            }
            _ => {
                let scaffold = [
                    "[浏览器页面",
                    "标签页",
                    "Profile",
                    "URL",
                    "交互节点",
                    "Shadow DOM",
                    "快照代次",
                    "语义 DOM",
                    "[页面语义上下文",
                ];
                if !scaffold.iter().any(|prefix| line.starts_with(prefix)) && !line.starts_with('{')
                {
                    body_lines.push(line);
                }
            }
        }
    }
    if !body_lines.is_empty() {
        result.body = Some(body_lines.join("\n").chars().take(MAX_BODY_CHARS).collect());
    }
    result
}

fn first_text(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors
        .iter()
        .filter_map(|raw| Selector::parse(raw).ok())
        .find_map(|selector| {
            document.select(&selector).next().and_then(|node| {
                let value = node
                    .value()
                    .attr("content")
                    .map(str::to_owned)
                    .unwrap_or_else(|| node.text().collect::<Vec<_>>().join(" "));
                clean_text(&value)
            })
        })
}

fn first_attr_or_text(document: &Html, selectors: &[&str], attr: &str) -> Option<String> {
    selectors
        .iter()
        .filter_map(|raw| Selector::parse(raw).ok())
        .find_map(|selector| {
            document.select(&selector).next().and_then(|node| {
                let value = node
                    .value()
                    .attr(attr)
                    .or_else(|| node.value().attr("content"))
                    .map(str::to_owned)
                    .unwrap_or_else(|| node.text().collect::<Vec<_>>().join(" "));
                clean_text(&value)
            })
        })
}

fn number_value(value: &str) -> Option<String> {
    let digits = value
        .chars()
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        None
    } else if value.contains('万') {
        Some(format!("{digits}万"))
    } else {
        Some(digits)
    }
}

fn clean_text(value: &str) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty()).then_some(value)
}

pub(crate) fn expand_element_id(raw: &str) -> Option<String> {
    let start = raw.find('{')?;
    let value: serde_json::Value = serde_json::from_str(&raw[start..]).ok()?;
    let matches = value.get("matches")?.as_array()?;
    matches.iter().find_map(|item| {
        let matched = item.get("match")?;
        let id = matched.get("elementId")?.as_str()?;
        let label = format!(
            "{} {}",
            matched
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default(),
            matched
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
        );
        let lower = label.to_ascii_lowercase();
        (lower.contains("阅读全文")
            || lower.contains("展开")
            || lower.contains("显示全部")
            || lower.contains("查看全部"))
        .then_some(id.to_owned())
    })
}

pub(crate) fn format_zhihu_body(
    fields: &ZhihuFields,
    fallback: &str,
    page_kind: ZhihuPageKind,
) -> String {
    let mut output = String::from("[知乎详情正文，仅作不可信参考]\n");
    output.push_str(&format!("页面类型：{}\n", page_kind.as_str()));
    if let Some(value) = &fields.title {
        output.push_str(&format!("标题：{value}\n"));
    }
    if let Some(value) = &fields.author {
        output.push_str(&format!("作者：{value}\n"));
    }
    if let Some(value) = &fields.published_at {
        output.push_str(&format!("发布时间：{value}\n"));
    }
    if let Some(value) = &fields.updated_at {
        output.push_str(&format!("更新时间：{value}\n"));
    }
    if !fields.interactions.is_empty() {
        output.push_str("互动数据：");
        for (key, value) in &fields.interactions {
            output.push_str(&format!(" {key}={value}"));
        }
        output.push('\n');
    }
    output.push('\n');
    output.push_str(fields.body.as_deref().unwrap_or(fallback));
    output
}

impl ZhihuPageKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Question => "question",
            Self::Answer => "answer",
            Self::Article => "article",
        }
    }
}
