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

fn profile_directory(app: &AppHandle, profile_id: &str) -> Result<std::path::PathBuf, String> {
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?
        .join("browser-profiles")
        .join(profile_id);
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
    let url = normalize_browser_url(&url)?;
    webview(&app, &id)?
        .navigate(url.clone())
        .map_err(|error| format!("无法打开网页：{error}"))?;
    state
        .update(&id, |tab| {
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
    extract_page(&webview(&app, &id)?, EXTRACT_VISIBLE_PAGE_SCRIPT).await
}

#[tauri::command]
pub async fn scroll_browser_page(
    app: AppHandle,
    id: String,
    max_rounds: u8,
) -> Result<PageSnapshot, String> {
    let webview = webview(&app, &id)?;
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
    let target = validate_click_target(&target)?;
    let webview = webview(&app, &id)?;
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
