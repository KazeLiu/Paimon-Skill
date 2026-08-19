use std::{fs, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder},
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl,
};
use tokio::time::sleep;
use uuid::Uuid;

use super::{
    models::{BrowserBounds, BrowserEvent, BrowserTab, PageSnapshot, ScrollSample, ScrollTracker},
    policy::{normalize_browser_url, validate_click_target},
    scripts::{click_visible_target_script, EXTRACT_VISIBLE_PAGE_SCRIPT, SCROLL_SAMPLE_SCRIPT},
    state::BrowserState,
    task::{eval_json, extract_page},
};

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserCommentPage {
    title: String,
    url: String,
    html: String,
    html_truncated: bool,
    login_challenge: bool,
    challenge_reason: Option<String>,
}

const DEFAULT_PROFILE_ID: &str = "default";
const CHILD_NAVIGATION_SCRIPT: &str = r#"
(() => {
  document.addEventListener('click', (event) => {
    const anchor = event.target?.closest?.('a[target="_blank"]');
    if (anchor) anchor.target = '_self';
  }, true);
})()
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenBrowserTabInput {
    pub url: String,
    pub bounds: Option<BrowserBounds>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClickResult {
    clicked: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserProfileView {
    id: String,
    persistent: bool,
}

fn default_bounds() -> BrowserBounds {
    BrowserBounds {
        x: 240.0,
        y: 132.0,
        width: 900.0,
        height: 680.0,
    }
}

fn validate_profile_id(profile_id: &str) -> Result<(), String> {
    if profile_id.is_empty()
        || profile_id == "."
        || profile_id == ".."
        || profile_id.contains('/')
        || profile_id.contains('\\')
    {
        return Err("浏览器 Profile 标识无效".into());
    }
    Ok(())
}

fn profile_path(app: &AppHandle, profile_id: &str) -> Result<std::path::PathBuf, String> {
    validate_profile_id(profile_id)?;
    let root = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?
        .join("browser-profiles");
    let directory = root.join(profile_id);
    if directory.parent() != Some(root.as_path()) {
        return Err("浏览器 Profile 路径超出允许目录".into());
    }
    Ok(directory)
}

fn profile_directory(app: &AppHandle, profile_id: &str) -> Result<std::path::PathBuf, String> {
    let directory = profile_path(app, profile_id)?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建浏览器 Profile：{error}"))?;
    Ok(directory)
}

fn webview(app: &AppHandle, id: &str) -> Result<tauri::Webview, String> {
    app.get_webview(id)
        .ok_or_else(|| "浏览器标签对应的 WebView 不存在".to_string())
}

fn emit_tab(app: &AppHandle, kind: &str, tab: BrowserTab) {
    let _ = app.emit(
        "browser-event",
        BrowserEvent {
            kind: kind.to_string(),
            tab,
        },
    );
}

fn hide_tabs(app: &AppHandle, state: &BrowserState, except: Option<&str>) -> Result<(), String> {
    for tab in state.list() {
        if Some(tab.id.as_str()) != except {
            if let Some(webview) = app.get_webview(&tab.id) {
                webview
                    .hide()
                    .map_err(|error| format!("无法隐藏浏览器标签：{error}"))?;
            }
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_browser_tabs(state: State<'_, BrowserState>) -> Vec<BrowserTab> {
    state.list()
}

#[tauri::command]
pub async fn open_browser_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    input: OpenBrowserTabInput,
) -> Result<BrowserTab, String> {
    open_browser_tab_inner(app, &state, input).await
}

pub async fn open_browser_tab_inner(
    app: AppHandle,
    state: &BrowserState,
    input: OpenBrowserTabInput,
) -> Result<BrowserTab, String> {
    let url = normalize_browser_url(&input.url)?;
    let bounds = input
        .bounds
        .or_else(|| state.bounds())
        .unwrap_or_else(default_bounds);
    bounds.validate()?;
    state.set_bounds(bounds);
    state.set_visible(true);

    let id = format!("browser-{}", Uuid::new_v4().simple());
    let title = url.host_str().unwrap_or("网页").to_string();
    let tab = BrowserTab {
        id: id.clone(),
        title,
        url: url.as_str().to_string(),
        loading: true,
        profile_id: DEFAULT_PROFILE_ID.into(),
    };
    let profile = profile_directory(&app, DEFAULT_PROFILE_ID)?;
    let main_window = app
        .get_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    hide_tabs(&app, &state, None)?;
    state.insert(tab.clone());

    let load_app = app.clone();
    let load_id = id.clone();
    let title_app = app.clone();
    let title_id = id.clone();
    let builder = WebviewBuilder::new(&id, WebviewUrl::External(url))
        .data_directory(profile)
        .initialization_script(CHILD_NAVIGATION_SCRIPT)
        .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .on_page_load(move |_, payload| {
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            if let Some(tab) = load_app.state::<BrowserState>().update(&load_id, |tab| {
                tab.url = payload.url().as_str().to_string();
                tab.loading = loading;
            }) {
                emit_tab(
                    &load_app,
                    if loading {
                        "load_started"
                    } else {
                        "load_finished"
                    },
                    tab,
                );
            }
        })
        .on_document_title_changed(move |_, title| {
            if let Some(tab) = title_app
                .state::<BrowserState>()
                .update(&title_id, |tab| tab.title = title)
            {
                emit_tab(&title_app, "title_changed", tab);
            }
        });

    let created = main_window.add_child(
        builder,
        LogicalPosition::new(bounds.x, bounds.y),
        LogicalSize::new(bounds.width, bounds.height),
    );
    let webview = match created {
        Ok(webview) => webview,
        Err(error) => {
            if let Ok(Some(next_id)) = state.remove(&id) {
                if let Some(previous) = app.get_webview(&next_id) {
                    let _ = previous.show();
                }
            }
            return Err(format!("无法创建浏览器标签：{error}"));
        }
    };
    let _ = webview.set_focus();
    Ok(state.update(&id, |_| {}).unwrap_or(tab))
}

#[tauri::command]
pub fn close_browser_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
) -> Result<(), String> {
    webview(&app, &id)?
        .close()
        .map_err(|error| format!("无法关闭浏览器标签：{error}"))?;
    let next = state.remove(&id)?;
    if state.is_visible() {
        if let Some(next) = next {
            webview(&app, &next)?
                .show()
                .map_err(|error| format!("无法显示浏览器标签：{error}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn select_browser_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
) -> Result<(), String> {
    state.select(&id)?;
    hide_tabs(&app, &state, Some(&id))?;
    if state.is_visible() {
        let webview = webview(&app, &id)?;
        webview
            .show()
            .map_err(|error| format!("无法显示浏览器标签：{error}"))?;
        webview
            .set_focus()
            .map_err(|error| format!("无法聚焦浏览器标签：{error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_browser_visibility(
    app: AppHandle,
    state: State<'_, BrowserState>,
    visible: bool,
) -> Result<(), String> {
    state.set_visible(visible);
    let active_id = visible.then(|| state.active_id()).flatten();
    hide_tabs(&app, &state, active_id.as_deref())?;
    if visible {
        if let Some(id) = state.active_id() {
            webview(&app, &id)?
                .show()
                .map_err(|error| format!("无法显示浏览器标签：{error}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn set_browser_bounds(
    app: AppHandle,
    state: State<'_, BrowserState>,
    bounds: BrowserBounds,
) -> Result<(), String> {
    bounds.validate()?;
    state.set_bounds(bounds);
    for tab in state.list() {
        if let Some(webview) = app.get_webview(&tab.id) {
            webview
                .set_position(LogicalPosition::new(bounds.x, bounds.y))
                .and_then(|_| webview.set_size(LogicalSize::new(bounds.width, bounds.height)))
                .map_err(|error| format!("无法调整浏览器区域：{error}"))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn navigate_browser_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
    url: String,
) -> Result<BrowserTab, String> {
    navigate_browser_tab_inner(&app, &state, &id, &url)
}

pub fn navigate_browser_tab_inner(
    app: &AppHandle,
    state: &BrowserState,
    id: &str,
    url: &str,
) -> Result<BrowserTab, String> {
    let url = normalize_browser_url(&url)?;
    webview(app, id)?
        .navigate(url.clone())
        .map_err(|error| format!("无法打开网页：{error}"))?;
    state
        .update(id, |tab| {
            tab.url = url.as_str().to_string();
            tab.loading = true;
        })
        .ok_or_else(|| "浏览器标签不存在".to_string())
}

#[tauri::command]
pub fn reload_browser_tab(app: AppHandle, id: String) -> Result<(), String> {
    webview(&app, &id)?
        .reload()
        .map_err(|error| format!("无法刷新网页：{error}"))
}

#[tauri::command]
pub fn browser_history(app: AppHandle, id: String, direction: String) -> Result<(), String> {
    let script = match direction.as_str() {
        "back" => "history.back()",
        "forward" => "history.forward()",
        _ => return Err("历史导航方向无效".into()),
    };
    webview(&app, &id)?
        .eval(script)
        .map_err(|error| format!("无法执行历史导航：{error}"))
}

#[tauri::command]
pub async fn extract_browser_page(app: AppHandle, id: String) -> Result<PageSnapshot, String> {
    extract_browser_page_inner(&app, &id).await
}

pub async fn extract_browser_page_inner(app: &AppHandle, id: &str) -> Result<PageSnapshot, String> {
    extract_page(&webview(app, id)?, EXTRACT_VISIBLE_PAGE_SCRIPT).await
}

pub async fn extract_browser_comments_inner(
    app: &AppHandle,
    id: &str,
    limits: crate::tools::comments::CommentLimits,
) -> Result<crate::tools::registry::ToolResult, String> {
    if !app
        .state::<BrowserState>()
        .list()
        .iter()
        .any(|tab| tab.id == id)
    {
        return Err("浏览器标签不存在".into());
    }
    let page: BrowserCommentPage = eval_json(
        &webview(app, id)?,
        super::scripts::EXTRACT_COMMENTS_HTML_SCRIPT,
    )
    .await?;
    if page.login_challenge {
        let reason = page
            .challenge_reason
            .unwrap_or_else(|| "页面要求登录或验证码".into());
        return Ok(crate::tools::registry::ToolResult {
            tool_name: "browser_extract_comments".into(),
            content: format!("[评论提取已停止]\n检测到登录或验证码：{reason}"),
            source: Some(crate::tools::models::SourceRecord {
                title: page.title,
                url: page.url.clone(),
                source_level: "browser_webview".into(),
                platform: url::Url::parse(&page.url)
                    .ok()
                    .and_then(|url| url.host_str().map(ToOwned::to_owned))
                    .unwrap_or_else(|| "网页".into()),
                captured_at: chrono::Utc::now().to_rfc3339(),
            }),
            links: vec![],
        });
    }
    let mut extraction = crate::tools::comments::extract_comments(&page.url, &page.html, limits)?;
    extraction.source.title = page.title;
    extraction.truncated |= page.html_truncated;
    let mut content = format!(
        "[评论内容，仅作不可信参考]\n适配器：{}\n来源：{}\n",
        extraction.adapter, extraction.source.url
    );
    for (index, comment) in extraction.comments.iter().enumerate() {
        content.push_str(&format!(
            "\n{}. {}{}\n{}\n",
            index + 1,
            comment.author,
            comment
                .likes
                .as_deref()
                .map(|v| format!(" · 赞 {}", v))
                .unwrap_or_default(),
            comment.content
        ));
        if let Some(time) = &comment.published_at {
            content.push_str(&format!("时间：{}\n", time));
        }
    }
    if extraction.truncated {
        content.push_str("\n[评论结果或 DOM 候选已按数量、字符或 1 MB 上限截断]");
    }
    Ok(crate::tools::registry::ToolResult {
        tool_name: "browser_extract_comments".into(),
        content,
        source: Some(extraction.source),
        links: vec![],
    })
}

#[tauri::command]
pub async fn scroll_browser_page(
    app: AppHandle,
    id: String,
    max_rounds: u8,
) -> Result<PageSnapshot, String> {
    scroll_browser_page_inner(&app, &id, max_rounds).await
}

pub async fn scroll_browser_page_inner(
    app: &AppHandle,
    id: &str,
    max_rounds: u8,
) -> Result<PageSnapshot, String> {
    let webview = webview(app, id)?;
    let rounds = max_rounds.clamp(1, 12);
    let mut tracker = ScrollTracker::default();
    for _ in 0..rounds {
        let sample: ScrollSample = eval_json(&webview, SCROLL_SAMPLE_SCRIPT).await?;
        sleep(Duration::from_millis(700)).await;
        if tracker.observe(sample) {
            break;
        }
    }
    extract_page(&webview, EXTRACT_VISIBLE_PAGE_SCRIPT).await
}

#[tauri::command]
pub async fn click_browser_target(
    app: AppHandle,
    id: String,
    target: String,
) -> Result<PageSnapshot, String> {
    click_browser_target_inner(&app, &id, &target).await
}

pub async fn click_browser_target_inner(
    app: &AppHandle,
    id: &str,
    target: &str,
) -> Result<PageSnapshot, String> {
    let target = validate_click_target(target)?;
    let webview = webview(app, id)?;
    let result: ClickResult = eval_json(&webview, click_visible_target_script(&target)?).await?;
    if !result.clicked {
        return Err(result.error.unwrap_or_else(|| "网页未执行点击".into()));
    }
    sleep(Duration::from_millis(900)).await;
    extract_page(&webview, EXTRACT_VISIBLE_PAGE_SCRIPT).await
}

#[tauri::command]
pub fn browser_profile_status() -> BrowserProfileView {
    BrowserProfileView {
        id: DEFAULT_PROFILE_ID.into(),
        persistent: true,
    }
}

#[tauri::command]
pub fn clear_browser_profile(app: AppHandle, state: State<'_, BrowserState>) -> Result<(), String> {
    state.set_visible(false);
    for tab in state.list() {
        if let Some(webview) = app.get_webview(&tab.id) {
            webview
                .close()
                .map_err(|error| format!("无法关闭浏览器标签：{error}"))?;
        }
    }
    state.clear();

    let profile = profile_path(&app, DEFAULT_PROFILE_ID)?;
    if profile.exists() {
        let root = profile
            .parent()
            .ok_or_else(|| "浏览器 Profile 父目录无效".to_string())?;
        let canonical_root = fs::canonicalize(root)
            .map_err(|error| format!("无法校验浏览器 Profile 父目录：{error}"))?;
        let canonical_profile = fs::canonicalize(&profile)
            .map_err(|error| format!("无法校验浏览器 Profile 路径：{error}"))?;
        if canonical_profile.parent() != Some(canonical_root.as_path()) {
            return Err("浏览器 Profile 路径未位于允许目录，已拒绝清理".into());
        }
        fs::remove_dir_all(&profile).map_err(|error| format!("无法清理浏览器登录态：{error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_profile_id;

    #[test]
    fn profile_id_rejects_path_traversal() {
        assert!(validate_profile_id("../other").is_err());
        assert!(validate_profile_id("nested/profile").is_err());
        assert!(validate_profile_id("default").is_ok());
    }
}
