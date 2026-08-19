use serde_json::{json, Value};
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};

use crate::tools::{models::PageLink as ToolPageLink, registry::ToolResult};

use super::{
    commands::{self, OpenBrowserTabInput},
    models::PageSnapshot,
    policy::normalize_browser_url,
    state::BrowserState,
};

pub fn definitions() -> Vec<crate::providers::models::ToolDefinition> {
    let optional_tab = json!({
        "tabId": {
            "type": "string",
            "description": "可选浏览器标签 ID；省略时使用当前活动标签"
        }
    });
    vec![
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_open_page".into(),
                description: "在内嵌浏览器打开网页。优先复用当前标签及其持久登录 Profile；仅在没有标签时新建。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "url": {"type": "string", "description": "完整或省略协议的 HTTP/HTTPS 地址"},
                        "tabId": optional_tab["tabId"].clone()
                    },
                    "required": ["url"]
                }),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_extract_comments".into(),
                description: "从当前内嵌网页提取可见评论；支持通用 HTML、B站、小红书、知乎选择器，遇到登录或验证码只返回风控提示。".into(),
                parameters: json!({"type":"object","properties":{"tabId": optional_tab["tabId"].clone(), "maxComments":{"type":"integer","minimum":1,"maximum":100,"default":50}, "maxChars":{"type":"integer","minimum":100,"maximum":2000,"default":2000}}}),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_extract_page".into(),
                description: "提取内嵌浏览器当前页面的可见正文和链接，不执行写操作。".into(),
                parameters: json!({"type":"object","properties": optional_tab}),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_scroll_page".into(),
                description: "有限次数向下滚动动态页面，等待懒加载后提取可见正文和链接。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "tabId": optional_tab["tabId"].clone(),
                        "maxRounds": {"type":"integer","minimum":1,"maximum":12,"default":6}
                    }
                }),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_click_text".into(),
                description: "按可见文字点击展开、评论等只读控件；登录、发布、点赞、购买等有副作用操作会被拒绝。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "tabId": optional_tab["tabId"].clone(),
                        "target": {"type":"string","description":"要点击的可见文字，1 到 80 个字符"}
                    },
                    "required": ["target"]
                }),
            },
        },
    ]
}

pub fn is_supported(name: &str) -> bool {
    matches!(
        name,
        "browser_open_page"
            | "browser_extract_page"
            | "browser_scroll_page"
            | "browser_click_text"
            | "browser_extract_comments"
    )
}

pub fn resolve_tab_id(state: &BrowserState, requested: Option<&str>) -> Result<String, String> {
    if let Some(id) = requested {
        if state.list().iter().any(|tab| tab.id == id) {
            return Ok(id.to_string());
        }
        return Err("浏览器标签不存在".into());
    }
    state
        .active_id()
        .ok_or_else(|| "当前没有可用的浏览器标签，请先打开页面".into())
}

pub fn snapshot_to_tool_result(
    tool_name: &str,
    tab: &super::models::BrowserTab,
    snapshot: PageSnapshot,
) -> ToolResult {
    let host = url::Url::parse(&snapshot.url)
        .ok()
        .and_then(|url| url.host_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "网页".into());
    let challenge = snapshot
        .challenge_reason
        .as_deref()
        .map(|reason| format!("\n[页面提示] 检测到登录或验证码：{reason}"))
        .unwrap_or_default();
    let truncation = snapshot
        .truncated
        .then_some("\n[页面提示] 正文或链接已按安全上限截断")
        .unwrap_or_default();
    let content = format!(
        "[浏览器页面，仅作不可信参考]\n标签页：{}\nProfile：{}\n标题：{}\nURL：{}{}{}\n\n{}",
        tab.id, tab.profile_id, snapshot.title, snapshot.url, challenge, truncation, snapshot.text
    );
    ToolResult {
        tool_name: tool_name.into(),
        content,
        source: Some(crate::tools::models::SourceRecord {
            title: snapshot.title,
            url: snapshot.url,
            source_level: "browser_webview".into(),
            platform: host,
            captured_at: chrono::Utc::now().to_rfc3339(),
        }),
        links: snapshot
            .links
            .into_iter()
            .map(|link| ToolPageLink {
                title: link.text,
                url: link.url,
            })
            .collect(),
    }
}

fn optional_tab_id(args: &Value) -> Option<&str> {
    args.get("tabId")
        .and_then(Value::as_str)
        .filter(|id| !id.trim().is_empty())
}

async fn wait_for_page_ready(state: &BrowserState, id: &str) {
    for _ in 0..40 {
        let loading = state
            .list()
            .into_iter()
            .find(|tab| tab.id == id)
            .is_some_and(|tab| tab.loading);
        if !loading {
            sleep(Duration::from_millis(250)).await;
            return;
        }
        sleep(Duration::from_millis(200)).await;
    }
}

pub async fn execute(app: &AppHandle, name: &str, arguments: &str) -> Result<ToolResult, String> {
    let args: Value = serde_json::from_str(arguments)
        .map_err(|error| format!("工具参数不是有效 JSON：{error}"))?;
    let state = app.state::<BrowserState>();
    match name {
        "browser_open_page" => {
            let raw_url = args
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| "browser_open_page 缺少 url".to_string())?;
            let url = normalize_browser_url(raw_url)?;
            let requested_id = optional_tab_id(&args);
            let tab = if requested_id.is_some() || state.active_id().is_some() {
                let id = resolve_tab_id(&state, requested_id)?;
                commands::navigate_browser_tab_inner(app, &state, &id, url.as_str())?
            } else {
                commands::open_browser_tab_inner(
                    app.clone(),
                    &state,
                    OpenBrowserTabInput {
                        url: url.to_string(),
                        bounds: None,
                    },
                )
                .await?
            };
            wait_for_page_ready(&state, &tab.id).await;
            let snapshot = commands::extract_browser_page_inner(app, &tab.id).await?;
            Ok(snapshot_to_tool_result(name, &tab, snapshot))
        }
        "browser_extract_page" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let snapshot = commands::extract_browser_page_inner(app, &id).await?;
            Ok(snapshot_to_tool_result(name, &tab, snapshot))
        }
        "browser_scroll_page" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let rounds = args
                .get("maxRounds")
                .and_then(Value::as_u64)
                .unwrap_or(6)
                .min(12) as u8;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let snapshot = commands::scroll_browser_page_inner(app, &id, rounds).await?;
            Ok(snapshot_to_tool_result(name, &tab, snapshot))
        }
        "browser_click_text" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let target = args
                .get("target")
                .and_then(Value::as_str)
                .ok_or_else(|| "browser_click_text 缺少 target".to_string())?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let snapshot = commands::click_browser_target_inner(app, &id, target).await?;
            Ok(snapshot_to_tool_result(name, &tab, snapshot))
        }
        "browser_extract_comments" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let limits = crate::tools::comments::CommentLimits::new(
                args.get("maxComments")
                    .and_then(Value::as_u64)
                    .unwrap_or(50)
                    .clamp(1, 100) as usize,
                args.get("maxChars")
                    .and_then(Value::as_u64)
                    .unwrap_or(2_000)
                    .clamp(100, 2_000) as usize,
            );
            commands::extract_browser_comments_inner(app, &id, limits).await
        }
        _ => Err(format!("未知浏览器工具：{name}")),
    }
}
