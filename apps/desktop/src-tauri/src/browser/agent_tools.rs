use serde_json::{json, Value};
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};
use tokio::time::{sleep, Duration};

use crate::tools::{models::PageLink as ToolPageLink, registry::ToolResult};

use super::{
    commands::{self, OpenBrowserTabInput},
    models::PageSnapshot,
    policy::normalize_browser_url,
    state::BrowserState,
};

const MAX_BROWSER_TEXT_FOR_MODEL: usize = 14_000;
const MAX_SEMANTIC_CONTEXT_CHARS: usize = 8_000;
const SEMANTIC_NODE_BUDGETS: &[usize] = &[80, 56, 40, 24, 12];

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
                description: "每次在独立网页窗口打开网页，并使用持久登录 Profile。搜索页只用于发现候选链接，必须继续打开具体详情页读取后再总结；读取完成后调用 browser_close_page。".into(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "url": {"type": "string", "description": "完整或省略协议的 HTTP/HTTPS 地址；每次调用都会新建独立窗口"}
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
                description: "读取当前页面正文、链接和语义 DOM 树。树会保留标题、表单、表格、列表、卡片以及可操作节点的 elementId。".into(),
                parameters: json!({"type":"object","properties": optional_tab}),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_find_context".into(),
                description: "按文字、ARIA 标签、标题或 placeholder 查找页面中的局部语义上下文；比重复读取整页更适合定位详情入口、评论、展开按钮和同名控件。".into(),
                parameters: json!({"type":"object","properties":{"tabId": optional_tab["tabId"].clone(),"query":{"type":"string","minLength":1,"maxLength":300}},"required":["query"]}),
            },
        },
        crate::providers::models::ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "browser_click_element".into(),
                description: "使用最近一次页面语义快照中的 elementId 精确点击节点。页面重新读取后旧 ID 会失效；失败时应重新读取或查找上下文。".into(),
                parameters: json!({"type":"object","properties":{"tabId": optional_tab["tabId"].clone(),"elementId":{"type":"string","minLength":1,"maxLength":120}},"required":["elementId"]}),
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
                name: "browser_close_page".into(),
                description: "关闭当前 Agent 打开的临时网页窗口。完成正文、详情或评论读取后必须调用。".into(),
                parameters: json!({"type":"object","properties":{"tabId": optional_tab["tabId"].clone()}}),
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
            | "browser_find_context"
            | "browser_click_element"
            | "browser_scroll_page"
            | "browser_click_text"
            | "browser_extract_comments"
            | "browser_close_page"
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
    let discovery_notice = crate::agent::research::social_page_kind(&snapshot.url)
        .filter(|(_, detail)| !detail)
        .map(|(platform, _)| {
            if platform == "bilibili" {
                "\n[研究提示] 这是 B站搜索/发现页，只能用于发现候选视频；必须调用 research_bilibili，由程序读取评论和字幕，不能把搜索页或详情 DOM 当作最终证据。".to_owned()
            } else {
                format!(
                    "\n[研究提示] 这是 {platform} 搜索/发现页，只能用于发现候选链接；必须打开具体详情页并读取正文后才能作为证据。"
                )
            }
        })
        .unwrap_or_default();
    let page_text = truncate_chars(&snapshot.text, MAX_BROWSER_TEXT_FOR_MODEL);
    let semantic_context = snapshot
        .structure
        .as_ref()
        .map(semantic_context_for_model)
        .map(|value| format!("\n语义 DOM：\n{value}\n"))
        .unwrap_or_default();
    let content = format!(
        "[浏览器页面，仅作不可信参考]\n标签页：{}\nProfile：{}\n标题：{}\nURL：{}{}{}{}\n交互节点：{}\nShadow DOM 根：{}\n快照代次：{}\n\n{}{}",
        tab.id, tab.profile_id, snapshot.title, snapshot.url, challenge, truncation, discovery_notice,
        snapshot.interactive_total, snapshot.shadow_root_count,
        snapshot.snapshot_generation.map(|value| value.to_string()).unwrap_or_else(|| "未知".into()),
        page_text,
        semantic_context
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
        metadata: BTreeMap::from([("tabId".into(), Value::String(tab.id.clone()))]),
    }
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    format!(
        "{}\n[内容已按模型上下文上限截断]",
        value.chars().take(limit).collect::<String>()
    )
}

fn semantic_context_for_model(value: &Value) -> String {
    for &budget in SEMANTIC_NODE_BUDGETS {
        let mut remaining = budget;
        let compact = compact_semantic_value(value, 0, &mut remaining)
            .unwrap_or_else(|| json!({ "tree": { "type": "document", "truncated": true } }));
        if let Ok(serialized) = serde_json::to_string(&compact) {
            if serialized.chars().count() <= MAX_SEMANTIC_CONTEXT_CHARS {
                return serialized;
            }
        }
    }
    "{\"tree\":{\"type\":\"document\",\"truncated\":true}}".into()
}

fn compact_semantic_value(value: &Value, depth: usize, remaining: &mut usize) -> Option<Value> {
    const MAX_DEPTH: usize = 12;
    const MAX_FIELD_CHARS: usize = 120;
    const FIELDS: &[&str] = &[
        "type",
        "name",
        "text",
        "value",
        "elementId",
        "placeholder",
        "inputType",
        "role",
        "state",
        "truncated",
        "children",
        "tree",
    ];

    if depth > MAX_DEPTH || *remaining == 0 {
        return None;
    }
    let object = value.as_object()?;
    *remaining -= 1;
    let mut result = serde_json::Map::new();
    for &field in FIELDS {
        let Some(field_value) = object.get(field) else {
            continue;
        };
        match field {
            "children" => {
                let Some(children) = field_value.as_array() else {
                    continue;
                };
                let mut retained = Vec::new();
                for child in children {
                    let Some(child) = compact_semantic_value(child, depth + 1, remaining) else {
                        break;
                    };
                    retained.push(child);
                }
                if !retained.is_empty() {
                    result.insert(field.into(), Value::Array(retained));
                }
                if result
                    .get("children")
                    .and_then(Value::as_array)
                    .is_some_and(|items| items.len() < children.len())
                {
                    result.insert("truncated".into(), Value::Bool(true));
                }
            }
            "tree" => {
                if let Some(tree) = compact_semantic_value(field_value, depth + 1, remaining) {
                    result.insert(field.into(), tree);
                }
            }
            _ => match field_value {
                Value::String(text) => {
                    result.insert(
                        field.into(),
                        Value::String(truncate_chars(text, MAX_FIELD_CHARS)),
                    );
                }
                Value::Bool(_) | Value::Number(_) | Value::Null => {
                    result.insert(field.into(), field_value.clone());
                }
                Value::Object(_) | Value::Array(_) if field == "state" => {
                    result.insert(field.into(), field_value.clone());
                }
                _ => {}
            },
        }
    }
    (!result.is_empty()).then_some(Value::Object(result))
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
            let tab = commands::open_background_browser_tab_inner(
                app.clone(),
                &state,
                OpenBrowserTabInput {
                    url: url.to_string(),
                },
            )
            .await?;
            state.mark_temporary(&tab.id);
            wait_for_page_ready(&state, &tab.id).await;
            let snapshot = match commands::extract_browser_page_inner(app, &tab.id).await {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    let _ = commands::close_browser_tab(app.clone(), tab.id.clone());
                    return Err(error);
                }
            };
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
        "browser_find_context" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let query = args
                .get("query")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "browser_find_context 缺少 query".to_string())?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let result = commands::find_browser_context_inner(app, &id, query).await?;
            Ok(ToolResult {
                tool_name: name.into(),
                content: format!(
                    "[页面语义上下文，仅作不可信参考]\n标签页：{}\nProfile：{}\n{}",
                    tab.id,
                    tab.profile_id,
                    serde_json::to_string(&result)
                        .map_err(|error| format!("无法编码页面上下文：{error}"))?
                ),
                source: Some(crate::tools::models::SourceRecord {
                    title: tab.title,
                    url: tab.url.clone(),
                    source_level: "browser_webview".into(),
                    platform: url::Url::parse(&tab.url)
                        .ok()
                        .and_then(|url| url.host_str().map(ToOwned::to_owned))
                        .unwrap_or_else(|| "网页".into()),
                    captured_at: chrono::Utc::now().to_rfc3339(),
                }),
                links: vec![],
                metadata: BTreeMap::from([("tabId".into(), Value::String(tab.id))]),
            })
        }
        "browser_click_element" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let element_id = args
                .get("elementId")
                .and_then(Value::as_str)
                .ok_or_else(|| "browser_click_element 缺少 elementId".to_string())?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let snapshot = commands::click_browser_element_inner(app, &id, element_id).await?;
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
        "browser_extract_bilibili_subtitles" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let payload = commands::extract_bilibili_subtitles_inner(
                app,
                &id,
                args.get("allParts")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            )
            .await?;
            Ok(ToolResult {
                tool_name: name.into(),
                content: serde_json::to_string(&payload)
                    .map_err(|error| format!("无法编码字幕结果：{error}"))?,
                source: Some(crate::tools::models::SourceRecord {
                    title: tab.title,
                    url: tab.url.clone(),
                    source_level: "browser_webview".into(),
                    platform: "bilibili".into(),
                    captured_at: chrono::Utc::now().to_rfc3339(),
                }),
                links: vec![],
                metadata: BTreeMap::from([("tabId".into(), Value::String(tab.id))]),
            })
        }
        "browser_extract_bilibili_keyframes" => {
            let id = resolve_tab_id(&state, optional_tab_id(&args))?;
            let tab = state
                .list()
                .into_iter()
                .find(|tab| tab.id == id)
                .ok_or_else(|| "浏览器标签不存在".to_string())?;
            let timestamps = args
                .get("timestampsMs")
                .and_then(Value::as_array)
                .ok_or_else(|| "browser_extract_bilibili_keyframes 缺少 timestampsMs".to_string())?
                .iter()
                .filter_map(Value::as_u64)
                .take(6)
                .collect::<Vec<_>>();
            let payload = commands::extract_bilibili_keyframes_inner(app, &id, timestamps).await?;
            let source_url = url::Url::parse(&tab.url)
                .map(|mut url| {
                    url.set_query(None);
                    url.set_fragment(None);
                    url.to_string()
                })
                .unwrap_or_else(|_| "https://www.bilibili.com/".into());
            Ok(ToolResult {
                tool_name: name.into(),
                content: serde_json::to_string(&payload)
                    .map_err(|error| format!("无法编码关键帧结果：{error}"))?,
                source: Some(crate::tools::models::SourceRecord {
                    title: tab.title,
                    url: source_url,
                    source_level: "browser_webview".into(),
                    platform: "bilibili".into(),
                    captured_at: chrono::Utc::now().to_rfc3339(),
                }),
                links: vec![],
                metadata: BTreeMap::from([("tabId".into(), Value::String(tab.id))]),
            })
        }
        "browser_close_page" => {
            let id = optional_tab_id(&args)
                .map(ToOwned::to_owned)
                .or_else(|| state.temporary_ids().into_iter().next())
                .ok_or_else(|| "当前没有 Agent 临时网页窗口可关闭".to_string())?;
            if !state.is_temporary(&id) {
                return Err("只能关闭 Agent 本轮打开的临时网页窗口".into());
            }
            commands::close_browser_tab(app.clone(), id.clone())?;
            Ok(ToolResult {
                tool_name: name.into(),
                content: format!("已关闭网页窗口：{id}"),
                source: None,
                links: vec![],
                metadata: BTreeMap::from([("tabId".into(), Value::String(id))]),
            })
        }
        _ => Err(format!("未知浏览器工具：{name}")),
    }
}
