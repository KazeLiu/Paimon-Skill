use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attachments::ChatAttachment;

pub const DEFAULT_PROVIDER_TIMEOUT_MS: u64 = 180_000;
pub const MAX_PROVIDER_TIMEOUT_MS: u64 = 600_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub model: String,
    pub secret_ref: String,
    pub timeout_ms: u64,
    pub streaming: bool,
    pub tool_calling: bool,
    pub enabled: bool,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self::text("system", content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::text("user", content)
    }

    pub fn user_with_attachments(
        content: impl Into<String>,
        attachments: Vec<ChatAttachment>,
    ) -> Self {
        let mut message = Self::text("user", content);
        message.attachments = attachments;
        message
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::text("assistant", content)
    }

    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            attachments: Vec::new(),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }

    pub fn assistant_with_tool_calls(content: impl Into<String>, calls: &[ToolCall]) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            attachments: Vec::new(),
            name: None,
            tool_call_id: None,
            tool_calls: Some(
                calls
                    .iter()
                    .map(|call| {
                        serde_json::json!({
                            "id": call.id,
                            "type": "function",
                            "function": {"name": call.name, "arguments": call.arguments}
                        })
                    })
                    .collect(),
            ),
        }
    }

    fn text(role: &str, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            attachments: Vec::new(),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    pub r#type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    ContentDelta(String),
    ToolCallDelta {
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: String,
    },
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReport {
    pub basic_request: CapabilityStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityStatus {
    pub supported: bool,
    pub message: String,
    pub diagnostic: Option<CapabilityDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityDiagnostic {
    pub stage: String,
    pub endpoint: String,
    pub http_status: Option<u16>,
    pub content_type: Option<String>,
    pub response_headers: Vec<String>,
    pub response_body: String,
    pub parse_error: Option<String>,
    pub truncated: bool,
    #[serde(default)]
    pub request_body: String,
}

impl CapabilityStatus {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            supported: true,
            message: message.into(),
            diagnostic: None,
        }
    }

    pub fn failed_with(message: impl Into<String>, diagnostic: CapabilityDiagnostic) -> Self {
        Self {
            supported: false,
            message: message.into(),
            diagnostic: Some(diagnostic),
        }
    }
}
