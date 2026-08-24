use super::*;
use crate::research::models::{
    ResearchAction, ResearchActionRequest, ResearchArtifact, ResearchArtifactKind,
};
use crate::research::zhihu_fields::{
    expand_element_id, extract_zhihu_fields, validate_zhihu_detail_url, ZhihuPageKind,
};
use async_trait::async_trait;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
};

#[test]
fn accepts_detail_kinds_and_rejects_discovery() {
    assert_eq!(
        validate_zhihu_detail_url("https://www.zhihu.com/question/123")
            .unwrap()
            .kind,
        ZhihuPageKind::Question
    );
    assert_eq!(
        validate_zhihu_detail_url("https://www.zhihu.com/question/123/answer/456?x=1#x")
            .unwrap()
            .kind,
        ZhihuPageKind::Answer
    );
    assert_eq!(
        validate_zhihu_detail_url("https://zhuanlan.zhihu.com/p/789")
            .unwrap()
            .kind,
        ZhihuPageKind::Article
    );
    for url in [
        "https://www.zhihu.com/",
        "https://www.zhihu.com/search?q=x",
        "https://www.zhihu.com/hot",
        "https://www.zhihu.com/people/a",
    ] {
        assert!(validate_zhihu_detail_url(url).is_err());
    }
}

#[test]
fn extracts_zhihu_fields_and_marks_visual_candidate() {
    let html = "<h1 class='QuestionHeader-title'>问题标题</h1><div class='AuthorInfo-name'>作者</div><time datetime='2026-08-20'></time><div class='ContentItem-time'>编辑于 2026-08-21</div><div class='RichContent-inner'>正文内容</div><button class='VoteButton'>赞同 128</button><button class='ContentItem-action'>32 条评论</button><table><tr><td>x</td></tr></table>";
    let f = extract_zhihu_fields(html);
    assert_eq!(f.title.as_deref(), Some("问题标题"));
    assert_eq!(f.author.as_deref(), Some("作者"));
    assert_eq!(f.body.as_deref(), Some("正文内容"));
    assert_eq!(f.interactions.get("votes").map(String::as_str), Some("128"));
    assert!(f.visual_candidate);
}

#[test]
fn expand_only_returns_read_only_labels() {
    assert_eq!(
        expand_element_id(r#"{"matches":[{"match":{"elementId":"safe","name":"阅读全文"}}]}"#)
            .as_deref(),
        Some("safe")
    );
    assert_eq!(
        expand_element_id(r#"{"matches":[{"match":{"elementId":"bad","name":"赞同"}}]}"#),
        None
    );
}

struct Fake {
    calls: Mutex<Vec<ResearchActionRequest>>,
    results: Mutex<VecDeque<Result<Option<ResearchArtifact>, ResearchError>>>,
}
#[async_trait]
impl ResearchToolExecutor for Fake {
    async fn execute(
        &self,
        req: ResearchActionRequest,
    ) -> Result<Option<ResearchArtifact>, ResearchError> {
        self.calls.lock().unwrap().push(req);
        self.results.lock().unwrap().pop_front().unwrap()
    }
}
fn artifact(kind: ResearchArtifactKind, content: &str, opened: bool) -> ResearchArtifact {
    let mut metadata = BTreeMap::new();
    if opened {
        metadata.insert("tabId".into(), "zhihu-tab".into());
    }
    ResearchArtifact {
        kind,
        title: "标题".into(),
        url: "https://www.zhihu.com/question/123/answer/456".into(),
        content: content.into(),
        source_level: "web".into(),
        captured_at: "now".into(),
        metadata,
    }
}
fn req(comments: bool) -> ResearchRequest {
    ResearchRequest {
        source_id: Some("zhihu".into()),
        url: "https://www.zhihu.com/question/123/answer/456".into(),
        query: "总结".into(),
        wants_comments: comments,
        wants_visual: false,
    }
}

#[tokio::test]
async fn runs_expand_scroll_extract_and_optional_comments_then_closes() {
    let fake = Fake {
        calls: Mutex::new(Vec::new()),
        results: Mutex::new(VecDeque::from([
            Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
            Ok(Some(artifact(
                ResearchArtifactKind::Body,
                r#"{"matches":[{"match":{"elementId":"expand","name":"阅读全文"}}]}"#,
                false,
            ))),
            Ok(Some(artifact(ResearchArtifactKind::Body, "clicked", false))),
            Ok(Some(artifact(ResearchArtifactKind::Body, "scroll", false))),
            Ok(Some(artifact(
                ResearchArtifactKind::Body,
                "标题：标题\n作者：作者\n正文：完整正文",
                false,
            ))),
            Ok(Some(artifact(
                ResearchArtifactKind::Comments,
                "评论",
                false,
            ))),
            Ok(None),
        ])),
    };
    let report = ZhihuAdapter::new(&fake)
        .research(req(true), ResearchContext::default())
        .await
        .unwrap();
    let calls = fake.calls.lock().unwrap();
    assert_eq!(calls.last().unwrap().action, ResearchAction::ClosePage);
    assert!(report.body.unwrap().content.contains("完整正文"));
    assert!(report.comments.is_some());
}

#[tokio::test]
async fn no_comments_action_when_not_requested() {
    let fake = Fake {
        calls: Mutex::new(Vec::new()),
        results: Mutex::new(VecDeque::from([
            Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
            Ok(Some(artifact(
                ResearchArtifactKind::Body,
                r#"{"matches":[]}"#,
                false,
            ))),
            Ok(Some(artifact(ResearchArtifactKind::Body, "scroll", false))),
            Ok(Some(artifact(
                ResearchArtifactKind::Body,
                "正文：内容",
                false,
            ))),
            Ok(None),
        ])),
    };
    ZhihuAdapter::new(&fake)
        .research(req(false), ResearchContext::default())
        .await
        .unwrap();
    assert!(!fake
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.action == ResearchAction::ExtractComments));
}

#[tokio::test]
async fn empty_body_returns_error_and_closes() {
    let fake = Fake {
        calls: Mutex::new(Vec::new()),
        results: Mutex::new(VecDeque::from([
            Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
            Ok(Some(artifact(
                ResearchArtifactKind::Body,
                r#"{"matches":[]}"#,
                false,
            ))),
            Ok(Some(artifact(ResearchArtifactKind::Body, "scroll", false))),
            Ok(Some(artifact(ResearchArtifactKind::Body, "", false))),
            Ok(None),
        ])),
    };
    assert_eq!(
        ZhihuAdapter::new(&fake)
            .research(req(false), ResearchContext::default())
            .await
            .unwrap_err()
            .code,
        "empty_body"
    );
    assert_eq!(
        fake.calls.lock().unwrap().last().unwrap().action,
        ResearchAction::ClosePage
    );
}
