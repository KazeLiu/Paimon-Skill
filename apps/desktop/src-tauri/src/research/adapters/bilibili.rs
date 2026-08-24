use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

use crate::research::{
    executor::ResearchToolExecutor,
    model_tasks::{query_needs_visual, VisualImageRef, VisualTaskExecutor, VisualTaskRequest},
    models::{
        ResearchAction, ResearchActionRequest, ResearchArtifact, ResearchArtifactKind,
        ResearchContext, ResearchError, ResearchReport, ResearchRequest,
    },
};

use super::ResearchAdapter;

const MAX_SUBTITLE_SEGMENTS: usize = 4_000;
const MAX_DESCRIPTION_CHARS: usize = 8_000;
const MAX_TRANSCRIPT_CHARS: usize = 60_000;
const NO_SUBTITLE_WARNING: &str = "当前视频未取得可读取字幕，已跳过音频转写。";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BilibiliVideoInfo {
    pub video_id: String,
    pub aid: Option<u64>,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub published_at: Option<String>,
    pub page_count: Option<u32>,
    pub duration_seconds: Option<u64>,
    pub views: Option<u64>,
    pub replies: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleTrack {
    pub language: Option<String>,
    pub segments: Vec<SubtitleSegment>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSubtitlePayload {
    version: u8,
    status: String,
    video_info: Option<BrowserVideoInfo>,
    #[serde(default)]
    tracks: Vec<BrowserSubtitleTrack>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVideoInfo {
    aid: Option<u64>,
    bvid: String,
    title: String,
    author: String,
    description: String,
    published_at: Option<i64>,
    duration_seconds: Option<u64>,
    views: Option<u64>,
    replies: Option<u64>,
    current_part: Option<u32>,
    #[serde(default)]
    pages: Vec<BrowserVideoPart>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVideoPart {
    cid: u64,
    page: u32,
    part: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserSubtitleTrack {
    subtitle_type: String,
    language: String,
    language_label: String,
    part_index: u32,
    part_id: String,
    part_title: String,
    segments: Vec<BrowserSubtitleSegment>,
}

#[derive(Debug, Clone, Deserialize)]
struct BrowserSubtitleSegment {
    from: f64,
    to: f64,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserKeyframePayload {
    status: String,
    #[serde(default)]
    frames: Vec<BrowserKeyframe>,
    #[serde(default)]
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserKeyframe {
    timestamp_ms: u64,
    data_url: String,
    width: u32,
    height: u32,
}

pub fn normalize_bilibili_video_url(raw: &str) -> Result<Url, ResearchError> {
    let url = Url::parse(raw)
        .map_err(|error| ResearchError::new("invalid_url", format!("B站网址格式无效：{error}")))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !(host == "bilibili.com" || host.ends_with(".bilibili.com")) {
        return Err(ResearchError::new("not_bilibili", "网址不是 B站页面"));
    }
    let path = url.path().trim_end_matches('/');
    let segment = path
        .strip_prefix("/video/")
        .and_then(|value| value.split('/').next())
        .unwrap_or_default();
    let is_bv = segment.len() >= 3
        && segment[..2].eq_ignore_ascii_case("bv")
        && segment[2..].chars().all(|c| c.is_ascii_alphanumeric());
    let is_av = segment.len() >= 3
        && segment[..2].eq_ignore_ascii_case("av")
        && segment[2..].chars().all(|c| c.is_ascii_digit());
    if !is_bv && !is_av {
        return Err(ResearchError::new(
            "not_video_detail",
            "B站搜索页或首页不能作为视频详情证据",
        ));
    }
    Ok(url)
}

pub fn parse_video_info(
    text: &str,
    fallback_url: &str,
) -> Result<BilibiliVideoInfo, ResearchError> {
    let url = normalize_bilibili_video_url(fallback_url)?;
    let video_id = url
        .path_segments()
        .and_then(|mut segments| segments.nth(1))
        .unwrap_or_default()
        .to_owned();
    let field = |name: &str| -> Option<String> {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(&format!("{name}:")))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
    };
    let number = |name: &str| field(name).and_then(|v| v.replace(',', "").parse().ok());
    Ok(BilibiliVideoInfo {
        video_id,
        aid: number("aid"),
        title: field("title").unwrap_or_default(),
        author: field("author"),
        description: field("description").map(|v| v.chars().take(MAX_DESCRIPTION_CHARS).collect()),
        published_at: field("publish_time"),
        page_count: number("parts").and_then(|value| u32::try_from(value).ok()),
        duration_seconds: field("duration").and_then(parse_duration_seconds),
        views: number("view"),
        replies: number("reply"),
    })
}

fn parse_duration_seconds(value: String) -> Option<u64> {
    if let Some(seconds) = value
        .strip_prefix('(')
        .and_then(|v| v.strip_suffix('s').and_then(|v| v.strip_suffix(')')))
        .and_then(|v| v.parse().ok())
    {
        return Some(seconds);
    }
    if let Some(start) = value.find('(') {
        if let Some(end) = value[start + 1..].find(')') {
            if let Some(seconds) = value[start + 1..start + 1 + end]
                .strip_suffix('s')
                .and_then(|v| v.parse().ok())
            {
                return Some(seconds);
            }
        }
    }
    let values = value
        .split(':')
        .map(|v| v.trim().parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [minutes, seconds] => Some(minutes * 60 + seconds),
        [hours, minutes, seconds] => Some(hours * 3600 + minutes * 60 + seconds),
        _ => None,
    }
}

pub fn parse_subtitle_json(value: &str) -> Result<SubtitleTrack, ResearchError> {
    #[derive(Deserialize)]
    struct RawBody {
        body: Vec<RawSegment>,
    }
    #[derive(Deserialize)]
    struct RawSegment {
        from: f64,
        to: f64,
        content: String,
    }
    let parsed: RawBody = serde_json::from_str(value)
        .map_err(|e| ResearchError::new("subtitle_parse", format!("字幕格式无效：{e}")))?;
    let mut segments = parsed
        .body
        .into_iter()
        .take(MAX_SUBTITLE_SEGMENTS)
        .filter_map(|raw| {
            if raw.content.trim().is_empty()
                || !raw.from.is_finite()
                || !raw.to.is_finite()
                || raw.to < raw.from
            {
                return None;
            }
            Some(SubtitleSegment {
                start_ms: (raw.from.max(0.0) * 1000.0) as u64,
                end_ms: (raw.to.max(0.0) * 1000.0) as u64,
                text: raw.content.trim().chars().take(2000).collect(),
            })
        })
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(ResearchError::new("subtitle_empty", "字幕没有有效分段"));
    }
    segments.sort_by_key(|segment| segment.start_ms);
    Ok(SubtitleTrack {
        language: None,
        segments,
    })
}

fn add_info_metadata(artifact: &mut ResearchArtifact, info: &BilibiliVideoInfo) {
    artifact
        .metadata
        .insert("videoId".into(), info.video_id.clone());
    if let Some(author) = &info.author {
        artifact.metadata.insert("author".into(), author.clone());
    }
    if let Some(date) = &info.published_at {
        artifact.metadata.insert("publishedAt".into(), date.clone());
    }
    if let Some(parts) = info.page_count {
        artifact
            .metadata
            .insert("pageCount".into(), parts.to_string());
    }
}

fn query_requests_all_parts(query: &str) -> bool {
    [
        "全集",
        "全部分P",
        "所有分P",
        "每个分P",
        "各分P",
        "整套",
        "全套",
    ]
    .iter()
    .any(|term| query.contains(term))
}

fn parse_browser_payload(value: &str) -> Result<BrowserSubtitlePayload, ResearchError> {
    let payload: BrowserSubtitlePayload = serde_json::from_str(value).map_err(|error| {
        ResearchError::new("subtitle_payload", format!("字幕页面数据无效：{error}"))
    })?;
    if payload.version != 1 {
        return Err(ResearchError::new(
            "subtitle_payload",
            "字幕页面数据版本不受支持",
        ));
    }
    Ok(payload)
}

fn preferred_tracks(tracks: Vec<BrowserSubtitleTrack>) -> Vec<BrowserSubtitleTrack> {
    let mut by_part = BTreeMap::<u32, Vec<BrowserSubtitleTrack>>::new();
    for track in tracks {
        if matches!(track.subtitle_type.as_str(), "manual" | "ai") {
            by_part.entry(track.part_index).or_default().push(track);
        }
    }
    by_part
        .into_values()
        .filter_map(|mut tracks| {
            tracks.sort_by_key(|track| {
                let type_rank = usize::from(track.subtitle_type != "manual");
                let language_rank = usize::from(
                    !track.language.to_ascii_lowercase().contains("zh")
                        && !track.language_label.contains("中文"),
                );
                (type_rank, language_rank)
            });
            tracks.into_iter().next()
        })
        .collect()
}

fn format_timestamp(milliseconds: u64) -> String {
    let total_seconds = milliseconds / 1_000;
    let millis = milliseconds % 1_000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        total_seconds / 3_600,
        total_seconds % 3_600 / 60,
        total_seconds % 60,
        millis
    )
}

fn transcript_artifact(
    raw: ResearchArtifact,
    payload: &BrowserSubtitlePayload,
) -> Result<Option<ResearchArtifact>, ResearchError> {
    if payload.status != "ok" {
        return Ok(None);
    }
    let mut content = String::new();
    let mut types = Vec::new();
    let mut languages = Vec::new();
    let mut parts = Vec::new();
    for track in preferred_tracks(payload.tracks.clone()) {
        let subtitle_type = if track.subtitle_type == "ai" {
            "AI字幕"
        } else {
            "普通字幕"
        };
        types.push(track.subtitle_type.clone());
        languages.push(track.language.clone());
        parts.push(track.part_index.to_string());
        content.push_str(&format!(
            "\n## P{} {} · {} · {}\n",
            track.part_index, track.part_title, subtitle_type, track.language_label
        ));
        let mut segments = track
            .segments
            .into_iter()
            .take(MAX_SUBTITLE_SEGMENTS)
            .filter_map(|segment| {
                if !segment.from.is_finite()
                    || !segment.to.is_finite()
                    || segment.from < 0.0
                    || segment.to < segment.from
                    || segment.content.trim().is_empty()
                {
                    return None;
                }
                Some(SubtitleSegment {
                    start_ms: (segment.from * 1_000.0) as u64,
                    end_ms: (segment.to * 1_000.0) as u64,
                    text: segment.content.trim().chars().take(2_000).collect(),
                })
            })
            .collect::<Vec<_>>();
        segments.sort_by_key(|segment| segment.start_ms);
        for segment in segments {
            let line = format!(
                "[{} - {}] {}\n",
                format_timestamp(segment.start_ms),
                format_timestamp(segment.end_ms),
                segment.text
            );
            if content.chars().count() + line.chars().count() > MAX_TRANSCRIPT_CHARS {
                content.push_str("[字幕已按总字符上限截断]\n");
                break;
            }
            content.push_str(&line);
        }
    }
    if content.trim().is_empty() {
        return Ok(None);
    }
    let mut metadata = BTreeMap::from([
        ("subtitleTypes".into(), types.join(",")),
        ("languages".into(), languages.join(",")),
        ("parts".into(), parts.join(",")),
    ]);
    if let Some(video) = &payload.video_info {
        metadata.insert("videoId".into(), video.bvid.clone());
    }
    Ok(Some(ResearchArtifact {
        kind: ResearchArtifactKind::Transcript,
        title: raw.title,
        url: raw.url,
        content: content.trim().to_owned(),
        source_level: raw.source_level,
        captured_at: raw.captured_at,
        metadata,
    }))
}

fn apply_browser_video_info(body: &mut ResearchArtifact, info: &BrowserVideoInfo) {
    body.metadata.insert("videoId".into(), info.bvid.clone());
    if let Some(aid) = info.aid {
        body.metadata.insert("aid".into(), aid.to_string());
    }
    if !info.author.is_empty() {
        body.metadata.insert("author".into(), info.author.clone());
    }
    if let Some(value) = info.published_at {
        body.metadata
            .insert("publishedAt".into(), value.to_string());
    }
    if let Some(value) = info.current_part {
        body.metadata
            .insert("currentPart".into(), value.to_string());
    }
    body.metadata
        .insert("pageCount".into(), info.pages.len().to_string());
    body.metadata.insert(
        "parts".into(),
        info.pages
            .iter()
            .map(|part| format!("P{}:{}:{}", part.page, part.cid, part.part))
            .collect::<Vec<_>>()
            .join(" | "),
    );
    if !info.title.is_empty() {
        body.title = info.title.clone();
    }
    for (key, value) in [
        ("durationSeconds", info.duration_seconds),
        ("views", info.views),
        ("replies", info.replies),
    ] {
        if let Some(value) = value {
            body.metadata.insert(key.into(), value.to_string());
        }
    }
    if !info.description.is_empty() {
        body.metadata.insert(
            "description".into(),
            info.description
                .chars()
                .take(MAX_DESCRIPTION_CHARS)
                .collect(),
        );
    }
}

fn visual_intent(request: &ResearchRequest) -> bool {
    request.wants_visual || query_needs_visual(&request.query)
}

fn parse_transcript_timestamp(line: &str) -> Option<u64> {
    let start = line.strip_prefix('[')?.split_once(" - ")?.0;
    let mut parts = start.split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds = parts.next()?.replace(',', ".");
    let seconds = seconds.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some(((hours * 3600 + minutes * 60) as f64 * 1000.0 + seconds * 1000.0) as u64)
}

fn choose_keyframe_timestamps(report: &ResearchReport) -> Vec<u64> {
    let mut timestamps = report
        .transcript
        .as_ref()
        .map(|artifact| {
            artifact
                .content
                .lines()
                .filter_map(parse_transcript_timestamp)
                .step_by(8)
                .take(4)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if timestamps.is_empty() {
        if let Some(duration) = report
            .body
            .as_ref()
            .and_then(|body| body.metadata.get("durationSeconds"))
            .and_then(|value| value.parse::<u64>().ok())
        {
            timestamps = [0, 25, 50, 75]
                .into_iter()
                .map(|percent| duration.saturating_mul(percent) / 100)
                .collect();
        }
    }
    timestamps.sort_unstable();
    timestamps.dedup();
    timestamps.into_iter().take(4).collect()
}

async fn run_keyframe_visual(
    executor: &impl ResearchToolExecutor,
    visual: &dyn VisualTaskExecutor,
    tab_id: &str,
    request: &ResearchRequest,
    report: &mut ResearchReport,
) -> Result<(), ResearchError> {
    let timestamps = choose_keyframe_timestamps(report);
    if timestamps.is_empty() {
        report
            .warnings
            .push("未找到适合的时间点，跳过关键帧视觉分析".into());
        return Ok(());
    }
    let raw = match executor
        .execute(ResearchActionRequest::bilibili_keyframes(
            Some(tab_id.to_owned()),
            timestamps,
        ))
        .await
    {
        Ok(Some(raw)) => raw,
        Ok(None) => {
            report
                .warnings
                .push("未取得可读取关键帧，跳过视觉分析".into());
            return Ok(());
        }
        Err(error) => {
            report
                .warnings
                .push(format!("关键帧读取失败：{}", error.message));
            return Ok(());
        }
    };
    let payload: BrowserKeyframePayload = match serde_json::from_str(&raw.content) {
        Ok(payload) => payload,
        Err(_) => {
            report
                .warnings
                .push("关键帧数据无法解析，跳过视觉分析".into());
            return Ok(());
        }
    };
    for warning in payload.warnings.into_iter().take(8) {
        report.warnings.push(format!("关键帧提示：{warning}"));
    }
    if payload.status != "ok" || payload.frames.is_empty() {
        report
            .warnings
            .push("未取得可读取关键帧，跳过视觉分析".into());
        return Ok(());
    }
    let images = payload
        .frames
        .into_iter()
        .take(6)
        .map(|frame| VisualImageRef {
            url: frame.data_url,
            alt: Some(format!(
                "B站视频关键帧 {}ms {}x{}",
                frame.timestamp_ms, frame.width, frame.height
            )),
        })
        .collect::<Vec<_>>();
    let Some(task) = VisualTaskRequest::bounded(&request.query, images) else {
        report
            .warnings
            .push("关键帧视觉请求为空，跳过视觉分析".into());
        return Ok(());
    };
    match visual.execute(task).await {
        Ok(result) => {
            let mut metadata = BTreeMap::new();
            metadata.insert("analyzedImages".into(), result.analyzed_images.to_string());
            metadata.insert(
                "visualReason".into(),
                request.query.chars().take(200).collect(),
            );
            report.visual = Some(ResearchArtifact {
                kind: ResearchArtifactKind::Visual,
                title: report.title.clone(),
                url: report.url.clone(),
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
    Ok(())
}

pub struct BilibiliAdapter<'a, E> {
    executor: &'a E,
}
impl<'a, E> BilibiliAdapter<'a, E> {
    pub fn new(executor: &'a E) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl<E: ResearchToolExecutor> ResearchAdapter for BilibiliAdapter<'_, E> {
    async fn research(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
    ) -> Result<ResearchReport, ResearchError> {
        self.research_internal(request, context, None).await
    }
}

impl<E: ResearchToolExecutor> BilibiliAdapter<'_, E> {
    pub async fn research_with_visual(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
        visual: &dyn VisualTaskExecutor,
    ) -> Result<ResearchReport, ResearchError> {
        self.research_internal(request, context, Some(visual)).await
    }

    async fn research_internal(
        &self,
        request: ResearchRequest,
        context: ResearchContext,
        visual: Option<&dyn VisualTaskExecutor>,
    ) -> Result<ResearchReport, ResearchError> {
        let normalized = normalize_bilibili_video_url(&request.url)?;
        let mut request = request;
        request.url = normalized.to_string();
        let all_parts = query_requests_all_parts(&request.query);
        let mut report = ResearchReport::empty(&request);

        let opened = self
            .executor
            .execute(ResearchActionRequest::open(request.url.clone()))
            .await?;
        let tab_id = opened
            .as_ref()
            .and_then(|artifact| artifact.metadata.get("tabId"))
            .cloned()
            .ok_or_else(|| ResearchError::new("missing_tab_id", "浏览器没有返回研究标签 ID"))?;

        let result = self
            .collect_report(&request, &context, &tab_id, all_parts, visual, &mut report)
            .await;
        let close_result = self
            .executor
            .execute(ResearchActionRequest::tab(
                ResearchAction::ClosePage,
                Some(tab_id),
            ))
            .await;
        if let Err(error) = close_result {
            if result.is_ok() {
                return Err(ResearchError::new(
                    "close_failed",
                    format!("研究完成但关闭临时网页失败：{}", error.message),
                ));
            }
        }
        result?;
        Ok(report)
    }

    async fn collect_report(
        &self,
        request: &ResearchRequest,
        context: &ResearchContext,
        tab_id: &str,
        all_parts: bool,
        visual: Option<&dyn VisualTaskExecutor>,
        report: &mut ResearchReport,
    ) -> Result<(), ResearchError> {
        if context.cancellation.is_cancelled() {
            return Err(ResearchError::new("cancelled", "研究任务已取消"));
        }
        let mut scroll =
            ResearchActionRequest::tab(ResearchAction::ScrollPage, Some(tab_id.to_owned()));
        scroll.max_scroll_rounds = context.max_scroll_rounds;
        let _ = self.executor.execute(scroll).await?;
        let body = self
            .executor
            .execute(ResearchActionRequest::tab(
                ResearchAction::ExtractPage,
                Some(tab_id.to_owned()),
            ))
            .await?;
        if let Some(mut body) = body {
            body.kind = ResearchArtifactKind::Body;
            report.add_artifact(body);
        }

        if context.cancellation.is_cancelled() {
            return Err(ResearchError::new("cancelled", "研究任务已取消"));
        }
        match self
            .executor
            .execute(ResearchActionRequest::bilibili_subtitles(
                Some(tab_id.to_owned()),
                all_parts,
            ))
            .await
        {
            Ok(Some(raw)) => match parse_browser_payload(&raw.content) {
                Ok(payload) => {
                    if let (Some(body), Some(info)) = (report.body.as_mut(), &payload.video_info) {
                        apply_browser_video_info(body, info);
                    }
                    if let Some(transcript) = transcript_artifact(raw, &payload)? {
                        report.add_artifact(transcript);
                    } else {
                        report.warnings.push(NO_SUBTITLE_WARNING.into());
                    }
                    for warning in payload.warnings.into_iter().take(12) {
                        report.warnings.push(format!("B站字幕提示：{warning}"));
                    }
                }
                Err(error) => {
                    report
                        .warnings
                        .push(format!("B站字幕页面数据无法解析：{}", error.message));
                    report.warnings.push(NO_SUBTITLE_WARNING.into());
                }
            },
            Ok(None) => report.warnings.push(NO_SUBTITLE_WARNING.into()),
            Err(error) => {
                report
                    .warnings
                    .push(format!("B站字幕读取失败：{}", error.message));
                report.warnings.push(NO_SUBTITLE_WARNING.into());
            }
        }

        if request.wants_comments {
            match self
                .executor
                .execute(ResearchActionRequest::tab(
                    ResearchAction::ExtractComments,
                    Some(tab_id.to_owned()),
                ))
                .await
            {
                Ok(Some(mut comments)) => {
                    comments.kind = ResearchArtifactKind::Comments;
                    report.add_artifact(comments);
                }
                Ok(None) => report.warnings.push("未取得视频评论".into()),
                Err(error) => report
                    .warnings
                    .push(format!("视频评论读取失败：{}", error.message)),
            }
        }
        if let Some(visual) = visual.filter(|_| visual_intent(request)) {
            run_keyframe_visual(self.executor, visual, tab_id, request, report).await?;
        }
        if report.body.is_none() {
            report.warnings.push("未取得视频详情正文".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    struct FakeExecutor {
        calls: Mutex<Vec<ResearchActionRequest>>,
        results: Mutex<VecDeque<Result<Option<ResearchArtifact>, ResearchError>>>,
    }

    struct RecordingVisual {
        calls: Mutex<Vec<VisualTaskRequest>>,
    }

    #[async_trait]
    impl VisualTaskExecutor for RecordingVisual {
        async fn execute(
            &self,
            request: VisualTaskRequest,
        ) -> Result<
            crate::research::model_tasks::VisualTaskResult,
            crate::research::model_tasks::VisualTaskError,
        > {
            self.calls.lock().unwrap().push(request);
            Ok(crate::research::model_tasks::VisualTaskResult {
                content: "画面事实".into(),
                analyzed_images: 1,
                warnings: Vec::new(),
            })
        }
    }

    #[async_trait]
    impl ResearchToolExecutor for FakeExecutor {
        async fn execute(
            &self,
            request: ResearchActionRequest,
        ) -> Result<Option<ResearchArtifact>, ResearchError> {
            self.calls.lock().unwrap().push(request);
            self.results.lock().unwrap().pop_front().unwrap()
        }
    }

    fn artifact(kind: ResearchArtifactKind, content: &str, opened: bool) -> ResearchArtifact {
        let mut metadata = BTreeMap::new();
        if opened {
            metadata.insert("tabId".into(), "bilibili-tab".into());
        }
        ResearchArtifact {
            kind,
            title: "视频".into(),
            url: "https://www.bilibili.com/video/BV1GY4y1U7oq".into(),
            content: content.into(),
            source_level: "browser_webview".into(),
            captured_at: "now".into(),
            metadata,
        }
    }

    fn keyframe_payload() -> ResearchArtifact {
        artifact(
            ResearchArtifactKind::Visual,
            r#"{"version":1,"status":"ok","frames":[{"timestampMs":1000,"dataUrl":"data:image/jpeg;base64,a2V5ZnJhbWU=","width":960,"height":540}],"warnings":[]}"#,
            false,
        )
    }

    fn subtitle_payload() -> ResearchArtifact {
        artifact(
            ResearchArtifactKind::Transcript,
            r#"{"version":1,"status":"ok","videoInfo":{"aid":2,"bvid":"BV1GY4y1U7oq","title":"视频","author":"UP","description":"","publishedAt":null,"durationSeconds":10,"views":1,"replies":0,"currentPart":1,"pages":[]},"tracks":[{"subtitleType":"manual","language":"zh-CN","languageLabel":"中文","partIndex":1,"partId":"1","partTitle":"","segments":[{"from":1.0,"to":2.0,"content":"画面"}]}],"warnings":[]}"#,
            false,
        )
    }
    #[test]
    fn accepts_bv_and_av_details_but_rejects_discovery_pages() {
        assert!(
            normalize_bilibili_video_url("https://www.bilibili.com/video/BV1xx411c7mD").is_ok()
        );
        assert!(normalize_bilibili_video_url("https://www.bilibili.com/video/av2").is_ok());
        assert_eq!(
            normalize_bilibili_video_url("https://search.bilibili.com/all?keyword=x")
                .unwrap_err()
                .code,
            "not_video_detail"
        );
    }
    #[test]
    fn parses_structured_video_metadata_fixture() {
        let info = parse_video_info("title: 测试视频\naid: 2\nauthor: UP主\npublish_time: 2026-08-20\nduration: 01:02:03\nview: 1,234\nreply: 56\nparts: 2\ndescription: 简介", "https://www.bilibili.com/video/BV1xx411c7mD").unwrap();
        assert_eq!(info.video_id, "BV1xx411c7mD");
        assert_eq!(info.duration_seconds, Some(3723));
        assert_eq!(info.views, Some(1234));
        assert_eq!(info.page_count, Some(2));
    }
    #[test]
    fn parses_and_bounds_timestamped_subtitles() {
        let track = parse_subtitle_json(r#"{"body":[{"from":2.5,"to":4.0,"content":"第二句"},{"from":0,"to":2,"content":"第一句"}]}"#).unwrap();
        assert_eq!(track.segments[0].start_ms, 0);
        assert_eq!(track.segments[1].end_ms, 4000);
    }
    #[test]
    fn rejects_empty_or_invalid_subtitles() {
        assert_eq!(
            parse_subtitle_json(r#"{"body":[]}"#).unwrap_err().code,
            "subtitle_empty"
        );
        assert_eq!(
            parse_subtitle_json("not-json").unwrap_err().code,
            "subtitle_parse"
        );
    }

    #[test]
    fn parses_manual_and_ai_tracks_into_one_transcript_shape() {
        let payload = parse_browser_payload(
            r#"{
              "version":1,"status":"ok","videoInfo":{"aid":2,"bvid":"BV1GY4y1U7oq","title":"标题","author":"UP","description":"简介","publishedAt":1,"durationSeconds":10,"views":2,"replies":3,"currentPart":1,"pages":[{"cid":11,"page":1,"part":"第一P"},{"cid":12,"page":2,"part":"第二P"}]},
              "tracks":[
                {"subtitleType":"ai","language":"ai-zh","languageLabel":"中文","partIndex":1,"partId":"11","partTitle":"第一P","segments":[{"from":1.5,"to":2.5,"content":"AI文本"}]},
                {"subtitleType":"manual","language":"zh-CN","languageLabel":"中文","partIndex":1,"partId":"11","partTitle":"第一P","segments":[{"from":0,"to":1.5,"content":"人工文本"}]},
                {"subtitleType":"ai","language":"ai-zh","languageLabel":"中文","partIndex":2,"partId":"12","partTitle":"第二P","segments":[{"from":0,"to":1,"content":"第二P文本"}]}
              ],"warnings":[]
            }"#,
        )
        .unwrap();
        let transcript = transcript_artifact(
            artifact(ResearchArtifactKind::Transcript, "", false),
            &payload,
        )
        .unwrap()
        .unwrap();
        assert!(transcript.content.contains("普通字幕"));
        assert!(transcript.content.contains("人工文本"));
        assert!(!transcript.content.contains("AI文本"));
        assert!(transcript.content.contains("第二P文本"));
        assert_eq!(transcript.metadata.get("parts").unwrap(), "1,2");
    }

    #[test]
    fn all_parts_intent_is_explicit() {
        assert!(!query_requests_all_parts("总结这个视频"));
        assert!(query_requests_all_parts("总结所有分P的内容"));
    }

    #[tokio::test]
    async fn no_subtitle_uses_fixed_degradation_and_closes_page() {
        let payload = r#"{"version":1,"status":"no_subtitles","videoInfo":{"aid":2,"bvid":"BV1GY4y1U7oq","title":"标题","author":"UP","description":"简介","publishedAt":1,"durationSeconds":10,"views":2,"replies":3,"currentPart":1,"pages":[{"cid":11,"page":1,"part":"第一P"}]},"tracks":[],"warnings":[]}"#;
        let fake = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "滚动", false))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "正文", false))),
                Ok(Some(artifact(
                    ResearchArtifactKind::Transcript,
                    payload,
                    false,
                ))),
                Ok(None),
            ])),
        };
        let report = BilibiliAdapter::new(&fake)
            .research(
                ResearchRequest {
                    source_id: Some("bilibili-video".into()),
                    url: "https://www.bilibili.com/video/BV1GY4y1U7oq".into(),
                    query: "总结".into(),
                    wants_comments: false,
                    wants_visual: false,
                },
                ResearchContext::default(),
            )
            .await
            .unwrap();
        assert!(report.transcript.is_none());
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning == NO_SUBTITLE_WARNING));
        let calls = fake.calls.lock().unwrap();
        assert_eq!(calls[3].action, ResearchAction::ExtractBilibiliSubtitles);
        assert_eq!(calls.last().unwrap().action, ResearchAction::ClosePage);
    }

    #[tokio::test]
    async fn subtitle_failure_keeps_body_and_still_closes_page() {
        let fake = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "滚动", false))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "正文", false))),
                Err(ResearchError::new("subtitle_eval", "页面变化")),
                Ok(None),
            ])),
        };
        let report = BilibiliAdapter::new(&fake)
            .research(
                ResearchRequest {
                    source_id: None,
                    url: "https://www.bilibili.com/video/BV1GY4y1U7oq".into(),
                    query: "总结".into(),
                    wants_comments: false,
                    wants_visual: false,
                },
                ResearchContext::default(),
            )
            .await
            .unwrap();
        assert!(report.body.is_some());
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning == NO_SUBTITLE_WARNING));
        assert_eq!(
            fake.calls.lock().unwrap().last().unwrap().action,
            ResearchAction::ClosePage
        );
    }

    #[tokio::test]
    async fn plain_summary_does_not_extract_keyframes_or_call_visual() {
        let fake = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "滚动", false))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "正文", false))),
                Ok(Some(subtitle_payload())),
                Ok(None),
            ])),
        };
        let visual = RecordingVisual {
            calls: Mutex::new(Vec::new()),
        };
        let report = BilibiliAdapter::new(&fake)
            .research_with_visual(
                ResearchRequest {
                    source_id: None,
                    url: "https://www.bilibili.com/video/BV1GY4y1U7oq".into(),
                    query: "总结视频".into(),
                    wants_comments: false,
                    wants_visual: false,
                },
                ResearchContext::default(),
                &visual,
            )
            .await
            .unwrap();
        assert!(report.visual.is_none());
        assert!(visual.calls.lock().unwrap().is_empty());
        assert!(!fake
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.action == ResearchAction::ExtractBilibiliKeyframes));
    }

    #[tokio::test]
    async fn visual_intent_extracts_keyframes_into_separate_artifact_and_closes() {
        let fake = FakeExecutor {
            calls: Mutex::new(Vec::new()),
            results: Mutex::new(VecDeque::from([
                Ok(Some(artifact(ResearchArtifactKind::Body, "打开", true))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "滚动", false))),
                Ok(Some(artifact(ResearchArtifactKind::Body, "正文", false))),
                Ok(Some(subtitle_payload())),
                Ok(Some(keyframe_payload())),
                Ok(None),
            ])),
        };
        let visual = RecordingVisual {
            calls: Mutex::new(Vec::new()),
        };
        let report = BilibiliAdapter::new(&fake)
            .research_with_visual(
                ResearchRequest {
                    source_id: None,
                    url: "https://www.bilibili.com/video/BV1GY4y1U7oq".into(),
                    query: "请看画面里的面板".into(),
                    wants_comments: false,
                    wants_visual: false,
                },
                ResearchContext::default(),
                &visual,
            )
            .await
            .unwrap();
        assert!(report.body.is_some());
        assert!(report.transcript.is_some());
        assert_eq!(
            report.visual.as_ref().unwrap().kind,
            ResearchArtifactKind::Visual
        );
        assert_eq!(visual.calls.lock().unwrap().len(), 1);
        assert_eq!(
            fake.calls.lock().unwrap().last().unwrap().action,
            ResearchAction::ClosePage
        );
    }
}
