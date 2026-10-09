use serde::{Deserialize, Serialize};

use crate::agent::runtime::AssistantSegment;
use crate::attachments::ChatAttachment;
use crate::tools::registry::ToolResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub segments: Vec<AssistantSegment>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunRecord {
    pub id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub status: String,
    pub result: Option<ToolResult>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StoredSourceRecord {
    pub id: String,
    pub conversation_id: String,
    pub run_id: String,
    pub call_id: String,
    pub title: String,
    pub url: String,
    pub source_level: String,
    pub platform: String,
    pub captured_at: String,
    pub created_at: String,
}
