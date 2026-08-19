use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

use super::models::{
    CapabilityReport, CapabilityStatus, ChatMessage, ProviderConfig, StreamEvent, ToolCall,
    ToolDefinition,
};

#[derive(Clone)]
pub struct OpenAiCompatibleClient {
    http: reqwest::Client,
}

impl OpenAiCompatibleClient {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .user_agent("PaimonDesktop/0.1")
            .build()
            .map_err(|error| format!("无法创建模型 HTTP 客户端：{error}"))?;
        Ok(Self { http })
    }

    pub async fn test_capabilities(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
    ) -> CapabilityReport {
        let basic_request = match self
            .complete(provider, api_key, &[ChatMessage::user("ping")], &[])
            .await
        {
            Ok(_) => CapabilityStatus::ok("基础请求成功"),
            Err(error) => CapabilityStatus::failed(error),
        };

        let streaming = if provider.streaming {
            match self
                .stream(
                    provider,
                    api_key,
                    &[ChatMessage::user("ping")],
                    &[],
                    &tokio::sync::mpsc::unbounded_channel().0,
                )
                .await
            {
                Ok(()) => CapabilityStatus::ok("流式输出成功"),
                Err(error) => CapabilityStatus::failed(error),
            }
        } else {
            CapabilityStatus::failed("配置已关闭流式输出")
        };

        let tool_calling = if provider.tool_calling {
            let tool = ToolDefinition {
                r#type: "function".into(),
                function: super::models::ToolFunction {
                    name: "connection_probe".into(),
                    description: "返回连接测试结果".into(),
                    parameters: json!({"type":"object","properties":{}}),
                },
            };
            match self
                .complete(
                    provider,
                    api_key,
                    &[ChatMessage::user("call the probe")],
                    &[tool],
                )
                .await
            {
                Ok(response) if response.contains_tool_call => CapabilityStatus::ok("工具调用成功"),
                Ok(_) => CapabilityStatus::failed("模型返回了普通文本，未检测到工具调用"),
                Err(error) => CapabilityStatus::failed(error),
            }
        } else {
            CapabilityStatus::failed("配置已关闭工具调用")
        };

        CapabilityReport {
            basic_request,
            streaming,
            tool_calling,
        }
    }

    pub async fn complete(
        &self,
        provider: &ProviderConfig,
        api_key: &str,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<CompletionResult, String> {
        let url = chat_completions_url(&provider.base_url)?;
        let body = request_body(provider, messages, tools);
        let response = self
            .http
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header(CONTENT_TYPE, "application/json")
            .timeout(std::time::Duration::from_millis(provider.timeout_ms))
            .json(&body)
            .send()
            .await
            .map_err(|error| format!("模型请求失败：{error}"))?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .map_err(|error| format!("模型响应不是有效 JSON：{error}"))?;
        if !status.is_success() {
            return Err(format!(
                "模型返回 HTTP {}：{}",
                status.as_u16(),
                error_message(&value)
            ));
        }
        let choice = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .ok_or_else(|| "模型响应缺少 choices".to_string())?;
        let message = choice.get("message").cloned().unwrap_or(Value::Null);
        Ok(CompletionResult {
            content: message
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            contains_tool_call: message
                .get("tool_calls")
                .and_then(Value::as_array)
                .is_some_and(|calls| !calls.is_empty()),
            tool_calls: message
                .get("tool_calls")
                .and_then(Value::as_array)
                .map(|calls| {
                    calls
                        .iter()
                        .filter_map(|call| {
                            Some(ToolCall {
                                id: call.get("id")?.as_str()?.to_owned(),
                                name: call.pointer("/function/name")?.as_str()?.to_owned(),
                                arguments: call
                                    .pointer("/function/arguments")?
                                    .as_str()?
                                    .to_owned(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
            raw: value,
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
        let url = chat_completions_url(&provider.base_url)?;
        let body = request_body(provider, messages, tools);
        let response = self
            .http
            .post(url)
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .header(CONTENT_TYPE, "application/json")
            .timeout(std::time::Duration::from_millis(provider.timeout_ms))
            .json(&body)
            .send()
            .await
            .map_err(|error| format!("模型流式请求失败：{error}"))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("模型返回 HTTP {}：{}", status.as_u16(), body));
        }

        let mut bytes = response.bytes_stream();
        let mut pending = String::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = chunk.map_err(|error| format!("读取模型流失败：{error}"))?;
            pending.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(index) = pending.find('\n') {
                let line = pending[..index].trim_end_matches('\r').to_owned();
                pending.drain(..=index);
                for event in parse_sse_line(&line)? {
                    let done = matches!(event, StreamEvent::Done);
                    sender
                        .send(event)
                        .map_err(|_| "流式响应接收方已关闭".to_string())?;
                    if done {
                        return Ok(());
                    }
                }
            }
        }
        if !pending.trim().is_empty() {
            for event in parse_sse_line(pending.trim())? {
                sender
                    .send(event)
                    .map_err(|_| "流式响应接收方已关闭".to_string())?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CompletionResult {
    pub content: String,
    pub contains_tool_call: bool,
    pub tool_calls: Vec<ToolCall>,
    pub raw: Value,
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

pub fn chat_completions_url(base_url: &str) -> Result<String, String> {
    let mut url = Url::parse(base_url.trim()).map_err(|error| format!("模型地址无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("模型地址必须使用 HTTP 或 HTTPS".into());
    }

    let path = url.path().trim_end_matches('/');
    if path.ends_with("/chat/completions") {
        return Ok(url.to_string().trim_end_matches('/').to_string());
    }

    let next_path = if path.is_empty() {
        "/chat/completions".to_owned()
    } else {
        format!("{path}/chat/completions")
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

pub fn request_body(
    provider: &ProviderConfig,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
) -> Value {
    let mut body = json!({
        "model": provider.model,
        "messages": messages,
        "temperature": provider.temperature,
        "max_tokens": provider.max_tokens,
        "stream": provider.streaming,
    });
    if provider.tool_calling && !tools.is_empty() {
        body["tools"] = serde_json::to_value(tools).expect("tool definitions serialize");
    }
    body
}

pub fn parse_sse_line(line: &str) -> Result<Vec<StreamEvent>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return Ok(Vec::new());
    }
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(Vec::new());
    };
    if data == "[DONE]" {
        return Ok(vec![StreamEvent::Done]);
    }

    let payload: Value =
        serde_json::from_str(data).map_err(|error| format!("SSE 数据无效：{error}"))?;
    let choice = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .ok_or_else(|| "SSE 数据缺少 choices".to_string())?;
    let delta = choice.get("delta").cloned().unwrap_or(Value::Null);

    let mut events = Vec::new();
    if let Some(content) = delta.get("content").and_then(Value::as_str) {
        if !content.is_empty() {
            events.push(StreamEvent::ContentDelta(content.into()));
        }
    }

    if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
        for tool in tool_calls {
            let function = tool.get("function").cloned().unwrap_or(Value::Null);
            events.push(StreamEvent::ToolCallDelta {
                index: tool.get("index").and_then(Value::as_u64).unwrap_or(0) as usize,
                id: tool.get("id").and_then(Value::as_str).map(str::to_owned),
                name: function
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                arguments: function
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            });
        }
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{chat_completions_url, parse_sse_line, request_body};
    use crate::providers::models::{
        ChatMessage, ProviderConfig, StreamEvent, ToolDefinition, ToolFunction,
    };

    fn provider(base_url: &str) -> ProviderConfig {
        ProviderConfig {
            id: "deepseek".into(),
            name: "DeepSeek".into(),
            base_url: base_url.into(),
            model: "deepseek-chat".into(),
            secret_ref: "provider:deepseek".into(),
            timeout_ms: 30_000,
            temperature: 0.3,
            max_tokens: 2048,
            streaming: true,
            tool_calling: true,
            enabled: true,
        }
    }

    #[test]
    fn normalizes_base_url_to_chat_completions_endpoint() {
        assert_eq!(
            chat_completions_url("https://api.deepseek.com").unwrap(),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_completions_url("http://127.0.0.1:11434/v1/").unwrap(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://relay.example/v1/chat/completions").unwrap(),
            "https://relay.example/v1/chat/completions"
        );
    }

    #[test]
    fn rejects_unsupported_endpoint_protocols() {
        let error = chat_completions_url("file:///etc/passwd").unwrap_err();
        assert!(error.contains("HTTP"));
    }

    #[test]
    fn builds_openai_compatible_stream_request() {
        let body = request_body(
            &provider("https://api.deepseek.com"),
            &[ChatMessage::system("system"), ChatMessage::user("hello")],
            &[],
        );

        assert_eq!(body["model"], "deepseek-chat");
        assert_eq!(body["stream"], true);
        assert_eq!(
            body["messages"][0],
            json!({"role":"system","content":"system"})
        );
        assert_eq!(
            body["messages"][1],
            json!({"role":"user","content":"hello"})
        );
    }

    #[test]
    fn omits_tools_when_provider_tool_calling_is_disabled() {
        let mut config = provider("https://api.deepseek.com");
        config.tool_calling = false;
        let tools = vec![ToolDefinition {
            r#type: "function".into(),
            function: ToolFunction {
                name: "read_url".into(),
                description: "read".into(),
                parameters: json!({"type":"object"}),
            },
        }];

        let body = request_body(&config, &[ChatMessage::user("hello")], &tools);

        assert!(body.get("tools").is_none());
    }

    #[test]
    fn parses_content_and_tool_call_sse_deltas() {
        let content = parse_sse_line(
            r#"data: {"choices":[{"delta":{"content":"你好"},"finish_reason":null}]}"#,
        )
        .unwrap();
        assert_eq!(content, vec![StreamEvent::ContentDelta("你好".into())]);

        let tool = parse_sse_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"search_bwiki","arguments":"{\"query\":"}}]},"finish_reason":null}]}"#,
        )
        .unwrap();
        assert_eq!(
            tool,
            vec![StreamEvent::ToolCallDelta {
                index: 0,
                id: Some("call_1".into()),
                name: Some("search_bwiki".into()),
                arguments: "{\"query\":".into(),
            }]
        );
    }

    #[test]
    fn parses_every_tool_call_delta_in_one_sse_event() {
        let events = parse_sse_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"search_bwiki","arguments":"{}"}},{"index":1,"id":"call_b","function":{"name":"read_url","arguments":"{\"url\":"}}]},"finish_reason":null}]}"#,
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            StreamEvent::ToolCallDelta { index: 0, id: Some(id), .. } if id == "call_a"
        ));
        assert!(matches!(
            &events[1],
            StreamEvent::ToolCallDelta { index: 1, id: Some(id), .. } if id == "call_b"
        ));
    }

    #[test]
    fn handles_done_comments_and_malformed_sse() {
        assert!(parse_sse_line(": keep-alive").unwrap().is_empty());
        assert_eq!(
            parse_sse_line("data: [DONE]").unwrap(),
            vec![StreamEvent::Done]
        );
        assert!(parse_sse_line("data: not-json")
            .unwrap_err()
            .contains("SSE"));
    }
}
