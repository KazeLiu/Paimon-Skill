use serde::de::DeserializeOwned;
use std::sync::Mutex;
use tauri::Webview;
use tokio::sync::oneshot;

use super::models::PageSnapshot;

pub fn decode_eval_result<T: DeserializeOwned>(raw: &str) -> Result<T, String> {
    match serde_json::from_str(raw) {
        Ok(value) => Ok(value),
        Err(first_error) => {
            let nested: String = serde_json::from_str(raw)
                .map_err(|_| format!("网页脚本返回了无法解析的数据：{first_error}"))?;
            serde_json::from_str(&nested)
                .map_err(|error| format!("网页脚本返回了无法解析的数据：{error}"))
        }
    }
}

pub async fn eval_json<T: DeserializeOwned + Send + 'static>(
    webview: &Webview,
    script: impl Into<String>,
) -> Result<T, String> {
    let (sender, receiver) = oneshot::channel();
    let sender = Mutex::new(Some(sender));
    webview
        .eval_with_callback(script, move |raw| {
            if let Some(sender) = sender.lock().expect("eval sender poisoned").take() {
                let _ = sender.send(raw);
            }
        })
        .map_err(|error| format!("无法执行网页脚本：{error}"))?;

    let raw = tokio::time::timeout(std::time::Duration::from_secs(12), receiver)
        .await
        .map_err(|_| "等待网页脚本结果超时".to_string())?
        .map_err(|_| "网页脚本结果通道已关闭".to_string())?;
    decode_eval_result(&raw)
}

pub async fn extract_page(webview: &Webview, script: &str) -> Result<PageSnapshot, String> {
    let snapshot: PageSnapshot = eval_json(webview, script).await?;
    Ok(PageSnapshot::bounded(
        snapshot.title,
        snapshot.url,
        snapshot.text,
        snapshot.links,
        snapshot.login_challenge,
        snapshot.challenge_reason,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browser::models::ScrollSample;

    #[test]
    fn decode_eval_result_accepts_webview_object_json() {
        let sample: ScrollSample =
            decode_eval_result(r#"{"scrollY":120,"scrollHeight":900,"viewportHeight":500}"#)
                .unwrap();

        assert_eq!(sample.scroll_y, 120.0);
    }

    #[test]
    fn decode_eval_result_accepts_double_encoded_json() {
        let sample: ScrollSample = decode_eval_result(
            r#""{\"scrollY\":120,\"scrollHeight\":900,\"viewportHeight\":500}""#,
        )
        .unwrap();

        assert_eq!(sample.scroll_height, 900.0);
    }
}
