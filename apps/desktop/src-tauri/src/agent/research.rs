use std::collections::HashSet;

use crate::tools::registry::ToolResult;

const SEARCH_TERMS: &[&str] = &["评论", "口碑", "讨论", "反馈", "评价"];

#[derive(Debug, Default, Clone)]
pub struct ResearchLedger {
    query: String,
    wiki_searches: usize,
    wiki_details: usize,
    social_search_pages: HashSet<String>,
    social_detail_pages: HashSet<String>,
    social_detail_scrolled: HashSet<String>,
    blocked_pages: HashSet<String>,
    comments_read: usize,
    comments_blocked: usize,
}

impl ResearchLedger {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Self::default()
        }
    }

    pub fn observe(&mut self, result: &ToolResult) {
        match result.tool_name.as_str() {
            "search_bwiki" => self.wiki_searches += 1,
            "read_bwiki_page" => self.wiki_details += 1,
            _ => {}
        }
        let Some(source) = result.source.as_ref() else {
            return;
        };
        let Some(platform) = social_platform(&source.url) else {
            return;
        };
        let page_key = detail_key(&source.url);
        if result.content.contains("[页面提示] 检测到登录或验证码：") {
            self.blocked_pages.insert(page_key.clone());
        }
        match result.tool_name.as_str() {
            "browser_open_page" => {
                if !is_detail_url(&source.url, platform) {
                    self.social_search_pages.insert(page_key.clone());
                }
            }
            "browser_extract_comments" => {
                if result.content.contains("[评论提取已停止]")
                    || result.content.contains("检测到登录或验证码")
                {
                    self.comments_blocked += 1;
                } else {
                    self.comments_read += 1;
                }
            }
            _ => {}
        }
        if is_detail_url(&source.url, platform) {
            self.social_detail_pages.insert(page_key.clone());
            if result.tool_name == "browser_scroll_page" {
                self.social_detail_scrolled.insert(page_key);
            }
        }
    }

    pub fn final_gate(&self) -> Option<String> {
        if self.wiki_searches > 0 && self.wiki_details == 0 {
            return Some("研究编排要求：BWiki 搜索结果只能用于发现候选条目。请调用 read_bwiki_page 读取至少一个具体条目的正文后再回答。".into());
        }
        if self
            .social_search_pages
            .iter()
            .any(|url| !self.blocked_pages.contains(url))
            && self.social_detail_pages.is_empty()
        {
            return Some("研究编排要求：你打开了社交平台搜索页，但搜索摘要不能作为证据。请从搜索结果中选择至少一个具体详情页，用 browser_open_page 打开并读取详情后再回答。".into());
        }
        if self.social_detail_pages.iter().any(|url| {
            !self.social_detail_scrolled.contains(url) && !self.blocked_pages.contains(url)
        }) {
            return Some("研究编排要求：请对已打开的社交平台详情页调用 browser_scroll_page，等待动态内容加载并再次读取页面，然后再回答。".into());
        }
        if self.requires_comments()
            && !self.social_detail_pages.is_empty()
            && self.comments_read == 0
            && self.comments_blocked == 0
            && self
                .social_detail_pages
                .iter()
                .any(|url| !self.blocked_pages.contains(url))
        {
            return Some("研究编排要求：用户询问了评论、口碑、讨论或反馈。请在详情页调用 browser_extract_comments；如果页面要求登录或验证码，调用该工具确认阻断后再说明降级。".into());
        }
        None
    }

    fn requires_comments(&self) -> bool {
        SEARCH_TERMS.iter().any(|term| self.query.contains(term))
    }
}

fn social_platform(url: &str) -> Option<&'static str> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_ascii_lowercase();
    if host.ends_with("xiaohongshu.com") || host.ends_with("xhslink.com") {
        Some("xiaohongshu")
    } else if host.ends_with("bilibili.com") {
        Some("bilibili")
    } else if host.ends_with("zhihu.com") {
        Some("zhihu")
    } else if host.ends_with("reddit.com") {
        Some("reddit")
    } else {
        None
    }
}

pub fn requires_browser_session(url: &str) -> bool {
    social_platform(url).is_some()
}

fn is_detail_url(url: &str, platform: &str) -> bool {
    let path = url::Url::parse(url)
        .ok()
        .map(|url| url.path().to_ascii_lowercase())
        .unwrap_or_default();
    match platform {
        "xiaohongshu" => {
            (path.starts_with("/explore/") && path.trim_end_matches('/').matches('/').count() >= 2)
                || path.contains("/discovery/item/")
                || path.contains("/note/")
                || path.contains("/notes/")
        }
        "bilibili" => path.contains("/video/") || path.contains("/read/cv"),
        "zhihu" => path.contains("/question/") || path.contains("/p/") || path.contains("/answer/"),
        "reddit" => {
            path.contains("/comments/") || path.contains("/r/") && path.split('/').count() > 4
        }
        _ => false,
    }
}

fn detail_key(url: &str) -> String {
    url::Url::parse(url)
        .map(|mut parsed| {
            parsed.set_query(None);
            parsed.set_fragment(None);
            parsed.to_string()
        })
        .unwrap_or_else(|_| url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::models::{PageLink, SourceRecord};

    fn result(tool_name: &str, url: &str, content: &str) -> ToolResult {
        ToolResult {
            tool_name: tool_name.into(),
            content: content.into(),
            source: Some(SourceRecord {
                title: "页面".into(),
                url: url.into(),
                source_level: "browser_webview".into(),
                platform: "test".into(),
                captured_at: "now".into(),
            }),
            links: Vec::<PageLink>::new(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn search_page_cannot_be_final_evidence_without_detail_page() {
        let mut ledger = ResearchLedger::new("配装评价");
        ledger.observe(&result(
            "browser_open_page",
            "https://www.bilibili.com/search?keyword=胡桃",
            "",
        ));
        assert!(ledger.final_gate().unwrap().contains("详情页"));
    }

    #[test]
    fn detail_page_requires_dynamic_scroll_before_completion() {
        let mut ledger = ResearchLedger::new("配装");
        ledger.observe(&result(
            "browser_open_page",
            "https://www.bilibili.com/video/BV1abc",
            "",
        ));
        assert!(ledger.final_gate().unwrap().contains("browser_scroll_page"));
        ledger.observe(&result(
            "browser_scroll_page",
            "https://www.bilibili.com/video/BV1abc",
            "",
        ));
        assert!(ledger.final_gate().is_none());
    }

    #[test]
    fn comment_intent_requires_comment_extraction_or_block_confirmation() {
        let mut ledger = ResearchLedger::new("这个视频评论怎么样");
        ledger.observe(&result(
            "browser_open_page",
            "https://www.bilibili.com/video/BV1abc",
            "",
        ));
        ledger.observe(&result(
            "browser_scroll_page",
            "https://www.bilibili.com/video/BV1abc",
            "",
        ));
        assert!(ledger
            .final_gate()
            .unwrap()
            .contains("browser_extract_comments"));
        ledger.observe(&result(
            "browser_extract_comments",
            "https://www.bilibili.com/video/BV1abc",
            "[评论提取已停止]\n检测到登录或验证码",
        ));
        assert!(ledger.final_gate().is_none());
    }

    #[test]
    fn non_social_pages_are_not_blocked_by_social_research_rules() {
        let mut ledger = ResearchLedger::new("配装");
        ledger.observe(&result(
            "browser_open_page",
            "https://wiki.example.com/guide",
            "",
        ));
        assert!(ledger.final_gate().is_none());
    }

    #[test]
    fn bilibili_search_host_and_social_homepages_are_discovery_pages() {
        let mut ledger = ResearchLedger::new("配装");
        ledger.observe(&result(
            "browser_open_page",
            "https://search.bilibili.com/all?keyword=胡桃",
            "",
        ));
        assert!(ledger.final_gate().unwrap().contains("详情页"));

        let mut homepage = ResearchLedger::new("配装");
        homepage.observe(&result(
            "browser_open_page",
            "https://www.xiaohongshu.com/explore",
            "",
        ));
        assert!(homepage.final_gate().unwrap().contains("详情页"));
    }

    #[test]
    fn login_challenge_allows_an_explicit_degraded_answer() {
        let mut ledger = ResearchLedger::new("小红书评论");
        ledger.observe(&result(
            "browser_open_page",
            "https://www.xiaohongshu.com/explore",
            "[页面提示] 检测到登录或验证码：检测到登录按钮",
        ));
        assert!(ledger.final_gate().is_none());
    }
}
