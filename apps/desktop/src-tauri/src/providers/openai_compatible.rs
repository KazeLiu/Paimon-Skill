use futures_util::StreamExt;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

use crate::diagnostics::redact_sensitive_text;

use super::models::{
    CapabilityDiagnostic, CapabilityReport, CapabilityStatus, ChatMessage, ProviderConfig,
    StreamEvent, ToolCall, ToolDefinition,
};

const MAX_DIAGNOSTIC_BODY: usize = 8_192;
const MAX_ERROR_MESSAGE: usize = 1_024;

fn provider_timeout(provider: &ProviderConfig) -> std::time::Duration {
    std::time::Duration::from_millis(provider.timeout_ms)
}
pub const CONNECTION_TEST_PROMPT: &str = "帮我看看今天的原神的卡池是什么";

#[derive(Debug)]
struct DiagnosticError {
    message: String,
    diagnostic: CapabilityDiagnostic,
}

impl From<String> for DiagnosticError {
    fn from(message: String) -> Self {
        let message = bounded_error_message(&message);
        Self {
            message: message.clone(),
            diagnostic: CapabilityDiagnostic {
                stage: "request".into(),
                endpoint: String::new(),
                http_status: None,
                content_type: None,
                response_headers: Vec::new(),
                response_body: String::new(),
                parse_error: Some(message),
                truncated: false,
                request_body: String::new(),
            },
        }
    }
}

fn safe_headers(response: &reqwest::Response) -> Vec<String> {
    response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            let name = name.as_str().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "authorization" | "cookie" | "set-cookie" | "proxy-authorization"
            ) || ["auth", "token", "secret", "api-key", "apikey"]
                .iter()
                .any(|term| name.contains(term))
            {
                return None;
            }
            Some(format!(
                "{}: {}",
                name,
                value.to_str().unwrap_or("<非 UTF-8>")
            ))
            .map(|header| redact_sensitive_text(&header))
        })
        .take(40)
        .collect()
}

fn bounded_body(body: &str) -> (String, bool) {
    let mut chars = body.chars();
    let bounded: String = chars.by_ref().take(MAX_DIAGNOSTIC_BODY).collect();
    let redacted = redact_sensitive_text(&bounded);
    (redacted, chars.next().is_some())
}

fn bounded_error_message(message: &str) -> String {
    let redacted = redact_sensitive_text(message);
    let mut chars = redacted.chars();
    let bounded: String = chars.by_ref().take(MAX_ERROR_MESSAGE).collect();
    if chars.next().is_some() {
        format!("{bounded}…")
    } else {
        bounded
    }
}

fn safe_endpoint(endpoint: &str) -> String {
    redact_sensitive_text(endpoint)
}

fn describe_request_error(error: &reqwest::Error, endpoint: &str) -> String {
    let kind = if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_request() {
        "request"
    } else {
        "transport"
    };
    format!(
        "模型请求未收到 HTTP 响应（kind={kind}, endpoint={}）：{}",
        safe_endpoint(endpoint),
        format_error_chain(error)
    )
}

fn format_error_chain(error: &reqwest::Error) -> String {
    let mut output = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(error) = source {
        output.push_str(" <- ");
        output.push_str(&error.to_string());
        source = error.source();
    }
    bounded_error_message(&output)
}

pub fn diagnostic_for_invalid_json(
    endpoint: &str,
    http_status: u16,
    content_type: Option<&str>,
    response_headers: &[String],
    response_body: &str,
    parse_error: &str,
) -> CapabilityDiagnostic {
    let (response_body, truncated) = bounded_body(response_body);
    CapabilityDiagnostic {
        stage: "decode_json".into(),
        endpoint: safe_endpoint(endpoint),
        http_status: Some(http_status),
        content_type: content_type.map(str::to_owned),
        response_headers: response_headers.to_vec(),
        response_body,
        parse_error: Some(bounded_error_message(parse_error)),
        truncated,
        request_body: String::new(),
    }
}

fn diagnostic_error(
    stage: &str,
    endpoint: &str,
    response: Option<&reqwest::Response>,
    body: &str,
    parse_error: Option<String>,
) -> DiagnosticError {
    let (response_body, truncated) = bounded_body(body);
    let parse_error = parse_error.map(|error| bounded_error_message(&error));
    DiagnosticError {
        message: parse_error.clone().unwrap_or_else(|| "模型请求失败".into()),
        diagnostic: CapabilityDiagnostic {
            stage: stage.into(),
            endpoint: safe_endpoint(endpoint),
            http_status: response.map(|value| value.status().as_u16()),
            content_type: response
                .and_then(|value| value.headers().get(CONTENT_TYPE))
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned),
            response_headers: response.map(safe_headers).unwrap_or_default(),
            response_body,
            parse_error,
            truncated,
            request_body: String::new(),
        },
    }
}

fn diagnostic_error_from_parts(
    stage: &str,
    endpoint: &str,
    http_status: Option<u16>,
    content_type: Option<String>,
    response_headers: Vec<String>,
    body: &str,
    parse_error: Option<String>,
) -> DiagnosticError {
    let (response_body, truncated) = bounded_body(body);
    let parse_error = parse_error.map(|error| bounded_error_message(&error));
    DiagnosticError {
        message: parse_error.clone().unwrap_or_else(|| "模型请求失败".into()),
        diagnostic: CapabilityDiagnostic {
            stage: stage.into(),
            endpoint: safe_endpoint(endpoint),
            http_status,
            content_type,
            response_headers,
            response_body,
            parse_error,
            truncated,
            request_body: String::new(),
        },
    }
}

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
    diagnostics: Arc<Mutex<Vec<CapabilityDiagnostic>>>,
}

impl OpenAiCompatibleClient {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .user_agent("PaimonDesktop/0.1")
            .build()
            .map_err(|error| format!("无法创建模型 HTTP 客户端：{error}"))?;
        Ok(Self {
            http,
            diagnostics: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn take_diagnostics(&self) -> Vec<CapabilityDiagnostic> {
        self.diagnostics
            .lock()
            .map(|mut diagnostics| std::mem::take(&mut *diagnostics))
            .unwrap_or_default()
    }

    fn record_diagnostic(&self, diagnostic: CapabilityDiagnostic) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.push(diagnostic);
        }
    }

    fn attach_request_body(error: &mut DiagnosticError, body: &Value) {
        let sanitized = crate::diagnostics::sanitize_json(body.clone());
        let serialized = serde_json::to_string(&sanitized).unwrap_or_else(|_| "{}".into());
        error.diagnostic.request_body = bounded_body(&serialized).0;
    }

    fn finish_diagnostic_error(
        &self,
        mut error: DiagnosticError,
        request_body: Value,
    ) -> DiagnosticError {
        Self::attach_request_body(&mut error, &request_body);
        error
    }

    pub async fn test_capabilities(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
    ) -> CapabilityReport {
        let basic_request = match self
            .complete_with_diagnostic(
                provider,
                api_key,
                &[ChatMessage::user(CONNECTION_TEST_PROMPT)],
                &[],
            )
            .await
        {
            Ok(_) => CapabilityStatus::ok("基础请求成功"),
            Err(error) => {
                let error = self.finish_diagnostic_error(
                    error,
                    request_body_for_provider(
                        provider,
                        &[ChatMessage::user(CONNECTION_TEST_PROMPT)],
                        &[],
                        false,
                    ),
                );
                CapabilityStatus::failed_with(error.message, error.diagnostic)
            }
        };

        CapabilityReport { basic_request }
    }

    pub async fn list_models(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
    ) -> Result<Vec<String>, String> {
        let url = super::responses::models_url(&provider.base_url)?;
        let response = self
            .http
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header("x-api-key", api_key)
            .header(ACCEPT, "application/json")
            .timeout(provider_timeout(provider))
            .send()
            .await
            .map_err(|error| format!("获取模型列表失败：{error}"))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| format!("读取模型列表失败：{error}"))?;
        if !status.is_success() {
            return Err(format!(
                "获取模型列表返回 HTTP {}：{}",
                status.as_u16(),
                error_message_from_body(&body)
            ));
        }
        let value: Value = serde_json::from_str(&body)
            .map_err(|error| format!("模型列表不是有效 JSON：{error}"))?;
        super::responses::parse_models(&value)
    }

    pub async fn complete(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<CompletionResult, String> {
        match self
            .complete_with_diagnostic(provider, api_key, messages, tools)
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                let error = self.finish_diagnostic_error(
                    error,
                    request_body_for_provider(provider, messages, tools, false),
                );
                self.record_diagnostic(error.diagnostic);
                Err(error.message)
            }
        }
    }

    async fn complete_with_diagnostic(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<CompletionResult, DiagnosticError> {
        let url = provider_url(provider).map_err(DiagnosticError::from)?;
        let body = request_body_for_provider(provider, messages, tools, false);
        let response = self
            .http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header("x-api-key", api_key)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .timeout(provider_timeout(provider))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                diagnostic_error(
                    "request",
                    &url,
                    None,
                    "",
                    Some(describe_request_error(&error, &url)),
                )
            })?;
        let status = response.status();
        let headers = safe_headers(&response);
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let endpoint = url.clone();
        let body_text = response.text().await.map_err(|error| {
            diagnostic_error(
                "read_body",
                &endpoint,
                None,
                "",
                Some(format!("读取模型响应失败：{error}")),
            )
        })?;
        if !status.is_success() {
            return Err(diagnostic_error_from_parts(
                "http_status",
                &endpoint,
                Some(status.as_u16()),
                content_type,
                headers,
                &body_text,
                Some(format!(
                    "模型返回 HTTP {}：{}",
                    status.as_u16(),
                    error_message_from_body(&body_text)
                )),
            ));
        }
        let value: Value = serde_json::from_str(&body_text).map_err(|error| DiagnosticError {
            message: format!("模型响应不是有效 JSON：{error}"),
            diagnostic: diagnostic_for_invalid_json(
                &endpoint,
                status.as_u16(),
                content_type.as_deref(),
                &headers,
                &body_text,
                &error.to_string(),
            ),
        })?;
        let response = super::responses::parse_response(&value).map_err(|error| {
            diagnostic_error_from_parts(
                "validate_schema",
                &endpoint,
                Some(status.as_u16()),
                content_type.clone(),
                headers.clone(),
                &body_text,
                Some(error),
            )
        })?;
        Ok(CompletionResult {
            content: response.content,
            tool_calls: response.tool_calls,
        })
    }

    pub async fn stream(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sender: &UnboundedSender<StreamEvent>,
    ) -> Result<(), String> {
        match self
            .stream_with_diagnostic(provider, api_key, messages, tools, sender)
            .await
        {
            Ok(()) => Ok(()),
            Err(error) => {
                let error = self.finish_diagnostic_error(
                    error,
                    request_body_for_provider(provider, messages, tools, true),
                );
                self.record_diagnostic(error.diagnostic);
                Err(error.message)
            }
        }
    }

    async fn stream_with_diagnostic(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sender: &UnboundedSender<StreamEvent>,
    ) -> Result<(), DiagnosticError> {
        let url = provider_url(provider).map_err(DiagnosticError::from)?;
        let body = request_body_for_provider(provider, messages, tools, true);
        let response = self
            .http
            .post(&url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header("x-api-key", api_key)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .timeout(provider_timeout(provider))
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                diagnostic_error(
                    "request",
                    &url,
                    None,
                    "",
                    Some(describe_request_error(&error, &url)),
                )
            })?;
        let status = response.status();
        let status_code = status.as_u16();
        let headers = safe_headers(&response);
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(diagnostic_error_from_parts(
                "http_status",
                &url,
                Some(status.as_u16()),
                content_type,
                headers,
                &body,
                Some(format!("模型返回 HTTP {}：{}", status.as_u16(), body)),
            ));
        }

        let mut bytes = response.bytes_stream();
        let mut pending = String::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|error| {
                diagnostic_error_from_parts(
                    "read_stream",
                    &url,
                    Some(status_code),
                    content_type.clone(),
                    headers.clone(),
                    &pending,
                    Some(format!("读取模型流失败：{error}")),
                )
            })?;
            pending.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = pending.find('\n') {
                let line = pending[..index].trim_end_matches('\r').to_owned();
                pending.drain(..=index);
                let events = parse_provider_sse_line(provider, &line).map_err(|error| {
                    diagnostic_error_from_parts(
                        "parse_sse",
                        &url,
                        Some(status_code),
                        content_type.clone(),
                        headers.clone(),
                        &line,
                        Some(error),
                    )
                })?;
                for event in events {
                    let done = matches!(event, StreamEvent::Done);
                    sender
                        .send(event)
                        .map_err(|_| DiagnosticError::from("流式响应接收方已关闭".to_string()))?;
                    if done {
                        return Ok(());
                    }
                }
            }
        }
        if !pending.trim().is_empty() {
            let events = parse_provider_sse_line(provider, pending.trim()).map_err(|error| {
                diagnostic_error_from_parts(
                    "parse_sse",
                    &url,
                    Some(status_code),
                    content_type,
                    headers,
                    pending.trim(),
                    Some(error),
                )
            })?;
            for event in events {
                sender
                    .send(event)
                    .map_err(|_| DiagnosticError::from("流式响应接收方已关闭".to_string()))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

fn error_message(value: &Value) -> String {
    value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .unwrap_or("未提供错误详情")
        .to_owned()
}

fn error_message_from_body(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .map(|value| error_message(&value))
        .filter(|message| message != "未提供错误详情")
        .or_else(|| {
            let message = body.trim();
            (!message.is_empty()).then(|| bounded_error_message(message))
        })
        .unwrap_or_else(|| "未提供错误详情".into())
}

fn provider_url(provider: &ProviderConfig) -> Result<String, String> {
    super::responses::responses_url(&provider.base_url)
}

fn request_body_for_provider(
    provider: &ProviderConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    stream: bool,
) -> Value {
    super::responses::request_body(
        &provider.model,
        messages,
        if provider.tool_calling { tools } else { &[] },
        stream,
    )
}

fn parse_provider_sse_line(
    _provider: &ProviderConfig,
    line: &str,
) -> Result<Vec<StreamEvent>, String> {
    super::responses::parse_sse_line(line)
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    use serde_json::Value;

    use super::*;

    fn provider(base_url: String) -> ProviderConfig {
        ProviderConfig {
            id: "relay".into(),
            name: "Relay".into(),
            base_url,
            model: "test-model".into(),
            secret_ref: "provider:relay".into(),
            timeout_ms: 2_000,
            streaming: true,
            tool_calling: true,
            enabled: true,
            is_default: false,
        }
    }

    fn read_request(mut stream: &std::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        let expected_length = loop {
            let count = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let content_length = text[..header_end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                break header_end + 4 + content_length;
            }
        };
        while request.len() < expected_length {
            let count = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..count]);
        }
        String::from_utf8(request).unwrap()
    }

    #[tokio::test]
    async fn connection_test_sends_one_responses_request_with_overridden_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let request = read_request(&stream);
            let response_body =
                r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .unwrap();
            request
        });

        let report = OpenAiCompatibleClient::new()
            .unwrap()
            .test_capabilities(&provider(format!("http://{address}/v1")), "test-secret")
            .await;

        assert!(report.basic_request.supported);
        let request = server.join().unwrap();
        let (head, body) = request.split_once("\r\n\r\n").unwrap();
        let head = head.to_ascii_lowercase();
        assert!(head.starts_with("post /v1/responses http/1.1"));
        assert!(head.contains("authorization: bearer test-secret"));
        assert!(head.contains("x-api-key: test-secret"));
        assert!(head.contains("accept: application/json"));

        let body: Value = serde_json::from_str(body).unwrap();
        assert_eq!(body["input"][0]["content"], CONNECTION_TEST_PROMPT);
        assert!(body.get("temperature").is_none());
        assert!(body.get("max_output_tokens").is_none());
    }

    #[tokio::test]
    async fn streaming_http_errors_are_redacted_and_bounded() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_request(&stream);
            let response_body = format!(
                "{{\"error\":{{\"message\":\"Bearer upstream-secret {}\"}}}}",
                "x".repeat(MAX_ERROR_MESSAGE * 2)
            );
            write!(
                stream,
                "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            )
            .unwrap();
        });
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();

        let error = OpenAiCompatibleClient::new()
            .unwrap()
            .stream_with_diagnostic(
                &provider(format!("http://{address}/v1")),
                "client-secret",
                &[ChatMessage::user("test")],
                &[],
                &sender,
            )
            .await
            .unwrap_err();

        server.join().unwrap();
        assert!(!error.message.contains("upstream-secret"));
        assert!(error.message.contains("[REDACTED]"));
        assert!(error.message.chars().count() <= MAX_ERROR_MESSAGE + 1);
    }

    #[tokio::test]
    async fn connection_failures_record_a_classified_error_chain_and_request_body() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        let client = OpenAiCompatibleClient::new().unwrap();
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let sensitive_tool = ToolDefinition {
            r#type: "function".into(),
            function: super::super::models::ToolFunction {
                name: "read_url".into(),
                description: "Read a page".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": { "apiKey": { "type": "string", "default": "client-secret" } }
                }),
            },
        };
        let error = client
            .stream(
                &provider(format!("http://{address}/v1")),
                "client-secret",
                &[ChatMessage::user("request content")],
                &[sensitive_tool],
                &sender,
            )
            .await
            .unwrap_err();

        assert!(error.contains("未收到 HTTP 响应"));
        assert!(
            error.contains("kind=connect") || error.contains("kind=timeout"),
            "unexpected request error: {error}"
        );
        assert!(!error.contains("client-secret"));

        let diagnostic = client.take_diagnostics().pop().unwrap();
        assert_eq!(diagnostic.stage, "request");
        assert_eq!(diagnostic.http_status, None);
        let parse_error = diagnostic.parse_error.unwrap();
        assert!(parse_error.contains("kind=connect") || parse_error.contains("kind=timeout"));
        assert!(diagnostic.request_body.contains("request content"));
        assert!(!diagnostic.request_body.contains("client-secret"));
    }
}
