use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewUrl};

use super::commands::{profile_directory, DEFAULT_PROFILE_ID};

const COOKIE_PROBE_LABEL: &str = "browser-cookie-probe";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceLoginStatus {
    LoggedIn,
    LoggedOut,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceLoginResult {
    pub status: SourceLoginStatus,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CookieMetadata {
    name: String,
    expires: f64,
    is_session: bool,
    has_value: bool,
}

#[derive(Debug, Clone, Copy)]
struct LoginCookieRule {
    source_id: &'static str,
    uri: &'static str,
    cookie_names: &'static [&'static str],
}

const LOGIN_COOKIE_RULES: &[LoginCookieRule] = &[
    LoginCookieRule {
        source_id: "xiaohongshu",
        uri: "https://www.xiaohongshu.com/",
        cookie_names: &["web_session"],
    },
    LoginCookieRule {
        source_id: "zhihu",
        uri: "https://www.zhihu.com/",
        cookie_names: &["z_c0"],
    },
];

pub async fn check_source_login(
    app: &AppHandle,
    source_id: &str,
) -> Result<SourceLoginResult, String> {
    let rule = LOGIN_COOKIE_RULES
        .iter()
        .find(|rule| rule.source_id == source_id)
        .ok_or_else(|| "该信息源没有 Cookie 登录检测适配".to_string())?;
    let cookies = read_cookie_metadata(app, rule.uri).await?;
    Ok(evaluate_login(rule, &cookies, unix_timestamp()))
}

fn unix_timestamp() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

fn evaluate_login(
    rule: &LoginCookieRule,
    cookies: &[CookieMetadata],
    now: f64,
) -> SourceLoginResult {
    let logged_in = cookies.iter().any(|cookie| {
        rule.cookie_names
            .iter()
            .any(|name| cookie.name.eq_ignore_ascii_case(name))
            && cookie.has_value
            && (cookie.is_session || cookie.expires <= 0.0 || cookie.expires > now)
    });
    if logged_in {
        SourceLoginResult {
            status: SourceLoginStatus::LoggedIn,
            message: "检测到有效登录 Cookie".into(),
        }
    } else {
        SourceLoginResult {
            status: SourceLoginStatus::LoggedOut,
            message: "未检测到有效登录 Cookie".into(),
        }
    }
}

#[cfg(windows)]
async fn read_cookie_metadata(
    app: &AppHandle,
    uri: &'static str,
) -> Result<Vec<CookieMetadata>, String> {
    use std::sync::{Arc, Mutex};

    use tauri::webview::{NewWindowResponse, WebviewWindowBuilder};
    use tokio::sync::oneshot;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2CookieList, ICoreWebView2_2};
    use webview2_com::{CoTaskMemPWSTR, GetCookiesCompletedHandler};
    use windows::core::Interface;

    let probe = if let Some(window) = app.get_webview_window(COOKIE_PROBE_LABEL) {
        window
    } else {
        let profile = profile_directory(app, DEFAULT_PROFILE_ID)?;
        WebviewWindowBuilder::new(
            app,
            COOKIE_PROBE_LABEL,
            WebviewUrl::External(
                url::Url::parse("about:blank")
                    .map_err(|error| format!("无法创建 Cookie 探针地址：{error}"))?,
            ),
        )
        .title("派蒙助手登录态检测")
        .inner_size(1.0, 1.0)
        .visible(false)
        .focused(false)
        .data_directory(profile)
        .devtools(false)
        .on_navigation(|url| url.as_str() == "about:blank")
        .on_new_window(|_, _| NewWindowResponse::Deny)
        .on_download(|_, _| false)
        .build()
        .map_err(|error| format!("无法创建 WebView2 Cookie 探针：{error}"))?
    };

    let (sender, receiver) = oneshot::channel::<Result<Vec<CookieMetadata>, String>>();
    let sender = Arc::new(Mutex::new(Some(sender)));
    let setup_sender = sender.clone();
    probe
        .with_webview(move |platform| {
            let callback_sender = sender.clone();
            let setup = (|| -> Result<(), String> {
                let controller = platform.controller();
                let core = unsafe { controller.CoreWebView2() }
                    .map_err(|error| format!("无法访问 WebView2：{error}"))?;
                let core: ICoreWebView2_2 = core
                    .cast()
                    .map_err(|error| format!("当前 WebView2 不支持 Cookie Manager：{error}"))?;
                let manager = unsafe { core.CookieManager() }
                    .map_err(|error| format!("无法访问 WebView2 Cookie Manager：{error}"))?;
                let handler = GetCookiesCompletedHandler::create(Box::new(
                    move |status, list: Option<ICoreWebView2CookieList>| {
                        let result = status
                            .and_then(|_| {
                                let list =
                                    list.ok_or_else(|| windows::core::Error::from_win32())?;
                                cookie_metadata_from_list(&list).map_err(|error| {
                                    windows::core::Error::new(windows::core::HRESULT(1), error)
                                })
                            })
                            .map_err(|error| format!("读取 WebView2 Cookie 失败：{error}"));
                        send_once(&callback_sender, result);
                        Ok(())
                    },
                ));
                let uri = CoTaskMemPWSTR::from(uri);
                unsafe { manager.GetCookies(*uri.as_ref().as_pcwstr(), &handler) }
                    .map_err(|error| format!("无法开始读取 WebView2 Cookie：{error}"))?;
                Ok(())
            })();
            if let Err(error) = setup {
                send_once(&setup_sender, Err(error));
            }
        })
        .map_err(|error| format!("无法调度 WebView2 Cookie 检测：{error}"))?;

    tokio::time::timeout(std::time::Duration::from_secs(6), receiver)
        .await
        .map_err(|_| "读取 WebView2 Cookie 超时".to_string())?
        .map_err(|_| "WebView2 Cookie 检测意外中断".to_string())?
}

#[cfg(windows)]
fn send_once<T>(
    sender: &std::sync::Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<T>>>>,
    value: T,
) {
    if let Some(sender) = sender.lock().expect("cookie sender poisoned").take() {
        let _ = sender.send(value);
    }
}

#[cfg(windows)]
fn cookie_metadata_from_list(
    list: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2CookieList,
) -> Result<Vec<CookieMetadata>, String> {
    let mut count = 0;
    unsafe { list.Count(&mut count) }.map_err(|error| error.to_string())?;
    let mut cookies = Vec::with_capacity(count as usize);
    for index in 0..count {
        let cookie = unsafe { list.GetValueAtIndex(index) }.map_err(|error| error.to_string())?;
        let mut name = windows::core::PWSTR::null();
        let mut value = windows::core::PWSTR::null();
        let mut expires = 0.0;
        let mut is_session = windows::core::BOOL(0);
        unsafe {
            cookie.Name(&mut name).map_err(|error| error.to_string())?;
            cookie
                .Value(&mut value)
                .map_err(|error| error.to_string())?;
            cookie
                .Expires(&mut expires)
                .map_err(|error| error.to_string())?;
            cookie
                .IsSession(&mut is_session)
                .map_err(|error| error.to_string())?;
        }
        cookies.push(CookieMetadata {
            name: webview2_com::take_pwstr(name),
            expires,
            is_session: is_session.as_bool(),
            has_value: !webview2_com::take_pwstr(value).is_empty(),
        });
    }
    Ok(cookies)
}

#[cfg(not(windows))]
async fn read_cookie_metadata(
    _app: &AppHandle,
    _uri: &'static str,
) -> Result<Vec<CookieMetadata>, String> {
    Err("Cookie 登录检测目前仅支持 Windows WebView2".into())
}

pub fn close_cookie_probe(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(COOKIE_PROBE_LABEL) {
        let _ = window.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cookie(name: &str, expires: f64, is_session: bool, has_value: bool) -> CookieMetadata {
        CookieMetadata {
            name: name.into(),
            expires,
            is_session,
            has_value,
        }
    }

    #[test]
    fn xiaohongshu_requires_the_authenticated_session_cookie() {
        let rule = LOGIN_COOKIE_RULES
            .iter()
            .find(|rule| rule.source_id == "xiaohongshu")
            .unwrap();
        assert_eq!(
            evaluate_login(rule, &[cookie("a1", 0.0, true, true)], 100.0).status,
            SourceLoginStatus::LoggedOut
        );
        assert_eq!(
            evaluate_login(rule, &[cookie("web_session", 0.0, true, true)], 100.0).status,
            SourceLoginStatus::LoggedIn
        );
    }

    #[test]
    fn zhihu_does_not_treat_the_anonymous_device_cookie_as_login() {
        let rule = LOGIN_COOKIE_RULES
            .iter()
            .find(|rule| rule.source_id == "zhihu")
            .unwrap();
        assert_eq!(
            evaluate_login(rule, &[cookie("d_c0", 500.0, false, true)], 100.0).status,
            SourceLoginStatus::LoggedOut
        );
        assert_eq!(
            evaluate_login(rule, &[cookie("z_c0", 500.0, false, true)], 100.0).status,
            SourceLoginStatus::LoggedIn
        );
    }

    #[test]
    fn expired_or_empty_login_cookie_is_rejected() {
        let rule = &LOGIN_COOKIE_RULES[1];
        assert_eq!(
            evaluate_login(rule, &[cookie("z_c0", 50.0, false, true)], 100.0).status,
            SourceLoginStatus::LoggedOut
        );
        assert_eq!(
            evaluate_login(rule, &[cookie("z_c0", 500.0, false, false)], 100.0).status,
            SourceLoginStatus::LoggedOut
        );
    }
}
