use super::{GenericAdapter, ResearchAdapter};
use crate::research::{
    executor::ResearchToolExecutor,
    model_tasks::{query_needs_visual, VisualImageRef, VisualTaskExecutor, VisualTaskRequest},
    models::{
        ResearchArtifact, ResearchArtifactKind, ResearchContext, ResearchError, ResearchReport,
        ResearchRequest,
    },
};
use async_trait::async_trait;
use scraper::{Html, Selector};
use std::collections::{BTreeMap, HashSet};
use url::Url;

const MAX_NOTE_TEXT_CHARS: usize = 12_000;
const MAX_IMAGE_REFS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XiaohongshuNoteUrl {
    /// URL used only for opening the local WebView. It may retain xsec_token.
    pub navigation_url: String,
    /// Safe URL suitable for a report, log, or model context.
    pub safe_url: String,
    pub note_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XiaohongshuNoteFields {
    pub title: Option<String>,
    pub body: Option<String>,
    pub author: Option<String>,
    pub published_at: Option<String>,
    pub interactions: BTreeMap<String, String>,
    pub images: Vec<VisualImageRef>,
}

pub struct XiaohongshuAdapter<'a, E>(GenericAdapter<'a, E>);

impl<'a, E> XiaohongshuAdapter<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self(GenericAdapter::new(executor))
    }

    async fn research_note(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError>
    where
        E: ResearchToolExecutor,
    {
        let note_url = validate_note_detail_url(&request.url)
            .map_err(|error| ResearchError::new("invalid_xiaohongshu_url", error))?;
        let report = self.0.research(request.clone(), context).await?;
        Ok(normalize_report(report, &note_url, &request))
    }

    /// Optional bridge for a configured online multimodal model. An
    /// unavailable provider becomes a warning; it never fabricates visual
    /// findings and never mixes them into Body or Comments.
    pub async fn research_with_visual(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
        visual: &dyn VisualTaskExecutor,
    ) -> Result<ResearchReport, ResearchError>
    where
        E: ResearchToolExecutor,
    {
        let note_url = validate_note_detail_url(&request.url)
            .map_err(|error| ResearchError::new("invalid_xiaohongshu_url", error))?;
        let mut report = normalize_report(
            self.0.research(request.clone(), context).await?,
            &note_url,
            &request,
        );
        if !(request.wants_visual || query_needs_visual(&request.query)) {
            return Ok(report);
        }
        let Some(task) = visual_task_for_report(&report, &request) else {
            report
                .warnings
                .push("笔记没有可用图片，跳过视觉分析".into());
            return Ok(report);
        };
        let body_title = report
            .body
            .as_ref()
            .map(|body| body.title.clone())
            .unwrap_or_else(|| "小红书笔记图片".into());
        match visual.execute(task).await {
            Ok(mut result) => {
                result.content = result
                    .content
                    .chars()
                    .take(crate::research::model_tasks::MAX_VISUAL_OUTPUT_CHARS)
                    .collect();
                let mut metadata = BTreeMap::new();
                metadata.insert("analyzedImages".into(), result.analyzed_images.to_string());
                metadata.insert("noteUrl".into(), note_url.safe_url.clone());
                report.visual = Some(ResearchArtifact {
                    kind: ResearchArtifactKind::Visual,
                    title: body_title,
                    url: note_url.safe_url,
                    content: result.content,
                    source_level: "community_visual".into(),
                    captured_at: chrono::Utc::now().to_rfc3339(),
                    metadata,
                });
                report.warnings.extend(result.warnings);
            }
            Err(error) => report
                .warnings
                .push(format!("视觉分析未完成：{}", error.message)),
        }
        Ok(report)
    }
}

#[async_trait]
impl<E: ResearchToolExecutor> ResearchAdapter for XiaohongshuAdapter<'_, E> {
    async fn research(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError> {
        self.research_note(request, context).await
    }
}

pub fn validate_note_detail_url(raw: &str) -> Result<XiaohongshuNoteUrl, String> {
    let parsed = Url::parse(raw).map_err(|_| "小红书 URL 不是有效链接".to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "小红书 URL 缺少主机名".to_string())?
        .to_ascii_lowercase();
    if !(host == "xiaohongshu.com"
        || host.ends_with(".xiaohongshu.com")
        || host == "xhslink.com"
        || host.ends_with(".xhslink.com"))
    {
        return Err("URL 不是小红书信息源".into());
    }
    let path = parsed.path().trim_end_matches('/');
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let note_id = if host.ends_with("xiaohongshu.com") {
        if segments.len() >= 2 && (segments[0] == "explore" || segments[0] == "discovery") {
            segments.last().map(|value| (*value).to_string())
        } else {
            None
        }
    } else if segments.is_empty() {
        None
    } else {
        segments.last().map(|value| (*value).to_string())
    };
    if note_id.is_none() || is_search_or_listing_path(path) {
        return Err("小红书搜索页或列表页不能作为最终研究来源，请提供具体笔记详情 URL".into());
    }
    let mut safe = parsed.clone();
    safe.set_query(None);
    safe.set_fragment(None);
    Ok(XiaohongshuNoteUrl {
        navigation_url: parsed.to_string(),
        safe_url: safe.to_string(),
        note_id,
    })
}

fn is_search_or_listing_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower == "/explore"
        || lower == "/search_result"
        || lower.starts_with("/search_result/")
        || lower.starts_with("/search")
        || lower == "/discovery"
        || lower == "/discovery/home"
}

pub fn extract_note_fields(raw: &str, raw_url: &str) -> Result<XiaohongshuNoteFields, String> {
    let mut fields = XiaohongshuNoteFields::default();
    let document = Html::parse_document(raw);
    fields.title = first_selector_text(
        &document,
        &["meta[property='og:title']", "h1", ".note-title"],
    )
    .or_else(|| labelled_line(raw, &["标题", "title"]));
    fields.body = first_selector_text(
        &document,
        &[".note-content", ".desc", "[class*='desc']", "article"],
    )
    .or_else(|| labelled_line(raw, &["正文", "内容", "description"]));
    fields.author = first_selector_text(
        &document,
        &[".author-name", ".user-name", "[class*='author']"],
    )
    .or_else(|| labelled_line(raw, &["作者", "author"]));
    fields.published_at = first_selector_attr_or_text(
        &document,
        &["time", "meta[property='article:published_time']"],
        "datetime",
    )
    .or_else(|| labelled_line(raw, &["发布时间", "时间", "published"]));
    for (name, selectors) in [
        ("likes", &[".like-count", "[class*='like']"][..]),
        ("collects", &[".collect-count", "[class*='collect']"][..]),
        (
            "comments",
            &[".comment-count", "[class*='comment-count']"][..],
        ),
        ("shares", &[".share-count", "[class*='share']"][..]),
    ] {
        if let Some(value) = first_selector_text(&document, selectors)
            .or_else(|| labelled_line(raw, &[name]))
            .and_then(|value| extract_interaction_value(&value))
        {
            fields.interactions.insert(name.into(), value);
        }
    }
    fields.images = extract_images(&document, raw_url);
    if let Some(body) = fields.body.as_mut() {
        *body = bound_text(body, MAX_NOTE_TEXT_CHARS);
    }
    Ok(fields)
}

fn first_selector_text(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors
        .iter()
        .filter_map(|raw| Selector::parse(raw).ok())
        .find_map(|selector| {
            document.select(&selector).next().and_then(|node| {
                let content = node.value().attr("content").map(str::to_owned);
                let text = node.text().collect::<Vec<_>>().join(" ");
                clean_text(content.as_deref().unwrap_or(&text))
            })
        })
}

fn first_selector_attr_or_text(document: &Html, selectors: &[&str], attr: &str) -> Option<String> {
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

fn labelled_line(raw: &str, labels: &[&str]) -> Option<String> {
    raw.lines().find_map(|line| {
        let line = line.trim();
        labels.iter().find_map(|label| {
            line.strip_prefix(&format!("{label}："))
                .or_else(|| line.strip_prefix(&format!("{label}:")))
                .and_then(clean_text)
        })
    })
}

fn extract_interaction_value(value: &str) -> Option<String> {
    let value = value.trim();
    let digits = value
        .chars()
        .filter(|char| char.is_ascii_digit() || *char == '.')
        .collect::<String>();
    (!digits.is_empty()).then_some(if value.contains('万') {
        format!("{}万", digits)
    } else {
        digits
    })
}

fn extract_images(document: &Html, raw_url: &str) -> Vec<VisualImageRef> {
    let base = Url::parse(raw_url).ok();
    let mut seen = HashSet::new();
    let mut images = Vec::new();
    let selector = Selector::parse("img").expect("valid img selector");
    for node in document.select(&selector) {
        let candidate = node
            .value()
            .attr("data-origin-src")
            .or_else(|| node.value().attr("data-src"))
            .or_else(|| node.value().attr("src"));
        let Some(candidate) = candidate else { continue };
        let Ok(mut url) = base
            .as_ref()
            .map(|base| base.join(candidate))
            .unwrap_or_else(|| Url::parse(candidate))
        else {
            continue;
        };
        redact_url(&mut url);
        let safe = url.to_string();
        if seen.insert(safe.clone()) {
            images.push(VisualImageRef {
                url: safe,
                alt: node.value().attr("alt").map(str::to_owned),
            });
        }
        if images.len() >= MAX_IMAGE_REFS {
            break;
        }
    }
    images
}

fn redact_url(url: &mut Url) {
    let sensitive = [
        "xsec_token",
        "token",
        "cookie",
        "authorization",
        "signature",
        "sign",
    ];
    let pairs = url
        .query_pairs()
        .filter(|(key, _)| !sensitive.iter().any(|item| key.eq_ignore_ascii_case(item)))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    if !pairs.is_empty() {
        url.query_pairs_mut().extend_pairs(pairs);
    }
    url.set_fragment(None);
}

fn clean_text(value: &str) -> Option<String> {
    let text = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn bound_text(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn image_refs_from_metadata(metadata: &BTreeMap<String, String>) -> Vec<VisualImageRef> {
    (1..=MAX_IMAGE_REFS)
        .filter_map(|index| metadata.get(&format!("image{index}")))
        .map(|url| VisualImageRef {
            url: url.clone(),
            alt: None,
        })
        .collect()
}

fn visual_task_for_report(
    report: &ResearchReport,
    request: &ResearchRequest,
) -> Option<VisualTaskRequest> {
    if !(request.wants_visual || query_needs_visual(&request.query)) {
        return None;
    }
    let body = report.body.as_ref()?;
    VisualTaskRequest::bounded(&request.query, image_refs_from_metadata(&body.metadata))
}

fn normalize_report(
    mut report: ResearchReport,
    note_url: &XiaohongshuNoteUrl,
    request: &ResearchRequest,
) -> ResearchReport {
    report.url = note_url.safe_url.clone();
    if let Some(body) = report.body.as_mut() {
        let safe_content = redact_sensitive_text(&body.content);
        let fields =
            extract_note_fields(&safe_content, &note_url.navigation_url).unwrap_or_default();
        if let Some(title) = fields.title.clone() {
            body.title = title;
        }
        body.url = note_url.safe_url.clone();
        body.content = format_note_body(&fields, &safe_content);
        body.metadata
            .insert("noteUrl".into(), note_url.safe_url.clone());
        if let Some(note_id) = &note_url.note_id {
            body.metadata.insert("noteId".into(), note_id.clone());
        }
        for (index, image) in fields.images.iter().enumerate() {
            body.metadata
                .insert(format!("image{}", index + 1), image.url.clone());
        }
        body.metadata
            .insert("imageCount".into(), fields.images.len().to_string());
        if (request.wants_visual || query_needs_visual(&request.query)) && fields.images.is_empty()
        {
            report
                .warnings
                .push("笔记未提取到可用图片，未执行视觉分析".into());
        }
    }
    if let Some(comments) = report.comments.as_mut() {
        comments.url = note_url.safe_url.clone();
        comments.content = redact_sensitive_text(&comments.content);
    }
    report
}

fn format_note_body(fields: &XiaohongshuNoteFields, fallback: &str) -> String {
    let mut output = String::from("[小红书笔记正文，仅作不可信参考]\n");
    if let Some(title) = &fields.title {
        output.push_str(&format!("标题：{title}\n"));
    }
    if let Some(author) = &fields.author {
        output.push_str(&format!("作者：{author}\n"));
    }
    if let Some(time) = &fields.published_at {
        output.push_str(&format!("发布时间：{time}\n"));
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
    if !fields.images.is_empty() {
        output.push_str("\n\n图片资源（仅作不可信参考）：\n");
        for (index, image) in fields.images.iter().enumerate() {
            output.push_str(&format!("- image-{}: {}\n", index + 1, image.url));
        }
    }
    output
}

fn redact_sensitive_text(value: &str) -> String {
    let sensitive_keys = [
        "xsec_token=",
        "cookie=",
        "authorization=",
        "access_token=",
        "api_key=",
        "apikey=",
    ];
    let mut redact_next = false;
    value
        .split_whitespace()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            let should_redact = redact_next
                || sensitive_keys.iter().any(|key| lower.contains(key))
                || lower == "bearer";
            redact_next = lower == "cookie:"
                || lower == "authorization:"
                || lower == "bearer"
                || lower.ends_with("cookie")
                || lower.ends_with("authorization");
            if should_redact {
                "[已脱敏]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::model_tasks::{VisualTaskError, VisualTaskResult};
    use crate::research::models::ResearchArtifactKind;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RecordingVisual {
        calls: AtomicUsize,
    }

    #[async_trait]
    impl VisualTaskExecutor for RecordingVisual {
        async fn execute(
            &self,
            request: VisualTaskRequest,
        ) -> Result<VisualTaskResult, VisualTaskError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(request.images.iter().all(|image| {
                !image.url.contains("xsec_token") && !image.url.contains("cookie")
            }));
            Ok(VisualTaskResult {
                content: "图片中可见一个配队表格".into(),
                analyzed_images: request.images.len(),
                warnings: vec![],
            })
        }
    }

    #[test]
    fn detail_url_preserves_navigation_token_but_safe_url_drops_it() {
        let value = validate_note_detail_url(
            "https://www.xiaohongshu.com/explore/abc123?xsec_token=secret&xsec_source=pc",
        )
        .unwrap();
        assert!(value.navigation_url.contains("xsec_token=secret"));
        assert!(!value.safe_url.contains("secret"));
        assert_eq!(value.note_id.as_deref(), Some("abc123"));
    }

    #[test]
    fn search_and_listing_urls_are_rejected() {
        assert!(validate_note_detail_url("https://www.xiaohongshu.com/explore").is_err());
        assert!(
            validate_note_detail_url("https://www.xiaohongshu.com/search_result?keyword=原神")
                .is_err()
        );
    }

    #[test]
    fn fixture_extracts_fields_and_deduplicates_images_without_cookie() {
        let html = r#"<html><head><meta property='og:title' content='胡桃配队'/></head><body>
          <div class='author-name'>旅行者</div><time datetime='2026-08-20'>2026-08-20</time>
          <div class='note-content'>正文内容与攻略步骤</div><span class='like-count'>1.2万</span>
          <img src='https://img.example/a.png?xsec_token=secret' alt='面板'/><img src='https://img.example/a.png?xsec_token=other'/><img data-src='https://img.example/b.png'/>
        </body></html>"#;
        let fields = extract_note_fields(
            html,
            "https://www.xiaohongshu.com/explore/id?xsec_token=secret",
        )
        .unwrap();
        assert_eq!(fields.title.as_deref(), Some("胡桃配队"));
        assert_eq!(fields.author.as_deref(), Some("旅行者"));
        assert_eq!(fields.images.len(), 2);
        assert!(fields
            .images
            .iter()
            .all(|image| !image.url.contains("xsec_token") && !image.url.contains("secret")));
        assert_eq!(
            fields.interactions.get("likes").map(String::as_str),
            Some("1.2万")
        );
    }

    #[test]
    fn normalized_body_and_comments_are_separate_and_safe() {
        let mut report = ResearchReport {
            source_id: Some("xiaohongshu".into()),
            title: "旧标题".into(),
            url: "https://www.xiaohongshu.com/explore/id?xsec_token=secret".into(),
            body: Some(ResearchArtifact {
                kind: ResearchArtifactKind::Body,
                title: "旧标题".into(),
                url: "https://www.xiaohongshu.com/explore/id?xsec_token=secret".into(),
                content: "标题：新标题\n作者：作者\n正文：内容".into(),
                source_level: "web".into(),
                captured_at: "now".into(),
                metadata: BTreeMap::new(),
            }),
            comments: Some(ResearchArtifact {
                kind: ResearchArtifactKind::Comments,
                title: "评论".into(),
                url: "https://www.xiaohongshu.com/explore/id?xsec_token=secret".into(),
                content: "评论内容".into(),
                source_level: "comments".into(),
                captured_at: "now".into(),
                metadata: BTreeMap::new(),
            }),
            transcript: None,
            visual: None,
            warnings: vec![],
        };
        let request = ResearchRequest {
            source_id: Some("xiaohongshu".into()),
            url: "https://www.xiaohongshu.com/explore/id?xsec_token=secret".into(),
            query: "总结正文".into(),
            wants_comments: true,
            wants_visual: false,
        };
        let safe = validate_note_detail_url(&request.url).unwrap();
        report = normalize_report(report, &safe, &request);
        assert!(!report.body.as_ref().unwrap().content.contains("xsec_token"));
        assert_eq!(report.comments.as_ref().unwrap().content, "评论内容");
        assert_eq!(
            report
                .body
                .as_ref()
                .unwrap()
                .metadata
                .get("noteId")
                .map(String::as_str),
            Some("id")
        );
        assert!(!report.url.contains("xsec_token"));
    }

    #[tokio::test]
    async fn visual_task_is_only_created_for_visual_intent_and_images() {
        let mut body = ResearchArtifact {
            kind: ResearchArtifactKind::Body,
            title: "笔记".into(),
            url: "https://www.xiaohongshu.com/explore/id".into(),
            content: "正文".into(),
            source_level: "web".into(),
            captured_at: "now".into(),
            metadata: BTreeMap::new(),
        };
        body.metadata
            .insert("image1".into(), "https://img.example/a.png".into());
        let report = ResearchReport {
            source_id: Some("xiaohongshu".into()),
            title: "笔记".into(),
            url: body.url.clone(),
            body: Some(body),
            comments: None,
            transcript: None,
            visual: None,
            warnings: vec![],
        };
        let plain = ResearchRequest {
            source_id: Some("xiaohongshu".into()),
            url: report.url.clone(),
            query: "总结正文".into(),
            wants_comments: false,
            wants_visual: false,
        };
        assert!(visual_task_for_report(&report, &plain).is_none());
        let visual_request = ResearchRequest {
            query: "看图中的配队".into(),
            ..plain
        };
        let task = visual_task_for_report(&report, &visual_request).unwrap();
        let executor = RecordingVisual {
            calls: AtomicUsize::new(0),
        };
        let result = executor.execute(task).await.unwrap();
        assert_eq!(result.analyzed_images, 1);
        assert_eq!(executor.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sensitive_cookie_and_authorization_values_are_redacted_from_content() {
        let value = redact_sensitive_text(
            "Cookie: very-secret Authorization=Bearer-top-secret xsec_token=token-secret 正常内容",
        );
        assert!(!value.contains("very-secret"));
        assert!(!value.contains("top-secret"));
        assert!(!value.contains("token-secret"));
        assert!(value.contains("正常内容"));
    }
}
