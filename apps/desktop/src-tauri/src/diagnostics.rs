use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;
use uuid::Uuid;

const MAX_ENTRIES: usize = 200;
const MAX_TEXT: usize = 8_192;
const MAX_VERBOSE_TEXT: usize = 65_536;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSettings {
    pub verbose: bool,
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self { verbose: false }
    }
}

pub struct DiagnosticsSettingsRepository {
    path: PathBuf,
}

impl DiagnosticsSettingsRepository {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn load(&self) -> Result<DiagnosticsSettings, String> {
        if !self.path.exists() {
            return Ok(DiagnosticsSettings::default());
        }
        let bytes =
            std::fs::read(&self.path).map_err(|error| format!("无法读取诊断设置：{error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("诊断设置格式无效：{error}"))
    }

    pub fn save(&self, settings: &DiagnosticsSettings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建诊断设置目录：{error}"))?;
        }
        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(settings)
            .map_err(|error| format!("无法序列化诊断设置：{error}"))?;
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("无法写入临时诊断设置：{error}"))?;
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| format!("无法替换诊断设置：{error}"))?;
        }
        std::fs::rename(&temporary, &self.path)
            .map_err(|error| format!("无法保存诊断设置：{error}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: String,
    pub created_at: String,
    pub level: String,
    pub category: String,
    pub message: String,
    pub details: Option<Value>,
}

pub struct LogRepository {
    path: PathBuf,
    verbose: bool,
}

impl LogRepository {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            verbose: false,
        }
    }

    pub fn with_verbose(path: impl Into<PathBuf>, verbose: bool) -> Self {
        Self {
            path: path.into(),
            verbose,
        }
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    pub fn list(&self) -> Result<Vec<LogEntry>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes = std::fs::read(&self.path).map_err(|error| format!("无法读取日志：{error}"))?;
        serde_json::from_slice(&bytes).map_err(|error| format!("日志文件格式无效：{error}"))
    }

    pub fn append(
        &self,
        level: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        details: Option<Value>,
    ) -> Result<LogEntry, String> {
        self.append_with_mode(level, category, message, details, self.verbose)
    }

    pub fn append_with_mode(
        &self,
        level: impl Into<String>,
        category: impl Into<String>,
        message: impl Into<String>,
        details: Option<Value>,
        verbose: bool,
    ) -> Result<LogEntry, String> {
        let mut entries = self.list()?;
        let limit = if verbose { MAX_VERBOSE_TEXT } else { MAX_TEXT };
        let entry = LogEntry {
            id: Uuid::new_v4().to_string(),
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level: level.into(),
            category: category.into(),
            message: truncate_to(&redact_text(&message.into()), limit),
            details: details.map(|value| sanitize_json_with_limit(value, limit)),
        };
        entries.push(entry.clone());
        if entries.len() > MAX_ENTRIES {
            entries.drain(0..entries.len() - MAX_ENTRIES);
        }
        self.save(&entries)?;
        Ok(entry)
    }

    pub fn clear(&self) -> Result<(), String> {
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|error| format!("无法清空日志：{error}"))?;
        }
        Ok(())
    }

    fn save(&self, entries: &[LogEntry]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建日志目录：{error}"))?;
        }
        let bytes = serde_json::to_vec_pretty(entries)
            .map_err(|error| format!("无法序列化日志：{error}"))?;
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, bytes).map_err(|error| format!("无法写入临时日志：{error}"))?;
        if self.path.exists() {
            std::fs::remove_file(&self.path).map_err(|error| format!("无法替换日志：{error}"))?;
        }
        std::fs::rename(&temporary, &self.path).map_err(|error| format!("无法保存日志：{error}"))
    }
}

pub fn export_markdown(entries: &[LogEntry]) -> String {
    let mut output = String::from("# 派蒙助手诊断日志\n\n");
    output.push_str(
        "> 以下内容已脱敏，不包含 API Key、Authorization、Cookie、签名参数或本地凭据。摘要日志单项最多 8KB；详细日志模式单项最多 64KB，可能包含提问和网页正文。\n\n",
    );
    output.push_str(&format!(
        "- 导出时间：{}\n- 应用版本：{}\n- 操作系统：{} / {}\n- 日志数量：{}\n\n",
        Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        entries.len()
    ));
    if entries.is_empty() {
        output.push_str("暂无日志。\n");
        return output;
    }
    for entry in entries {
        output.push_str(&format!(
            "## {} [{}] {}\n\n- 时间：{}\n- 分类：{}\n- 消息：{}\n",
            entry.level, entry.category, entry.id, entry.created_at, entry.category, entry.message
        ));
        if let Some(details) = &entry.details {
            output.push_str("\n```json\n");
            output.push_str(&serde_json::to_string_pretty(details).unwrap_or_else(|_| "{}".into()));
            output.push_str("\n```\n");
        }
        output.push('\n');
    }
    output
}

pub(crate) fn sanitize_json(value: Value) -> Value {
    sanitize_json_with_limit(value, MAX_TEXT)
}

fn sanitize_json_with_limit(value: Value, limit: usize) -> Value {
    match value {
        Value::Object(mut object) => {
            for (key, value) in object.iter_mut() {
                let lower = key.to_ascii_lowercase();
                if lower.contains("authorization")
                    || lower.contains("api_key")
                    || lower.contains("apikey")
                    || lower.contains("token")
                    || lower.contains("secret")
                    || lower.contains("cookie")
                    || lower.contains("password")
                    || lower.contains("credential")
                {
                    *value = Value::String("[REDACTED]".into());
                } else {
                    *value = sanitize_json_with_limit(value.take(), limit);
                }
            }
            Value::Object(object)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(|value| sanitize_json_with_limit(value, limit))
                .collect(),
        ),
        Value::String(value) => Value::String(truncate_to(&redact_text(&value), limit)),
        other => other,
    }
}

pub fn redact_sensitive_text(text: &str) -> String {
    let mut output = text.to_owned();
    for prefix in [
        "Bearer ",
        "bearer ",
        "sk-",
        "x-api-key:",
        "x-api-key=",
        "api-key:",
        "api-key=",
        "api_key:",
        "api_key=",
        "token:",
        "token=",
        "secret:",
        "secret=",
        "key:",
        "key=",
        "authorization:",
        "Authorization:",
        "cookie:",
        "Cookie:",
        "set-cookie:",
        "Set-Cookie:",
        "proxy-authorization:",
        "Proxy-Authorization:",
    ] {
        let mut cursor = 0;
        while let Some(relative_start) = output[cursor..].find(prefix) {
            let value_start = cursor + relative_start + prefix.len();
            let secret_start = value_start
                + output[value_start..]
                    .chars()
                    .take_while(|character| character.is_whitespace())
                    .map(char::len_utf8)
                    .sum::<usize>();
            let secret_end = output[secret_start..]
                .find(|character: char| {
                    character.is_whitespace() || matches!(character, '"' | '\'' | ',' | '}')
                })
                .map(|offset| secret_start + offset)
                .unwrap_or(output.len());
            output.replace_range(secret_start..secret_end, "[REDACTED]");
            cursor = secret_start + "[REDACTED]".len();
        }
    }
    redact_local_paths(&redact_urls(&output))
}

/// Build the only diagnostic payload shape allowed for online ASR failures.
///
/// Callers pass an arbitrary upstream/config object, but this function keeps a
/// strict scalar allow-list. Audio bytes/base64, local paths, source URLs,
/// authorization headers, cookies and credentials are discarded rather than
/// attempting best-effort redaction.
#[cfg_attr(not(test), allow(dead_code))]
pub fn sanitize_asr_diagnostic(details: Value) -> Value {
    let Some(object) = details.as_object() else {
        return Value::Object(Default::default());
    };
    let mut safe = serde_json::Map::new();
    for key in [
        "providerId",
        "stage",
        "errorCode",
        "httpStatus",
        "retryable",
        "requestId",
        "elapsedMs",
    ] {
        let Some(value) = object.get(key) else {
            continue;
        };
        let value = match (key, value) {
            ("httpStatus" | "elapsedMs", Value::Number(_)) | ("retryable", Value::Bool(_)) => {
                value.clone()
            }
            ("requestId", Value::String(value)) => Value::String(
                value
                    .chars()
                    .filter(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                    })
                    .take(96)
                    .collect(),
            ),
            (_, Value::String(value)) => {
                Value::String(redact_sensitive_text(value).chars().take(256).collect())
            }
            _ => continue,
        };
        safe.insert(key.to_owned(), value);
    }
    Value::Object(safe)
}

fn redact_text(text: &str) -> String {
    redact_sensitive_text(text)
}

fn redact_urls(text: &str) -> String {
    let mut output = text.to_owned();
    let mut cursor = 0;
    while cursor < output.len() {
        let next_http = output[cursor..].find("http://");
        let next_https = output[cursor..].find("https://");
        let (relative_start, scheme_len) = match (next_http, next_https) {
            (Some(http), Some(https)) if http <= https => (http, 7),
            (Some(http), None) => (http, 7),
            (_, Some(https)) => (https, 8),
            (None, None) => break,
        };
        let start = cursor + relative_start;
        let end = output[start..]
            .find(|character: char| {
                character.is_whitespace() || matches!(character, '"' | '\'' | '<' | '>' | '}')
            })
            .map(|offset| start + offset)
            .unwrap_or(output.len());
        let candidate = output[start..end].to_owned();
        if let Ok(mut url) = Url::parse(&candidate) {
            if url
                .query_pairs()
                .any(|(key, _)| is_sensitive_query_key(&key))
            {
                url.set_query(Some("redacted=[REDACTED]"));
                {
                    let redacted = url.to_string();
                    output.replace_range(start..end, &redacted);
                    cursor = start + redacted.len();
                }
                continue;
            }
        }
        cursor = (start + scheme_len).max(end);
    }
    output
}

fn is_sensitive_query_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "key",
        "token",
        "secret",
        "auth",
        "password",
        "credential",
        "signature",
        "sig",
        "xsec_token",
    ]
    .iter()
    .any(|term| key.contains(term))
}

fn truncate_to(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let value: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{value}…")
    } else {
        value
    }
}

fn redact_local_paths(text: &str) -> String {
    text.split_inclusive(char::is_whitespace)
        .map(|part| {
            if part.contains(":\\") || part.starts_with("\\\\") {
                let suffix = part
                    .chars()
                    .rev()
                    .take_while(|character| character.is_whitespace())
                    .collect::<String>();
                format!(
                    "[LOCAL_PATH_REDACTED]{}",
                    suffix.chars().rev().collect::<String>()
                )
            } else {
                part.to_owned()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn persists_bounded_sanitized_entries() {
        let directory = tempfile::tempdir().unwrap();
        let repository = LogRepository::new(directory.path().join("logs.json"));
        repository
            .append(
                "error",
                "model",
                "request failed with Bearer sk-message-secret",
                Some(json!({"authorization":"Bearer sk-secret", "response":"ok"})),
            )
            .unwrap();
        let entries = repository.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].details.as_ref().unwrap()["authorization"],
            "[REDACTED]"
        );
        assert!(!entries[0].message.contains("message-secret"));
        assert!(!serde_json::to_string(&entries)
            .unwrap()
            .contains("sk-secret"));
    }

    #[test]
    fn redacts_unknown_secrets_from_error_details_and_urls() {
        let directory = tempfile::tempdir().unwrap();
        let repository = LogRepository::new(directory.path().join("logs.json"));
        repository
            .append(
                "error",
                "chat",
                "upstream rejected x-api-key: message-secret and token=query-secret",
                Some(json!({
                    "provider": { "baseUrl": "https://relay.example/v1?api_key=url-secret" },
                    "response": { "api_key": "body-secret", "token": "nested-secret" }
                })),
            )
            .unwrap();

        let serialized = serde_json::to_string(&repository.list().unwrap()).unwrap();
        for secret in [
            "message-secret",
            "query-secret",
            "url-secret",
            "body-secret",
            "nested-secret",
        ] {
            assert!(!serialized.contains(secret), "leaked secret: {secret}");
        }
    }

    #[test]
    fn exports_ai_readable_markdown() {
        let entry = LogEntry {
            id: "log-1".into(),
            created_at: "2026-08-20T10:00:00Z".into(),
            level: "error".into(),
            category: "model".into(),
            message: "HTTP 503".into(),
            details: Some(json!({"httpStatus":503})),
        };
        let markdown = export_markdown(&[entry]);
        assert!(markdown.starts_with("# 派蒙助手诊断日志"));
        assert!(markdown.contains("应用版本"));
        assert!(markdown.contains("HTTP 503"));
        assert!(markdown.contains("```json"));
    }

    #[test]
    fn preserves_useful_http_diagnostic_details_while_redacting_secrets() {
        let directory = tempfile::tempdir().unwrap();
        let repository = LogRepository::new(directory.path().join("logs.json"));
        repository
            .append(
                "error",
                "model_request",
                "流式模型请求失败",
                Some(json!({
                    "diagnostic": {
                        "stage": "http_status",
                        "endpoint": "https://relay.example/v1/responses",
                        "httpStatus": 503,
                        "responseHeaders": ["cf-ray: trace"],
                        "responseBody": "{\\\"error\\\":\\\"unavailable\\\"}",
                        "authorization": "Bearer leak"
                    },
                    "request": {"apiKey": "leak", "toolDefinitionCount": 8}
                })),
            )
            .unwrap();
        let markdown = export_markdown(&repository.list().unwrap());
        assert!(markdown.contains("http_status"));
        assert!(markdown.contains("relay.example"));
        assert!(markdown.contains("cf-ray: trace"));
        assert!(markdown.contains("toolDefinitionCount"));
        assert!(!markdown.contains("leak"));
    }

    #[test]
    fn asr_diagnostics_drop_audio_urls_paths_and_credentials() {
        let safe = sanitize_asr_diagnostic(json!({
            "providerId": "tencent",
            "stage": "submit",
            "errorCode": "AuthFailure",
            "httpStatus": 401,
            "retryable": false,
            "requestId": "req/../../secret?token=x",
            "audioBase64": "UklGR-secret-audio",
            "audioUrl": "https://audio.example/file.mp3?signature=url-secret",
            "localPath": "C:\\private\\voice.mp3",
            "authorization": "Bearer auth-secret",
            "cookie": "session=cookie-secret",
            "secretId": "id-secret",
            "secretKey": "key-secret"
        }));
        let serialized = serde_json::to_string(&safe).unwrap();
        assert!(serialized.contains("AuthFailure"));
        assert!(serialized.contains("reqsecrettokenx"));
        for secret in [
            "secret-audio",
            "url-secret",
            "private",
            "auth-secret",
            "cookie-secret",
            "id-secret",
            "key-secret",
        ] {
            assert!(!serialized.contains(secret), "leaked ASR data: {secret}");
        }
    }

    #[test]
    fn detailed_mode_preserves_more_sanitized_payload() {
        let directory = tempfile::tempdir().unwrap();
        let repository = LogRepository::with_verbose(directory.path().join("logs.json"), true);
        let payload = "公开正文 ".repeat(3_000);
        repository
            .append(
                "info",
                "tool",
                "详细工具调用",
                Some(json!({"content": payload})),
            )
            .unwrap();
        let value = repository.list().unwrap();
        let content = value[0].details.as_ref().unwrap()["content"]
            .as_str()
            .unwrap();
        assert!(content.chars().count() > MAX_TEXT);
        assert!(content.chars().count() <= MAX_VERBOSE_TEXT + 1);
    }

    #[test]
    fn diagnostics_settings_round_trip() {
        let directory = tempfile::tempdir().unwrap();
        let repository = DiagnosticsSettingsRepository::new(directory.path().join("settings.json"));
        assert_eq!(repository.load().unwrap(), DiagnosticsSettings::default());
        let settings = DiagnosticsSettings { verbose: true };
        repository.save(&settings).unwrap();
        assert_eq!(repository.load().unwrap(), settings);
    }

    #[test]
    fn redacts_windows_local_paths() {
        let directory = tempfile::tempdir().unwrap();
        let repository = LogRepository::new(directory.path().join("logs.json"));
        repository
            .append(
                "info",
                "tool",
                r#"opened C:\Users\private\secret.json"#,
                None,
            )
            .unwrap();
        let message = repository.list().unwrap()[0].message.clone();
        assert!(!message.contains("private"));
        assert!(message.contains("LOCAL_PATH_REDACTED"));
    }
}
