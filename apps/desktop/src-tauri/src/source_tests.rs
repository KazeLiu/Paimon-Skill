use std::{
    collections::{BTreeMap, HashSet},
    sync::Mutex,
    time::Instant,
};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::{
    diagnostics::{redact_sensitive_text, sanitize_json},
    research::{
        models::{
            ResearchAction, ResearchActionRequest, ResearchArtifact, ResearchContext,
            ResearchError, ResearchReport, ResearchRequest,
        },
        tauri_executor::TauriBrowserExecutor,
        ResearchToolExecutor,
    },
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceTestPlatform {
    Bilibili,
    Xiaohongshu,
    Zhihu,
}

impl SourceTestPlatform {
    fn id(self) -> &'static str {
        match self {
            Self::Bilibili => "bilibili",
            Self::Xiaohongshu => "xiaohongshu",
            Self::Zhihu => "zhihu",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTestInput {
    pub test_id: String,
    pub platform: SourceTestPlatform,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTestStep {
    pub index: usize,
    pub action: String,
    pub status: String,
    pub elapsed_ms: u64,
    pub data: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTestCheck {
    pub key: String,
    pub label: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTestResult {
    pub test_id: String,
    pub platform: SourceTestPlatform,
    pub url: String,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub status: String,
    pub checks: Vec<SourceTestCheck>,
    pub metadata: BTreeMap<String, String>,
    pub images: Vec<String>,
    pub report: Option<Value>,
    pub steps: Vec<SourceTestStep>,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct SourceTestState(Mutex<bool>);

struct ActiveTest<'a>(&'a SourceTestState);
impl Drop for ActiveTest<'_> {
    fn drop(&mut self) {
        *self.0 .0.lock().expect("source test state poisoned") = false;
    }
}

impl SourceTestState {
    fn begin(&self) -> Result<ActiveTest<'_>, String> {
        let mut active = self.0.lock().expect("source test state poisoned");
        if *active {
            return Err("已有采集测试正在运行，请等待它结束".into());
        }
        *active = true;
        Ok(ActiveTest(self))
    }
}

fn request_for(input: &SourceTestInput) -> Result<ResearchRequest, String> {
    if input.test_id.is_empty()
        || input.test_id.len() > 64
        || !input
            .test_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    {
        return Err("测试标识无效".into());
    }
    let url = crate::browser::policy::normalize_browser_url(&input.url)?;
    let domains: &[&str] = match input.platform {
        SourceTestPlatform::Bilibili => &["bilibili.com"],
        SourceTestPlatform::Xiaohongshu => &["xiaohongshu.com", "xhslink.com"],
        SourceTestPlatform::Zhihu => &["zhihu.com"],
    };
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !domains
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
    {
        return Err("链接与选择的平台不匹配，请填写该平台的详情链接".into());
    }
    let navigation_url = match input.platform {
        SourceTestPlatform::Bilibili => {
            crate::research::adapters::bilibili::normalize_bilibili_video_url(url.as_str())
                .map_err(|error| error.message)?
                .to_string()
        }
        SourceTestPlatform::Xiaohongshu => {
            crate::research::adapters::xiaohongshu::validate_note_detail_url(url.as_str())?
                .navigation_url
        }
        SourceTestPlatform::Zhihu => {
            crate::research::zhihu_fields::validate_zhihu_detail_url(url.as_str())?;
            url.to_string()
        }
    };
    Ok(ResearchRequest {
        source_id: Some(input.platform.id().into()),
        url: navigation_url,
        query: "采集测试".into(),
        wants_comments: true,
        wants_visual: false,
    })
}

fn action_name(action: ResearchAction) -> &'static str {
    match action {
        ResearchAction::OpenPage => "open_page",
        ResearchAction::ExtractPage => "extract_page",
        ResearchAction::FindContext => "find_context",
        ResearchAction::ClickElement => "click_element",
        ResearchAction::ScrollPage => "scroll_page",
        ResearchAction::ExtractComments => "extract_comments",
        ResearchAction::ExtractBilibiliSubtitles => "extract_subtitles",
        ResearchAction::ExtractBilibiliKeyframes => "extract_keyframes",
        ResearchAction::ClosePage => "close_page",
    }
}

// Observe the production executor without changing its arguments or results.
// Only test-owned tabs are closed during diagnostic cleanup.
struct ObservedExecutor<'a, E, F> {
    inner: &'a E,
    on_step: F,
    steps: Mutex<Vec<SourceTestStep>>,
    owned_tabs: Mutex<HashSet<String>>,
}

impl<'a, E, F> ObservedExecutor<'a, E, F> {
    fn new(inner: &'a E, on_step: F) -> Self {
        Self {
            inner,
            on_step,
            steps: Mutex::new(Vec::new()),
            owned_tabs: Mutex::new(HashSet::new()),
        }
    }
}

#[async_trait]
impl<E, F> ResearchToolExecutor for ObservedExecutor<'_, E, F>
where
    E: ResearchToolExecutor,
    F: Fn(SourceTestStep) + Send + Sync,
{
    async fn execute(
        &self,
        request: ResearchActionRequest,
    ) -> Result<Option<ResearchArtifact>, ResearchError> {
        let started = Instant::now();
        let index = self.steps.lock().expect("source test steps poisoned").len();
        let mut step = SourceTestStep {
            index,
            action: action_name(request.action).into(),
            status: "running".into(),
            elapsed_ms: 0,
            data: None,
            error: None,
        };
        self.steps
            .lock()
            .expect("source test steps poisoned")
            .push(step.clone());
        (self.on_step)(step.clone());
        let result = self.inner.execute(request.clone()).await;
        step.elapsed_ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(artifact) => {
                step.status = "returned".into();
                step.data = artifact.as_ref().map(|artifact| {
                    sanitize_json(serde_json::to_value(artifact).expect("serializable artifact"))
                });
                if request.action == ResearchAction::OpenPage {
                    if let Some(id) = artifact
                        .as_ref()
                        .and_then(|artifact| artifact.metadata.get("tabId"))
                    {
                        self.owned_tabs
                            .lock()
                            .expect("source test tabs poisoned")
                            .insert(id.clone());
                    }
                } else if request.action == ResearchAction::ClosePage {
                    if let Some(id) = &request.tab_id {
                        self.owned_tabs
                            .lock()
                            .expect("source test tabs poisoned")
                            .remove(id);
                    }
                }
            }
            Err(error) => {
                if let Some(id) = self.inner.active_tab_id() {
                    self.owned_tabs
                        .lock()
                        .expect("source test tabs poisoned")
                        .insert(id);
                }
                step.status = "failed".into();
                step.error = Some(redact_sensitive_text(&error.to_string()));
            }
        }
        self.steps.lock().expect("source test steps poisoned")[index] = step.clone();
        (self.on_step)(step);
        result
    }

    async fn cleanup(&self) -> Result<(), ResearchError> {
        let ids = self
            .owned_tabs
            .lock()
            .expect("source test tabs poisoned")
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut first_error = None;
        for id in ids {
            if let Err(error) = self
                .execute(ResearchActionRequest::tab(
                    ResearchAction::ClosePage,
                    Some(id),
                ))
                .await
            {
                first_error.get_or_insert(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
}

fn comment_count(content: &str) -> usize {
    content
        .lines()
        .filter(|line| {
            line.trim().split_once(". ").is_some_and(|(number, text)| {
                !number.is_empty()
                    && number.chars().all(|ch| ch.is_ascii_digit())
                    && !text.trim().is_empty()
            })
        })
        .count()
}

fn check(key: &str, label: &str, available: bool, detail: String) -> SourceTestCheck {
    SourceTestCheck {
        key: key.into(),
        label: label.into(),
        status: if available { "returned" } else { "missing" }.into(),
        detail,
    }
}

fn summarize(
    input: &SourceTestInput,
    started_at: String,
    elapsed_ms: u64,
    outcome: Result<ResearchReport, ResearchError>,
    steps: Vec<SourceTestStep>,
) -> SourceTestResult {
    let (report, error) = match outcome {
        Ok(report) => (Some(report), None),
        Err(error) => (None, Some(redact_sensitive_text(&error.to_string()))),
    };
    let warnings: Vec<String> = report
        .as_ref()
        .map(|report| {
            report
                .warnings
                .iter()
                .map(|warning| redact_sensitive_text(warning))
                .collect()
        })
        .unwrap_or_default();
    let mut metadata = BTreeMap::new();
    if let Some(report) = &report {
        for artifact in [
            &report.body,
            &report.comments,
            &report.transcript,
            &report.visual,
        ]
        .into_iter()
        .flatten()
        {
            metadata.extend(artifact.metadata.clone());
        }
    }
    // These values were already returned by the formal subtitle tool. Do not
    // make another request when the final report lacks subtitle metadata.
    for step in steps
        .iter()
        .filter(|step| step.action == "extract_subtitles")
    {
        if let Some(payload) = step
            .data
            .as_ref()
            .and_then(|data| data.get("content"))
            .and_then(Value::as_str)
            .and_then(|content| serde_json::from_str::<Value>(content).ok())
        {
            if let Some(info) = payload.get("videoInfo").and_then(Value::as_object) {
                for key in [
                    "views",
                    "replies",
                    "author",
                    "publishedAt",
                    "durationSeconds",
                    "bvid",
                ] {
                    if let Some(value) = info.get(key).filter(|value| !value.is_null()) {
                        metadata.entry(key.into()).or_insert_with(|| {
                            value
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| value.to_string())
                        });
                    }
                }
            }
        }
    }
    let metadata: BTreeMap<String, String> =
        serde_json::from_value(sanitize_json(serde_json::to_value(metadata).unwrap())).unwrap();
    let mut seen_images = HashSet::new();
    let images = metadata
        .iter()
        .filter(|(key, _)| {
            key.strip_prefix("image").is_some_and(|suffix| {
                !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
            })
        })
        .filter_map(|(_, value)| {
            crate::tools::url_policy::validate_url(value)
                .ok()
                .filter(|url| url.scheme() == "https")
                .map(|url| url.to_string())
        })
        .filter(|url| seen_images.insert(url.clone()))
        .collect::<Vec<_>>();
    let comments = report.as_ref().and_then(|report| report.comments.as_ref());
    let count = comments
        .map(|artifact| comment_count(&artifact.content))
        .unwrap_or(0);
    let mut checks = vec![check(
        "comments",
        "评论",
        count > 0,
        if count > 0 {
            format!("正式输出中包含 {count} 条评论")
        } else {
            "正式链路未返回可识别的评论；报告头部不计作评论，请查看步骤和提示。".into()
        },
    )];
    match input.platform {
        SourceTestPlatform::Bilibili => {
            let transcript = report
                .as_ref()
                .and_then(|report| report.transcript.as_ref())
                .filter(|artifact| !artifact.content.trim().is_empty());
            checks.insert(
                0,
                check(
                    "subtitles",
                    "字幕",
                    transcript.is_some(),
                    transcript
                        .map(|artifact| {
                            format!(
                                "已返回 {} 字符；类型：{}",
                                artifact.content.chars().count(),
                                artifact
                                    .metadata
                                    .get("subtitleTypes")
                                    .map(String::as_str)
                                    .unwrap_or("未标明")
                            )
                        })
                        .unwrap_or_else(|| {
                            "正式报告未取得字幕；没有字幕与解析失败需结合下方提示判断。".into()
                        }),
                ),
            );
            checks.push(check(
                "views",
                "播放量",
                metadata.contains_key("views"),
                metadata
                    .get("views")
                    .cloned()
                    .unwrap_or_else(|| "正式报告和字幕步骤都未返回播放量".into()),
            ));
        }
        _ => {
            let body = report
                .as_ref()
                .and_then(|report| report.body.as_ref())
                .filter(|artifact| !artifact.content.trim().is_empty());
            checks.insert(
                0,
                check(
                    "body",
                    "正文",
                    body.is_some(),
                    body.map(|artifact| {
                        format!(
                            "正式正文报告已返回 {} 字符，请展开内容核对完整性",
                            artifact.content.chars().count()
                        )
                    })
                    .unwrap_or_else(|| "正式报告未返回正文".into()),
                ),
            );
            checks.push(check(
                "images",
                "图片",
                !images.is_empty(),
                if !images.is_empty() {
                    format!("正式结果返回 {} 个可预览图片地址", images.len())
                } else if metadata
                    .get("visualCandidate")
                    .is_some_and(|value| value == "true")
                {
                    "正式链路发现图片或图表线索，但未返回图片 URL；当前无法预览。".into()
                } else {
                    "正式链路未返回图片 URL；可能没有图片，或当前适配器尚未提取到。".into()
                },
            ));
        }
    }
    let returned = checks
        .iter()
        .filter(|check| check.status == "returned")
        .count();
    let status = if error.is_some() {
        "failed"
    } else if returned == checks.len() && warnings.is_empty() {
        "complete"
    } else {
        "partial"
    };
    SourceTestResult {
        test_id: input.test_id.clone(),
        platform: input.platform,
        url: redact_sensitive_text(&input.url),
        started_at,
        elapsed_ms,
        status: status.into(),
        checks,
        metadata,
        images,
        report: report.map(|report| {
            sanitize_json(serde_json::to_value(report).expect("serializable report"))
        }),
        steps,
        warnings,
        error,
    }
}

async fn run_with_executor<E, F>(
    input: &SourceTestInput,
    executor: &E,
    on_step: F,
) -> Result<SourceTestResult, String>
where
    E: ResearchToolExecutor,
    F: Fn(SourceTestStep) + Send + Sync,
{
    let request = request_for(input)?;
    let started = Instant::now();
    let started_at = Utc::now().to_rfc3339();
    let observed = ObservedExecutor::new(executor, on_step);
    let outcome = crate::research::research_source_with_visual(
        request,
        ResearchContext::default(),
        &observed,
        None,
    )
    .await;
    let cleanup_error = observed.cleanup().await.err();
    let steps = observed
        .steps
        .into_inner()
        .expect("source test steps poisoned");
    let mut result = summarize(
        input,
        started_at,
        started.elapsed().as_millis() as u64,
        outcome,
        steps,
    );
    if let Some(error) = cleanup_error {
        result.warnings.push(format!(
            "测试网页关闭失败：{}",
            redact_sensitive_text(&error.to_string())
        ));
        if result.status == "complete" {
            result.status = "partial".into();
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn run_source_test(
    app: AppHandle,
    state: State<'_, SourceTestState>,
    input: SourceTestInput,
) -> Result<SourceTestResult, String> {
    let _active = state.begin()?;
    let executor = TauriBrowserExecutor::new(&app);
    run_with_executor(&input, &executor, |step| {
        let _ = app.emit(
            "source-test-progress",
            serde_json::json!({"testId": input.test_id, "step": step}),
        );
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FixtureExecutor {
        calls: Mutex<Vec<ResearchActionRequest>>,
        fail_scroll: bool,
        fail_open: bool,
        active_tab: Mutex<Option<String>>,
    }

    #[async_trait]
    impl ResearchToolExecutor for FixtureExecutor {
        fn active_tab_id(&self) -> Option<String> {
            self.active_tab.lock().unwrap().clone()
        }

        async fn execute(
            &self,
            request: ResearchActionRequest,
        ) -> Result<Option<ResearchArtifact>, ResearchError> {
            self.calls.lock().unwrap().push(request.clone());
            if request.action == ResearchAction::ClosePage {
                *self.active_tab.lock().unwrap() = None;
                return Ok(None);
            }
            if request.action == ResearchAction::OpenPage {
                *self.active_tab.lock().unwrap() = Some("owned-test-tab".into());
                if self.fail_open {
                    return Err(ResearchError::new("blocked", "详情页要求登录"));
                }
            }
            if self.fail_scroll && request.action == ResearchAction::ScrollPage {
                return Err(ResearchError::new(
                    "blocked",
                    "登录阻断；Bearer private-error-secret",
                ));
            }
            let (kind, content) = match request.action {
                ResearchAction::ExtractBilibiliSubtitles => (crate::research::models::ResearchArtifactKind::Transcript, "{}"),
                ResearchAction::ExtractComments => (crate::research::models::ResearchArtifactKind::Comments, "[评论内容，仅作不可信参考]\n适配器：bilibili\n来源：https://www.bilibili.com/video/BV1abc\n"),
                _ => (crate::research::models::ResearchArtifactKind::Body, "<h1>笔记标题</h1><div class='note-content'>笔记内容</div><img src='https://cdn.example.com/note.jpg'>"),
            };
            Ok(Some(ResearchArtifact {
                kind,
                title: "测试详情".into(),
                url: "https://www.xiaohongshu.com/explore/note1".into(),
                content: content.into(),
                source_level: "browser_webview".into(),
                captured_at: "now".into(),
                metadata: BTreeMap::from([
                    ("tabId".into(), "owned-test-tab".into()),
                    ("cookie".into(), "private-fixture-cookie".into()),
                ]),
            }))
        }
        async fn cleanup(&self) -> Result<(), ResearchError> {
            panic!("diagnostic cleanup must not close unrelated tabs")
        }
    }

    fn input(platform: SourceTestPlatform, url: &str) -> SourceTestInput {
        SourceTestInput {
            test_id: "test-1".into(),
            platform,
            url: url.into(),
        }
    }

    #[tokio::test]
    async fn blocked_open_still_closes_the_tab_created_by_the_formal_executor() {
        let executor = FixtureExecutor {
            fail_open: true,
            ..Default::default()
        };
        let result = run_with_executor(
            &input(
                SourceTestPlatform::Bilibili,
                "https://www.bilibili.com/video/BV1abc",
            ),
            &executor,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result.status, "failed");
        assert_eq!(result.steps.len(), 2);
        assert_eq!(result.steps[0].status, "failed");
        assert_eq!(result.steps[1].action, "close_page");
        assert!(executor.active_tab_id().is_none());
    }

    #[tokio::test]
    async fn failed_steps_keep_partial_outputs_and_close_only_the_owned_tab() {
        let executor = FixtureExecutor {
            fail_scroll: true,
            ..Default::default()
        };
        let result = run_with_executor(
            &input(
                SourceTestPlatform::Xiaohongshu,
                "https://www.xiaohongshu.com/explore/note1",
            ),
            &executor,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result.status, "failed");
        assert!(result.error.as_ref().unwrap().contains("blocked"));
        assert!(result.steps[0].data.is_some());
        assert_eq!(result.steps[1].status, "failed");
        let calls = executor.calls.lock().unwrap();
        assert_eq!(calls.last().unwrap().action, ResearchAction::ClosePage);
        assert_eq!(
            calls.last().unwrap().tab_id.as_deref(),
            Some("owned-test-tab")
        );
        let json = serde_json::to_string(&result).unwrap();
        assert!(!json.contains("private-error-secret"));
        assert!(!json.contains("private-fixture-cookie"));
    }

    #[tokio::test]
    async fn observation_redacts_its_copy_without_modifying_production_results() {
        let executor = FixtureExecutor::default();
        let observed = ObservedExecutor::new(&executor, |_| {});
        let artifact = observed
            .execute(ResearchActionRequest::open(
                "https://www.xiaohongshu.com/explore/note1".into(),
            ))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(artifact.metadata["cookie"], "private-fixture-cookie");
        let steps = observed.steps.lock().unwrap();
        assert_eq!(
            steps[0].data.as_ref().unwrap()["metadata"]["cookie"],
            "[REDACTED]"
        );
    }

    #[tokio::test]
    async fn preserves_production_subtitle_failure_and_does_not_count_comment_headers() {
        let executor = FixtureExecutor::default();
        let result = run_with_executor(
            &input(
                SourceTestPlatform::Bilibili,
                "https://www.bilibili.com/video/BV1abc",
            ),
            &executor,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result.status, "partial");
        assert!(result.checks.iter().all(|check| check.status == "missing"));
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("missing field `version`")));
        let subtitles = result
            .steps
            .iter()
            .find(|step| step.action == "extract_subtitles")
            .unwrap();
        assert_eq!(subtitles.data.as_ref().unwrap()["content"], "{}");
        assert_eq!(
            executor
                .calls
                .lock()
                .unwrap()
                .iter()
                .map(|call| call.action)
                .collect::<Vec<_>>(),
            vec![
                ResearchAction::OpenPage,
                ResearchAction::ScrollPage,
                ResearchAction::ExtractComments,
                ResearchAction::ExtractBilibiliSubtitles,
                ResearchAction::ClosePage,
            ]
        );
    }

    #[tokio::test]
    async fn xiaohongshu_runs_the_formal_adapter_and_keeps_navigation_token_out_of_results() {
        let executor = FixtureExecutor::default();
        let result = run_with_executor(
            &input(
                SourceTestPlatform::Xiaohongshu,
                "https://www.xiaohongshu.com/explore/note1?xsec_token=private-navigation-token",
            ),
            &executor,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result.images, vec!["https://cdn.example.com/note.jpg"]);
        assert_eq!(result.metadata["noteId"], "note1");
        assert!(executor.calls.lock().unwrap()[0]
            .url
            .as_ref()
            .unwrap()
            .contains("private-navigation-token"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("private-navigation-token"));
        assert_eq!(
            executor
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|call| call.action == ResearchAction::ClosePage)
                .count(),
            1
        );
    }

    #[test]
    fn rejects_other_platforms_search_pages_and_credential_urls() {
        for (platform, url) in [
            (
                SourceTestPlatform::Bilibili,
                "https://www.xiaohongshu.com/explore/note1",
            ),
            (
                SourceTestPlatform::Bilibili,
                "https://search.bilibili.com/all?keyword=1",
            ),
            (
                SourceTestPlatform::Xiaohongshu,
                "https://www.xiaohongshu.com/search_result?keyword=1",
            ),
            (
                SourceTestPlatform::Zhihu,
                "https://www.zhihu.com/search?q=1",
            ),
            (
                SourceTestPlatform::Zhihu,
                "https://username:password@www.zhihu.com/question/1/answer/2",
            ),
        ] {
            assert!(request_for(&input(platform, url)).is_err());
        }
        assert!(request_for(&input(
            SourceTestPlatform::Xiaohongshu,
            "https://notxiaohongshu.com/explore/note1"
        ))
        .is_err());
        assert!(request_for(&input(
            SourceTestPlatform::Zhihu,
            "https://www.zhihu.com/question/1/answer/2"
        ))
        .is_ok());
        assert!(request_for(&input(
            SourceTestPlatform::Zhihu,
            "https://zhuanlan.zhihu.com/p/123"
        ))
        .is_ok());
    }

    #[test]
    fn zero_views_are_available_and_visual_candidates_are_not_image_urls() {
        let request = request_for(&input(
            SourceTestPlatform::Bilibili,
            "https://www.bilibili.com/video/BV1abc",
        ))
        .unwrap();
        let step = SourceTestStep {
            index: 0,
            action: "extract_subtitles".into(),
            status: "returned".into(),
            elapsed_ms: 1,
            data: Some(serde_json::json!({"content": "{\"videoInfo\":{\"views\":0}}"})),
            error: None,
        };
        let result = summarize(
            &input(SourceTestPlatform::Bilibili, &request.url),
            "now".into(),
            1,
            Ok(ResearchReport::empty(&request)),
            vec![step],
        );
        assert_eq!(result.metadata["views"], "0");
        assert_eq!(
            result
                .checks
                .iter()
                .find(|check| check.key == "views")
                .unwrap()
                .status,
            "returned"
        );

        let request = request_for(&input(
            SourceTestPlatform::Zhihu,
            "https://www.zhihu.com/question/1/answer/2",
        ))
        .unwrap();
        let mut report = ResearchReport::empty(&request);
        report.body = Some(ResearchArtifact {
            kind: crate::research::models::ResearchArtifactKind::Body,
            title: "回答".into(),
            url: request.url.clone(),
            content: "包含图片的正文".into(),
            source_level: "community".into(),
            captured_at: "now".into(),
            metadata: BTreeMap::from([
                ("visualCandidate".into(), "true".into()),
                ("cookie".into(), "private-cookie".into()),
            ]),
        });
        let result = summarize(
            &input(SourceTestPlatform::Zhihu, &request.url),
            "now".into(),
            1,
            Ok(report),
            vec![],
        );
        assert!(result.images.is_empty());
        let images = result
            .checks
            .iter()
            .find(|check| check.key == "images")
            .unwrap();
        assert_eq!(images.status, "missing");
        assert!(images.detail.contains("未返回图片 URL"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("private-cookie"));
    }

    #[test]
    fn comment_count_requires_actual_numbered_entries_and_test_state_releases() {
        assert_eq!(comment_count("[评论内容]\n适配器：bilibili\n来源：URL"), 0);
        assert_eq!(comment_count("1. 用户甲 · 赞 2\n内容\n2. 用户乙\n内容"), 2);
        let state = SourceTestState::default();
        let guard = state.begin().unwrap();
        assert!(state.begin().is_err());
        drop(guard);
        assert!(state.begin().is_ok());
    }
}
