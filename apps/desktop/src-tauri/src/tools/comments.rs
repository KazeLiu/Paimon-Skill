use chrono::Utc;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use super::models::SourceRecord;
use super::url_policy::validate_url;

pub const MAX_COMMENTS: usize = 100;
pub const MAX_COMMENT_CHARS: usize = 2_000;

#[derive(Debug, Clone, Copy)]
pub struct CommentLimits {
    pub max_comments: usize,
    pub max_chars: usize,
}

impl CommentLimits {
    pub const fn new(max_comments: usize, max_chars: usize) -> Self {
        Self {
            max_comments,
            max_chars,
        }
    }
}

impl Default for CommentLimits {
    fn default() -> Self {
        Self::new(MAX_COMMENTS, MAX_COMMENT_CHARS)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentRecord {
    pub author: String,
    pub content: String,
    pub likes: Option<String>,
    pub published_at: Option<String>,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CommentExtraction {
    pub adapter: String,
    pub source: SourceRecord,
    pub comments: Vec<CommentRecord>,
    pub login_challenge: bool,
    pub challenge_reason: Option<String>,
    pub truncated: bool,
}

pub fn extract_comments(
    raw_url: &str,
    html: &str,
    limits: CommentLimits,
) -> Result<CommentExtraction, String> {
    let url = validate_url(raw_url)?;
    let document = Html::parse_document(html);
    let (adapter, selectors) = adapter_for(&url);
    let mut comments = Vec::new();
    let mut truncated = false;

    for selector in selectors
        .container
        .iter()
        .filter_map(|value| Selector::parse(value).ok())
    {
        for node in document.select(&selector) {
            if comments.len() >= limits.max_comments {
                truncated = true;
                break;
            }
            let author = first_field(node, selectors.author).unwrap_or_else(|| "匿名用户".into());
            let Some(mut content) = first_field(node, selectors.content) else {
                continue;
            };
            if content.is_empty() {
                continue;
            }
            if content.chars().count() > limits.max_chars {
                content = content.chars().take(limits.max_chars).collect();
                truncated = true;
            }
            comments.push(CommentRecord {
                author,
                content,
                likes: first_field(node, selectors.likes)
                    .map(|value| extract_number(&value).unwrap_or(value)),
                published_at: first_attr_or_field(node, selectors.time),
                source_url: url.to_string(),
            });
        }
        if !comments.is_empty() || truncated {
            break;
        }
    }

    let body_text =
        clean_text(&document.root_element().text().collect::<Vec<_>>().join(" ")).to_lowercase();
    let challenge_reason = [
        ("请先登录", "请先登录"),
        ("扫码登录", "扫码登录"),
        ("验证码", "验证码"),
        ("安全验证", "安全验证"),
        ("captcha", "captcha"),
        ("verify you are human", "verify you are human"),
        ("sign in to continue", "sign in to continue"),
    ]
    .iter()
    .find(|(term, _)| body_text.contains(term))
    .map(|(_, reason)| (*reason).to_string());
    let title = document
        .select(&Selector::parse("title").expect("valid selector"))
        .next()
        .map(|node| clean_text(&node.text().collect::<Vec<_>>().join(" ")))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| url.host_str().unwrap_or("评论").into());
    Ok(CommentExtraction {
        adapter: adapter.into(),
        source: SourceRecord {
            title,
            url: url.to_string(),
            source_level: "community_comments".into(),
            platform: adapter.into(),
            captured_at: Utc::now().to_rfc3339(),
        },
        comments,
        login_challenge: challenge_reason.is_some(),
        challenge_reason,
        truncated,
    })
}

struct AdapterSelectors {
    container: &'static [&'static str],
    author: &'static [&'static str],
    content: &'static [&'static str],
    likes: &'static [&'static str],
    time: &'static [&'static str],
}

fn adapter_for(url: &Url) -> (&'static str, AdapterSelectors) {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host.ends_with("bilibili.com") {
        return (
            "bilibili",
            AdapterSelectors {
                container: &[".reply-item", ".reply-wrap", "[data-comment-id]"],
                author: &[".user-name", ".user-name a", "[class*=user]"],
                content: &[".reply-content", ".root-reply", "[class*=content]"],
                likes: &[".reply-like", ".like", "[class*=like]"],
                time: &[".reply-time", "time", ".time"],
            },
        );
    }
    if host.ends_with("xiaohongshu.com") || host.ends_with("xhslink.com") {
        return (
            "xiaohongshu",
            AdapterSelectors {
                container: &[".comment-item", ".note-comment", "[class*=comment-item]"],
                author: &[".author", ".user-name", "[class*=author]"],
                content: &[".comment-content", ".content", "[class*=comment-content]"],
                likes: &[".like", ".like-count", "[class*=like]"],
                time: &["time", ".time", ".date"],
            },
        );
    }
    if host.ends_with("zhihu.com") {
        return (
            "zhihu",
            AdapterSelectors {
                container: &[".CommentItem", ".CommentItemV2", "[class*=CommentItem]"],
                author: &[".UserLink-link", ".AuthorInfo-name", "[class*=UserLink]"],
                content: &[
                    ".RichContent-inner",
                    ".CommentItem-content",
                    "[class*=content]",
                ],
                likes: &[".VoteButton", "[class*=VoteButton]", "[class*=vote]"],
                time: &["time", ".ContentItem-time", "[class*=time]"],
            },
        );
    }
    (
        "generic",
        AdapterSelectors {
            container: &[
                "[data-comment]",
                ".comment",
                ".comments > article",
                "article.comment",
                "[class*=comment]",
            ],
            author: &[
                "[data-author]",
                ".author",
                ".username",
                "[class*=author]",
                "[class*=user]",
            ],
            content: &[
                "[data-content]",
                ".content",
                ".comment-content",
                "[class*=content]",
                "p",
            ],
            likes: &["[data-likes]", ".likes", ".like", "[class*=like]"],
            time: &["time", "[datetime]", ".time", ".date"],
        },
    )
}

fn first_field(node: ElementRef<'_>, selectors: &[&str]) -> Option<String> {
    selectors
        .iter()
        .filter_map(|selector| Selector::parse(selector).ok())
        .find_map(|selector| {
            node.select(&selector)
                .next()
                .map(|child| clean_text(&child.text().collect::<Vec<_>>().join(" ")))
                .filter(|text| !text.is_empty())
        })
}

fn first_attr_or_field(node: ElementRef<'_>, selectors: &[&str]) -> Option<String> {
    selectors
        .iter()
        .filter_map(|selector| Selector::parse(selector).ok())
        .find_map(|selector| {
            node.select(&selector)
                .next()
                .and_then(|child| {
                    child
                        .value()
                        .attr("datetime")
                        .or_else(|| child.value().attr("title"))
                        .map(str::to_owned)
                        .or_else(|| Some(clean_text(&child.text().collect::<Vec<_>>().join(" "))))
                })
                .filter(|text| !text.is_empty())
        })
}

fn extract_number(value: &str) -> Option<String> {
    let number = value
        .chars()
        .filter(|char| char.is_ascii_digit())
        .collect::<String>();
    (!number.is_empty()).then_some(number)
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_fixture_extracts_bounded_comment_fields() {
        let html = r#"
            <html><head><title>讨论页</title></head><body>
              <article class="comment" data-comment="1">
                <span class="author">小明</span><time datetime="2026-08-19">昨天</time>
                <p class="content">这是一条评论</p><span class="likes">12</span>
              </article>
              <article class="comment" data-comment="1">
                <span class="author">小红</span><p class="content">第二条</p>
              </article>
            </body></html>
        "#;
        let result =
            extract_comments("https://example.com/post", html, CommentLimits::new(1, 5)).unwrap();
        assert_eq!(result.adapter, "generic");
        assert_eq!(result.comments.len(), 1);
        assert_eq!(result.comments[0].author, "小明");
        assert_eq!(result.comments[0].content, "这是一条评");
        assert_eq!(result.comments[0].likes.as_deref(), Some("12"));
        assert_eq!(
            result.comments[0].published_at.as_deref(),
            Some("2026-08-19")
        );
        assert!(result.truncated);
    }

    #[test]
    fn platform_adapters_use_domain_specific_boundaries_and_fallbacks() {
        let bilibili = r#"<div class="reply-item"><a class="user-name">阿B</a><div class="reply-content">视频评论</div><span class="reply-time">刚刚</span><span class="reply-like">8</span></div>"#;
        let result = extract_comments(
            "https://www.bilibili.com/video/BV1",
            bilibili,
            CommentLimits::default(),
        )
        .unwrap();
        assert_eq!(result.adapter, "bilibili");
        assert_eq!(result.comments[0].author, "阿B");
        assert_eq!(result.comments[0].content, "视频评论");

        let xhs = r#"<div class="comment-item"><span class="author">笔记用户</span><div class="comment-content">笔记评论</div><span class="like">3</span></div>"#;
        let result = extract_comments(
            "https://www.xiaohongshu.com/explore/abc",
            xhs,
            CommentLimits::default(),
        )
        .unwrap();
        assert_eq!(result.adapter, "xiaohongshu");
        assert_eq!(result.comments[0].content, "笔记评论");

        let zhihu = r#"<div class="CommentItem"><a class="UserLink-link">知乎用户</a><div class="RichContent-inner">知乎评论</div><button class="VoteButton">赞同 9</button><time>2026-08-19</time></div>"#;
        let result = extract_comments(
            "https://www.zhihu.com/question/1",
            zhihu,
            CommentLimits::default(),
        )
        .unwrap();
        assert_eq!(result.adapter, "zhihu");
        assert_eq!(result.comments[0].likes.as_deref(), Some("9"));
    }

    #[test]
    fn challenge_detection_matches_page_snapshot_semantics() {
        let result = extract_comments(
            "https://example.com",
            "<body><h1>请先登录后查看评论</h1></body>",
            CommentLimits::default(),
        )
        .unwrap();
        assert!(result.login_challenge);
        assert_eq!(result.challenge_reason.as_deref(), Some("请先登录"));
    }
}
