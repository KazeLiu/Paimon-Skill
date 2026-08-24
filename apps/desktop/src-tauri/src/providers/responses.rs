use serde_json::Value;
use url::Url;

use crate::attachments::AttachmentKind;

use super::models::{ChatMessage, StreamEvent, ToolCall, ToolDefinition};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResponseResult {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

pub(crate) fn responses_url(base_url: &str) -> Result<String, String> {
    let mut url = Url::parse(base_url.trim()).map_err(|error| format!("模型地址无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("模型地址必须使用 HTTP 或 HTTPS".into());
    }
    let path = base_path(url.path());
    if path.ends_with("/responses") {
        return Ok(url.to_string().trim_end_matches('/').to_string());
    }
    let next_path = if path.is_empty() {
        "/responses".to_owned()
    } else {
        format!("{path}/responses")
    };
    url.set_path(&next_path);
    Ok(url.to_string())
}

pub(crate) fn models_url(base_url: &str) -> Result<String, String> {
    let mut url = Url::parse(base_url.trim()).map_err(|error| format!("模型地址无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("模型地址必须使用 HTTP 或 HTTPS".into());
    }
    let path = base_path(url.path());
    let next_path = if path.ends_with("/models") {
        path.to_owned()
    } else if path.is_empty() {
        "/models".to_owned()
    } else {
        format!("{path}/models")
    };
    url.set_path(&next_path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn base_path(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    path.strip_suffix("/responses")
        .or_else(|| path.strip_suffix("/models"))
        .or_else(|| path.strip_suffix("/chat/completions"))
        .unwrap_or(path)
}

pub(crate) fn parse_models(value: &Value) -> Result<Vec<String>, String> {
    let data = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.as_array())
        .ok_or_else(|| "模型列表响应缺少 data 数组".to_string())?;
    let mut models = data
        .iter()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    if models.is_empty() {
        return Err("模型列表为空".into());
    }
    Ok(models)
}

pub(crate) fn request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    stream: bool,
) -> Value {
    let input = messages
        .iter()
        .flat_map(message_to_input_items)
        .collect::<Vec<_>>();
    let mut body = serde_json::json!({
        "model": model,
        "input": input,
        "stream": stream,
    });
    if !tools.is_empty() {
        body["tools"] = Value::Array(
            tools
                .iter()
                .map(|tool| {
                    serde_json::json!({
                        "type": "function",
                        "name": tool.function.name,
                        "description": tool.function.description,
                        "parameters": tool.function.parameters,
                    })
                })
                .collect(),
        );
        body["tool_choice"] = Value::String("auto".into());
    }
    body
}

fn message_to_input_items(message: &ChatMessage) -> Vec<Value> {
    if message.role == "tool" {
        return vec![serde_json::json!({
            "type": "function_call_output",
            "call_id": message.tool_call_id,
            "output": message.content,
        })];
    }
    if let Some(calls) = &message.tool_calls {
        return calls
            .iter()
            .map(|call| {
                serde_json::json!({
                    "type": "function_call",
                    "call_id": call.get("id"),
                    "name": call.pointer("/function/name"),
                    "arguments": call.pointer("/function/arguments"),
                })
            })
            .collect();
    }
    let content = if message.role == "user" && !message.attachments.is_empty() {
        let mut content = Vec::with_capacity(message.attachments.len() + 1);
        if !message.content.trim().is_empty() {
            content.push(serde_json::json!({
                "type": "input_text",
                "text": message.content,
            }));
        }
        content.extend(
            message
                .attachments
                .iter()
                .map(|attachment| match attachment.kind {
                    AttachmentKind::Image => serde_json::json!({
                        "type": "input_image",
                        "image_url": attachment.data_url,
                    }),
                    AttachmentKind::File => serde_json::json!({
                        "type": "input_file",
                        "filename": attachment.name,
                        "file_data": attachment.data_url,
                    }),
                }),
        );
        Value::Array(content)
    } else {
        Value::String(message.content.clone())
    };
    vec![serde_json::json!({
        "type": "message",
        "role": message.role,
        "content": content,
    })]
}

pub(crate) fn parse_response(value: &Value) -> Result<ResponseResult, String> {
    let output = value
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| "Responses API 响应缺少 output".to_string())?;
    let mut content = String::new();
    let mut tool_calls = Vec::new();
    for item in output {
        match item.get("type").and_then(Value::as_str) {
            Some("message") => {
                for part in item
                    .get("content")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if part.get("type").and_then(Value::as_str) == Some("output_text") {
                        if let Some(text) = part.get("text").and_then(Value::as_str) {
                            content.push_str(text);
                        }
                    }
                }
            }
            Some("function_call") => tool_calls.push(ToolCall {
                id: item
                    .get("call_id")
                    .and_then(Value::as_str)
                    .or_else(|| item.get("id").and_then(Value::as_str))
                    .ok_or_else(|| "Responses API 工具调用缺少 call_id".to_string())?
                    .to_owned(),
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "Responses API 工具调用缺少 name".to_string())?
                    .to_owned(),
                arguments: item
                    .get("arguments")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            }),
            _ => {}
        }
    }
    Ok(ResponseResult {
        content,
        tool_calls,
    })
}

pub(crate) fn parse_sse_line(line: &str) -> Result<Vec<StreamEvent>, String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
        return Ok(Vec::new());
    }
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(Vec::new());
    };
    if data == "[DONE]" {
        return Ok(vec![StreamEvent::Done]);
    }
    let payload: Value =
        serde_json::from_str(data).map_err(|error| format!("Responses SSE 数据无效：{error}"))?;
    match payload.get("type").and_then(Value::as_str) {
        Some("response.output_text.delta") => Ok(payload
            .get("delta")
            .and_then(Value::as_str)
            .filter(|delta| !delta.is_empty())
            .map(|delta| vec![StreamEvent::ContentDelta(delta.into())])
            .unwrap_or_default()),
        Some("response.output_item.added") => parse_added_item(&payload),
        Some("response.function_call_arguments.delta") => Ok(vec![StreamEvent::ToolCallDelta {
            index: output_index(&payload),
            id: None,
            name: None,
            arguments: payload
                .get("delta")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        }]),
        Some("response.completed") => Ok(vec![StreamEvent::Done]),
        Some("response.failed") => Err(payload
            .pointer("/response/error/message")
            .and_then(Value::as_str)
            .unwrap_or("Responses API 返回失败事件")
            .to_owned()),
        _ => Ok(Vec::new()),
    }
}

fn parse_added_item(payload: &Value) -> Result<Vec<StreamEvent>, String> {
    let item = payload.get("item").cloned().unwrap_or(Value::Null);
    if item.get("type").and_then(Value::as_str) != Some("function_call") {
        return Ok(Vec::new());
    }
    Ok(vec![StreamEvent::ToolCallDelta {
        index: output_index(payload),
        id: item
            .get("call_id")
            .and_then(Value::as_str)
            .or_else(|| item.get("id").and_then(Value::as_str))
            .map(str::to_owned),
        name: item.get("name").and_then(Value::as_str).map(str::to_owned),
        arguments: item
            .get("arguments")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    }])
}

fn output_index(payload: &Value) -> usize {
    payload
        .get("output_index")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn tool() -> ToolDefinition {
        ToolDefinition {
            r#type: "function".into(),
            function: super::super::models::ToolFunction {
                name: "read_url".into(),
                description: "Read a page".into(),
                parameters: json!({"type":"object","properties":{"url":{"type":"string"}}}),
            },
        }
    }

    #[test]
    fn normalizes_responses_endpoint() {
        assert_eq!(
            responses_url("https://relay.example/v1").unwrap(),
            "https://relay.example/v1/responses"
        );
        assert_eq!(
            responses_url("https://relay.example/v1/responses").unwrap(),
            "https://relay.example/v1/responses"
        );
        assert_eq!(
            responses_url("https://relay.example/v1/chat/completions").unwrap(),
            "https://relay.example/v1/responses"
        );
    }

    #[test]
    fn normalizes_models_endpoint_from_base_or_responses_url() {
        assert_eq!(
            models_url("https://relay.example/v1").unwrap(),
            "https://relay.example/v1/models"
        );
        assert_eq!(
            models_url("https://relay.example/v1/responses").unwrap(),
            "https://relay.example/v1/models"
        );
    }

    #[test]
    fn parses_openai_model_list() {
        assert_eq!(
            parse_models(&json!({"data":[{"id":"zeta"},{"id":"alpha"},{"id":"zeta"}]})).unwrap(),
            vec!["alpha", "zeta"]
        );
    }

    #[test]
    fn builds_responses_request_with_input_and_flat_function_tools() {
        let body = request_body(
            "gpt-6.5-luna",
            &[ChatMessage::system("system"), ChatMessage::user("hello")],
            &[tool()],
            false,
        );

        assert_eq!(body["model"], "gpt-6.5-luna");
        assert!(body.get("temperature").is_none());
        assert!(body.get("max_output_tokens").is_none());
        assert!(body.get("store").is_none());
        assert_eq!(body["input"][0]["role"], "system");
        assert_eq!(body["input"][1]["content"], "hello");
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "read_url");
        assert_eq!(body["tool_choice"], "auto");
    }

    #[test]
    fn builds_multimodal_user_content_for_images_and_files() {
        let message = ChatMessage::user_with_attachments(
            "分析附件",
            vec![
                crate::attachments::ChatAttachment::image(
                    "screen.png",
                    "image/png",
                    "data:image/png;base64,YQ==",
                    1,
                ),
                crate::attachments::ChatAttachment::file(
                    "notes.pdf",
                    "application/pdf",
                    "data:application/pdf;base64,YQ==",
                    1,
                ),
            ],
        );

        let body = request_body("vision", &[message], &[], false);
        let content = body["input"][0]["content"].as_array().unwrap();

        assert_eq!(content[0], json!({"type":"input_text","text":"分析附件"}));
        assert_eq!(content[1]["type"], "input_image");
        assert_eq!(content[1]["image_url"], "data:image/png;base64,YQ==");
        assert_eq!(content[2]["type"], "input_file");
        assert_eq!(content[2]["filename"], "notes.pdf");
        assert_eq!(content[2]["file_data"], "data:application/pdf;base64,YQ==");
    }

    #[test]
    fn parses_response_text_and_function_calls() {
        let result = parse_response(&json!({
            "output": [
                {"type":"message","content":[{"type":"output_text","text":"hello"}]},
                {"type":"function_call","call_id":"call_1","name":"read_url","arguments":"{\"url\":\"https://example.com\"}"}
            ]
        })).unwrap();

        assert_eq!(result.content, "hello");
        assert_eq!(result.tool_calls[0].id, "call_1");
        assert_eq!(result.tool_calls[0].name, "read_url");
    }

    #[test]
    fn parses_responses_sse_events() {
        assert_eq!(
            parse_sse_line(r#"data: {"type":"response.output_text.delta","delta":"hi"}"#).unwrap(),
            vec![StreamEvent::ContentDelta("hi".into())]
        );
        assert!(matches!(
            parse_sse_line(r#"data: {"type":"response.completed"}"#)
                .unwrap()
                .as_slice(),
            [StreamEvent::Done]
        ));
    }
}
