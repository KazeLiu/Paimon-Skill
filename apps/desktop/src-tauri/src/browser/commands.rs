use std::{fs, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{
    webview::{NewWindowResponse, PageLoadEvent, WebviewWindowBuilder},
    AppHandle, Emitter, Manager, State, WebviewUrl, WindowEvent,
};
use tokio::time::sleep;
use uuid::Uuid;

use super::{
    models::{BrowserEvent, BrowserTab, PageSnapshot, ScrollSample, ScrollTracker},
    policy::{normalize_browser_url, validate_click_target},
    scripts::{
        click_element_script, click_visible_target_script, semantic_scan_script,
        SCROLL_SAMPLE_SCRIPT,
    },
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

pub(super) const DEFAULT_PROFILE_ID: &str = "default";
pub(crate) const CHILD_NAVIGATION_SCRIPT: &str = r#"
(() => {
  const mark = () => document.querySelectorAll('a[href]').forEach((anchor) => {
    anchor.target = '_blank';
    anchor.rel = 'noopener noreferrer';
  });
  mark();
  new MutationObserver(mark).observe(document.documentElement, { childList: true, subtree: true });
})()
"#;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenBrowserTabInput {
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserWindowPresentation {
    Foreground,
    Background,
}

impl BrowserWindowPresentation {
    fn visible(self) -> bool {
        matches!(self, Self::Foreground)
    }

    fn focused(self) -> bool {
        matches!(self, Self::Foreground)
    }

    fn allows_child_windows(self) -> bool {
        matches!(self, Self::Foreground)
    }
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceConnectionResult {
    reachable: bool,
    status_code: Option<u16>,
    message: String,
}

fn source_status_is_reachable(_status: reqwest::StatusCode) -> bool {
    true
}

#[tauri::command]
pub async fn check_source_connection(url: String) -> Result<SourceConnectionResult, String> {
    let url = super::policy::validate_source_connection_url(&url)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .user_agent("Paimon Desktop source checker/0.1")
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| format!("无法创建连接检测器：{error}"))?;
    let response = client
        .get(url.clone())
        .header(reqwest::header::RANGE, "bytes=0-0")
        .send()
        .await;
    match response {
        Ok(response) => {
            let status = response.status();
            Ok(SourceConnectionResult {
                reachable: source_status_is_reachable(status),
                status_code: Some(status.as_u16()),
                message: format!("HTTP {}", status.as_u16()),
            })
        }
        Err(error) => Ok(SourceConnectionResult {
            reachable: false,
            status_code: None,
            message: format!("连接失败：{error}"),
        }),
    }
}

#[tauri::command]
pub async fn check_source_login(
    app: AppHandle,
    source_id: String,
) -> Result<super::login::SourceLoginResult, String> {
    super::login::check_source_login(&app, &source_id).await
}

fn browser_window_title(title: &str, url: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        return url.to_string();
    }
    format!("{title} · {url}")
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

pub(super) fn profile_directory(
    app: &AppHandle,
    profile_id: &str,
) -> Result<std::path::PathBuf, String> {
    let directory = profile_path(app, profile_id)?;
    fs::create_dir_all(&directory).map_err(|error| format!("无法创建浏览器 Profile：{error}"))?;
    Ok(directory)
}

fn webview(app: &AppHandle, id: &str) -> Result<tauri::Webview, String> {
    app.get_webview_window(id)
        .map(|window| window.as_ref().clone())
        .ok_or_else(|| "浏览器标签对应的窗口不存在".to_string())
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

fn remove_tab_and_emit(app: &AppHandle, id: &str) {
    let state = app.state::<BrowserState>();
    if let Some(tab) = state.update(id, |_| {}) {
        let _ = state.remove(id);
        emit_tab(app, "closed", tab);
    }
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
    open_browser_tab_with_presentation(app, state, input, BrowserWindowPresentation::Foreground)
        .await
}

pub async fn open_background_browser_tab_inner(
    app: AppHandle,
    state: &BrowserState,
    input: OpenBrowserTabInput,
) -> Result<BrowserTab, String> {
    open_browser_tab_with_presentation(app, state, input, BrowserWindowPresentation::Background)
        .await
}

async fn open_browser_tab_with_presentation(
    app: AppHandle,
    state: &BrowserState,
    input: OpenBrowserTabInput,
    presentation: BrowserWindowPresentation,
) -> Result<BrowserTab, String> {
    let url = normalize_browser_url(&input.url)?;

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
    state.insert(tab.clone());

    let load_app = app.clone();
    let load_id = id.clone();
    let title_app = app.clone();
    let title_id = id.clone();
    let builder = WebviewWindowBuilder::new(&app, &id, WebviewUrl::External(url))
        .title(browser_window_title(&tab.title, &tab.url))
        .inner_size(1120.0, 760.0)
        .min_inner_size(720.0, 480.0)
        .center()
        .visible(presentation.visible())
        .focused(presentation.focused())
        .data_directory(profile)
        .devtools(true)
        .initialization_script(CHILD_NAVIGATION_SCRIPT)
        .on_navigation(|url| matches!(url.scheme(), "http" | "https"))
        .on_new_window(move |_, _| {
            if presentation.allows_child_windows() {
                NewWindowResponse::Allow
            } else {
                NewWindowResponse::Deny
            }
        })
        .on_download(|_, _| false)
        .on_page_load(move |window, payload| {
            let loading = matches!(payload.event(), PageLoadEvent::Started);
            if let Some(tab) = load_app.state::<BrowserState>().update(&load_id, |tab| {
                tab.url = payload.url().as_str().to_string();
                tab.loading = loading;
            }) {
                let _ = window.set_title(&browser_window_title(&tab.title, &tab.url));
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
        .on_document_title_changed(move |window, title| {
            if let Some(tab) = title_app
                .state::<BrowserState>()
                .update(&title_id, |tab| tab.title = title)
            {
                let _ = window.set_title(&browser_window_title(&tab.title, &tab.url));
                emit_tab(&title_app, "title_changed", tab);
            }
        });
    let builder = if let Some(main_window) = app.get_webview_window("main") {
        builder
            .parent(&main_window)
            .map_err(|error| format!("无法关联浏览器子窗口：{error}"))?
    } else {
        builder
    };

    let window = match builder.build() {
        Ok(window) => window,
        Err(error) => {
            let _ = state.remove(&id);
            return Err(format!("无法创建浏览器标签：{error}"));
        }
    };
    let close_app = app.clone();
    let close_id = id.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) {
            remove_tab_and_emit(&close_app, &close_id);
        }
    });
    if presentation.focused() {
        let _ = window.set_focus();
    }
    Ok(state.update(&id, |_| {}).unwrap_or(tab))
}

#[tauri::command]
pub fn close_browser_tab(app: AppHandle, id: String) -> Result<(), String> {
    app.get_webview_window(&id)
        .ok_or_else(|| "浏览器标签对应的窗口不存在".to_string())?
        .close()
        .map_err(|error| format!("无法关闭浏览器标签：{error}"))?;
    remove_tab_and_emit(&app, &id);
    Ok(())
}

#[tauri::command]
pub fn select_browser_tab(
    app: AppHandle,
    state: State<'_, BrowserState>,
    id: String,
) -> Result<(), String> {
    state.select(&id)?;
    let window = app
        .get_webview_window(&id)
        .ok_or_else(|| "浏览器标签对应的窗口不存在".to_string())?;
    window
        .show()
        .and_then(|_| window.unminimize())
        .and_then(|_| window.set_focus())
        .map_err(|error| format!("无法聚焦浏览器窗口：{error}"))?;
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

#[tauri::command]
pub async fn find_browser_context(
    app: AppHandle,
    id: String,
    query: String,
) -> Result<serde_json::Value, String> {
    find_browser_context_inner(&app, &id, &query).await
}

#[tauri::command]
pub async fn click_browser_element(
    app: AppHandle,
    id: String,
    element_id: String,
) -> Result<PageSnapshot, String> {
    click_browser_element_inner(&app, &id, &element_id).await
}

pub async fn extract_browser_page_inner(app: &AppHandle, id: &str) -> Result<PageSnapshot, String> {
    extract_page(&webview(app, id)?, &semantic_scan_script()).await
}

pub async fn extract_bilibili_subtitles_inner(
    app: &AppHandle,
    id: &str,
    all_parts: bool,
) -> Result<serde_json::Value, String> {
    if !app
        .state::<BrowserState>()
        .list()
        .iter()
        .any(|tab| tab.id == id)
    {
        return Err("浏览器标签不存在".into());
    }
    eval_json(
        &webview(app, id)?,
        &super::scripts::extract_bilibili_subtitles_script(all_parts),
    )
    .await
}

pub async fn extract_bilibili_keyframes_inner(
    app: &AppHandle,
    id: &str,
    timestamps_ms: Vec<u64>,
) -> Result<serde_json::Value, String> {
    if !app
        .state::<BrowserState>()
        .list()
        .iter()
        .any(|tab| tab.id == id)
    {
        return Err("浏览器标签不存在".into());
    }
    let timestamps = timestamps_ms.into_iter().take(6).collect::<Vec<_>>();
    eval_json(
        &webview(app, id)?,
        &super::scripts::extract_bilibili_keyframes_script(&timestamps)?,
    )
    .await
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
            metadata: std::collections::BTreeMap::from([(
                "tabId".into(),
                serde_json::Value::String(id.to_string()),
            )]),
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
        metadata: std::collections::BTreeMap::from([(
            "tabId".into(),
            serde_json::Value::String(id.to_string()),
        )]),
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
    extract_page(&webview, &semantic_scan_script()).await
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
    extract_page(&webview, &semantic_scan_script()).await
}

pub async fn find_browser_context_inner(
    app: &AppHandle,
    id: &str,
    query: &str,
) -> Result<serde_json::Value, String> {
    let webview = webview(app, id)?;
    eval_json(&webview, super::scripts::find_page_context_script(query)?).await
}

pub async fn click_browser_element_inner(
    app: &AppHandle,
    id: &str,
    element_id: &str,
) -> Result<PageSnapshot, String> {
    let webview = webview(app, id)?;
    let result: ClickResult = eval_json(&webview, click_element_script(element_id)?).await?;
    if !result.clicked {
        return Err(result.error.unwrap_or_else(|| "网页未执行点击".into()));
    }
    sleep(Duration::from_millis(700)).await;
    extract_page(&webview, &semantic_scan_script()).await
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
    super::login::close_cookie_probe(&app);
    for tab in state.list() {
        if let Some(window) = app.get_webview_window(&tab.id) {
            window
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
    use super::{
        browser_window_title, source_status_is_reachable, validate_profile_id,
        BrowserWindowPresentation,
    };

    #[test]
    fn agent_browser_windows_are_hidden_and_do_not_take_focus() {
        assert!(!BrowserWindowPresentation::Background.visible());
        assert!(!BrowserWindowPresentation::Background.focused());
        assert!(!BrowserWindowPresentation::Background.allows_child_windows());
        assert!(BrowserWindowPresentation::Foreground.visible());
        assert!(BrowserWindowPresentation::Foreground.focused());
        assert!(BrowserWindowPresentation::Foreground.allows_child_windows());
    }

    #[test]
    fn profile_id_rejects_path_traversal() {
        assert!(validate_profile_id("../other").is_err());
        assert!(validate_profile_id("nested/profile").is_err());
        assert!(validate_profile_id("default").is_ok());
    }

    #[test]
    fn browser_window_title_contains_the_page_title_and_url() {
        assert_eq!(
            browser_window_title("米游社观测枢", "https://baike.mihoyo.com/ys/obc/"),
            "米游社观测枢 · https://baike.mihoyo.com/ys/obc/"
        );
    }

    #[test]
    fn any_http_response_proves_the_source_is_reachable() {
        assert!(source_status_is_reachable(reqwest::StatusCode::OK));
        assert!(source_status_is_reachable(reqwest::StatusCode::FORBIDDEN));
        assert!(source_status_is_reachable(
            reqwest::StatusCode::SERVICE_UNAVAILABLE
        ));
    }
}
