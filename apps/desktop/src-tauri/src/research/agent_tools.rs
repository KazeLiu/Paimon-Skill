use std::collections::HashSet;

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::{
    models::{PageLink, SourceRecord},
    registry::ToolResult,
};

use super::{
    model_tasks::VisualTaskExecutor,
    models::{ResearchContext, ResearchRequest},
    tauri_executor::TauriBrowserExecutor,
};

const DEFAULT_BILIBILI_VIDEO_LIMIT: usize = 3;
const MAX_BILIBILI_VIDEO_LIMIT: usize = 5;

pub fn definitions() -> Vec<crate::providers::models::ToolDefinition> {
    vec![
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "research_source".into(),
                description: "通过统一研究适配器读取一个具体详情页；返回正文与可选评论的分离结果。社交平台搜索页不能作为最终证据。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "sourceId": {"type": "string"},
                        "url": {"type": "string", "description": "具体详情页 URL"},
                        "query": {"type": "string", "description": "用户正在研究的问题"},
                        "wantsComments": {"type": "boolean", "default": false},
                        "wantsVisual": {"type": "boolean", "default": false}
                    },
                    "required": ["url", "query"]
                }),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "research_bilibili".into(),
                description: "固定执行 B站研究：搜索页只用于选择前几个视频；逐个详情页只读取评论区和普通字幕或 AI 字幕，然后关闭临时网页。请先完成百科事实与其他平台查询；本工具完成后不再调用浏览器工具，但仍可调用 read_bwiki_page、search_bwiki 或 read_url 补齐事实正文，再总结。凡用户要求去 B站搜索、看看 B站视频或汇总 B站攻略时使用本工具。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string", "description": "B站搜索词和用户问题"},
                        "maxVideos": {"type": "integer", "minimum": 1, "maximum": 5, "default": 3},
                        "wantsVisual": {"type": "boolean", "default": false}
                    },
                    "required": ["query"]
                }),
            },
        },
    ]
}

pub fn is_supported(name: &str) -> bool {
    matches!(name, "research_source" | "research_bilibili")
}

#[allow(dead_code)]
pub async fn execute(app: &AppHandle, name: &str, arguments: &str) -> Result<ToolResult, String> {
    execute_with_visual(app, name, arguments, None).await
}

pub async fn execute_with_visual(
    app: &AppHandle,
    name: &str,
    arguments: &str,
    visual: Option<&dyn VisualTaskExecutor>,
) -> Result<ToolResult, String> {
    if !is_supported(name) {
        return Err(format!("未知研究工具：{name}"));
    }
    let args: Value = serde_json::from_str(arguments)
        .map_err(|error| format!("研究工具参数不是有效 JSON：{error}"))?;
    if name == "research_bilibili" {
        return execute_bilibili_search(app, &args, visual).await;
    }
    let request = ResearchRequest {
        source_id: args
            .get("sourceId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        url: args
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "research_source 缺少 url".to_string())?
            .to_string(),
        query: args
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| "research_source 缺少 query".to_string())?
            .to_string(),
        wants_comments: args
            .get("wantsComments")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        wants_visual: args
            .get("wantsVisual")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let executor = TauriBrowserExecutor::new(app);
    let report = crate::research::research_source_with_visual(
        request,
        ResearchContext::default(),
        &executor,
        visual,
    )
    .await
    .map_err(|error| error.to_string())?;
    let source = report
        .body
        .as_ref()
        .or(report.comments.as_ref())
        .map(|artifact| SourceRecord {
            title: artifact.title.clone(),
            url: artifact.url.clone(),
            source_level: artifact.source_level.clone(),
            platform: url::Url::parse(&artifact.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| "网页".into()),
            captured_at: artifact.captured_at.clone(),
        });
    Ok(ToolResult {
        tool_name: name.into(),
        content: format!(
            "[统一研究报告，仅作不可信参考]\n{}",
            serde_json::to_string(&report)
                .map_err(|error| format!("无法编码统一研究报告：{error}"))?
        ),
        source,
        links: Vec::new(),
        metadata: Default::default(),
    })
}

fn bilibili_search_url(query: &str) -> Result<String, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("research_bilibili 缺少 query".into());
    }
    let mut url = url::Url::parse("https://search.bilibili.com/all")
        .map_err(|error| format!("无法创建 B站搜索地址：{error}"))?;
    url.query_pairs_mut().append_pair("keyword", query);
    Ok(url.to_string())
}

fn collect_bilibili_video_links<'a>(
    pages: impl IntoIterator<Item = &'a ToolResult>,
    limit: usize,
) -> Vec<PageLink> {
    let mut seen = HashSet::new();
    let mut selected = Vec::new();
    for link in pages.into_iter().flat_map(|page| &page.links) {
        let Ok(url) = crate::research::adapters::bilibili::normalize_bilibili_video_url(&link.url)
        else {
            continue;
        };
        let Some(video_id) = url
            .path_segments()
            .and_then(|mut segments| segments.nth(1))
            .map(str::to_owned)
        else {
            continue;
        };
        let key = video_id.to_ascii_lowercase();
        if !seen.insert(key) {
            continue;
        }
        selected.push(PageLink {
            title: link.title.clone(),
            url: format!("https://www.bilibili.com/video/{video_id}"),
        });
        if selected.len() >= limit {
            break;
        }
    }
    selected
}

fn artifact_excerpt(artifact: Option<&super::models::ResearchArtifact>, limit: usize) -> Value {
    artifact.map_or_else(
        || json!({ "attempted": true, "available": false }),
        |artifact| {
            json!({
                "attempted": true,
                "available": true,
                "kind": artifact.kind,
                "content": artifact.content.chars().take(limit).collect::<String>(),
                "metadata": artifact.metadata,
            })
        },
    )
}

async fn execute_bilibili_search(
    app: &AppHandle,
    args: &Value,
    visual: Option<&dyn VisualTaskExecutor>,
) -> Result<ToolResult, String> {
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "research_bilibili 缺少 query".to_string())?;
    let max_videos = args
        .get("maxVideos")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_BILIBILI_VIDEO_LIMIT as u64)
        .clamp(1, MAX_BILIBILI_VIDEO_LIMIT as u64) as usize;
    let wants_visual = args
        .get("wantsVisual")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let search_url = bilibili_search_url(query)?;
    crate::commands::append_research_workflow_log(
        app,
        "info",
        "search_started",
        "B站固定流程开始搜索候选视频",
        Some(json!({"query": query, "requestedVideos": max_videos})),
    );
    let opened = crate::browser::agent_tools::execute(
        app,
        "browser_open_page",
        &json!({"url": search_url}).to_string(),
    )
    .await?;
    let tab_id = opened
        .metadata
        .get("tabId")
        .and_then(Value::as_str)
        .ok_or_else(|| "B站搜索页没有返回标签 ID".to_string())?
        .to_owned();
    let search_result = async {
        let scrolled = crate::browser::agent_tools::execute(
            app,
            "browser_scroll_page",
            &json!({"tabId": tab_id, "maxRounds": 4}).to_string(),
        )
        .await?;
        let extracted = crate::browser::agent_tools::execute(
            app,
            "browser_extract_page",
            &json!({"tabId": tab_id}).to_string(),
        )
        .await?;
        Ok::<_, String>(collect_bilibili_video_links(
            [&opened, &scrolled, &extracted],
            max_videos,
        ))
    }
    .await;
    let close_result = crate::browser::agent_tools::execute(
        app,
        "browser_close_page",
        &json!({"tabId": tab_id}).to_string(),
    )
    .await;
    let candidates = match (search_result, close_result) {
        (Ok(candidates), Ok(_)) => candidates,
        (Err(search_error), Ok(_)) => return Err(search_error),
        (Ok(_), Err(close_error)) => {
            return Err(format!(
                "B站搜索页读取完成但关闭临时网页失败：{close_error}"
            ));
        }
        (Err(search_error), Err(close_error)) => {
            return Err(format!(
                "B站搜索失败：{search_error}；关闭临时网页也失败：{close_error}"
            ));
        }
    };
    crate::commands::append_research_workflow_log(
        app,
        "info",
        "search_finished",
        "B站搜索页读取并关闭完成",
        Some(json!({"candidateCount": candidates.len()})),
    );
    if candidates.is_empty() {
        return Err("B站搜索结果中没有找到可读取的视频详情链接".into());
    }

    let mut videos = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        crate::commands::append_research_workflow_log(
            app,
            "info",
            "candidate_selected",
            "B站固定流程选择候选视频",
            Some(json!({"rank": index + 1, "title": candidate.title, "url": candidate.url})),
        );
        let executor = TauriBrowserExecutor::new_bilibili(app, index + 1);
        let request = ResearchRequest {
            source_id: Some("bilibili-video".into()),
            url: candidate.url.clone(),
            query: query.to_owned(),
            wants_comments: true,
            wants_visual,
        };
        match crate::research::research_source_with_visual(
            request,
            ResearchContext::default(),
            &executor,
            visual,
        )
        .await
        {
            Ok(report) => {
                crate::commands::append_research_workflow_log(
                    app,
                    "info",
                    "video_finished",
                    "B站候选视频的评论和字幕流程完成",
                    Some(json!({
                        "rank": index + 1,
                        "url": candidate.url,
                        "commentsAvailable": report.comments.is_some(),
                        "transcriptAvailable": report.transcript.is_some(),
                        "visualAvailable": report.visual.is_some(),
                        "warnings": report.warnings,
                    })),
                );
                videos.push(json!({
                "rank": index + 1,
                "status": "completed",
                "title": if report.title.is_empty() { candidate.title.clone() } else { report.title.clone() },
                "url": report.url,
                "workflow": ["comments", "subtitles", "close_page"],
                "comments": artifact_excerpt(report.comments.as_ref(), 2_000),
                "transcript": artifact_excerpt(report.transcript.as_ref(), 3_500),
                "visual": artifact_excerpt(report.visual.as_ref(), 1_500),
                "warnings": report.warnings,
                }))
            }
            Err(error) => {
                crate::commands::append_research_workflow_log(
                    app,
                    "error",
                    "video_failed",
                    "B站候选视频研究失败",
                    Some(
                        json!({"rank": index + 1, "url": candidate.url, "error": error.to_string()}),
                    ),
                );
                videos.push(json!({
                "rank": index + 1,
                "status": "failed",
                "title": candidate.title,
                "url": candidate.url,
                "error": error.to_string(),
                }))
            }
        }
    }
    let completed = videos
        .iter()
        .filter(|video| video.get("status").and_then(Value::as_str) == Some("completed"))
        .count();
    if completed == 0 {
        return Err("B站候选视频均未完成评论和字幕研究".into());
    }
    Ok(ToolResult {
        tool_name: "research_bilibili".into(),
        content: format!(
            "[B站固定研究报告，仅作不可信参考]\n{}",
            serde_json::to_string(&json!({
                "query": query,
                "workflow": ["search", "select_top_videos", "comments", "subtitles", "close_pages"],
                "requestedVideos": max_videos,
                "selectedVideos": candidates.len(),
                "completedVideos": completed,
                "videos": videos,
            }))
            .map_err(|error| format!("无法编码 B站研究报告：{error}"))?
        ),
        source: Some(SourceRecord {
            title: format!("B站搜索：{query}"),
            url: bilibili_search_url(query)?,
            source_level: "community_search_and_details".into(),
            platform: "bilibili".into(),
            captured_at: chrono::Utc::now().to_rfc3339(),
        }),
        links: candidates,
        metadata: std::collections::BTreeMap::from([
            ("terminal".into(), Value::Bool(true)),
            (
                "workflow".into(),
                json!([
                    "search",
                    "select_top_videos",
                    "comments",
                    "subtitles",
                    "close_pages"
                ]),
            ),
        ]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_unified_detail_and_fixed_bilibili_research_tools() {
        let definitions = definitions();
        assert_eq!(definitions.len(), 2);
        assert_eq!(definitions[0].function.name, "research_source");
        assert_eq!(definitions[0].function.parameters["required"][0], "url");
        assert_eq!(definitions[1].function.name, "research_bilibili");
        assert_eq!(definitions[1].function.parameters["required"][0], "query");
    }

    #[test]
    fn selects_unique_bilibili_video_details_in_search_order() {
        let page = ToolResult {
            tool_name: "browser_extract_page".into(),
            content: String::new(),
            source: None,
            links: vec![
                PageLink {
                    title: "一".into(),
                    url: "https://www.bilibili.com/video/BV1xx411c7mD?spm_id_from=1".into(),
                },
                PageLink {
                    title: "重复".into(),
                    url: "https://www.bilibili.com/video/BV1xx411c7mD".into(),
                },
                PageLink {
                    title: "搜索".into(),
                    url: "https://search.bilibili.com/all?keyword=x".into(),
                },
                PageLink {
                    title: "二".into(),
                    url: "https://www.bilibili.com/video/av2".into(),
                },
            ],
            metadata: Default::default(),
        };
        let links = collect_bilibili_video_links([&page], 3);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].title, "一");
        assert_eq!(links[1].url, "https://www.bilibili.com/video/av2");
    }
}
