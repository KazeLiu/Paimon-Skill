use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatEvent {
    pub kind: String,
    pub run_id: String,
    pub conversation_id: String,
    pub delta: Option<String>,
    pub final_content: Option<String>,
    pub message_id: Option<String>,
    pub error: Option<String>,
}

impl ChatEvent {
    pub fn delta(run_id: &str, conversation_id: &str, delta: String) -> Self {
        Self {
            kind: "message_delta".into(),
            run_id: run_id.into(),
            conversation_id: conversation_id.into(),
            delta: Some(delta),
            final_content: None,
            message_id: None,
            error: None,
        }
    }

    pub fn finished(
        run_id: &str,
        conversation_id: &str,
        message_id: String,
        final_content: String,
    ) -> Self {
        Self {
            kind: "assistant_finished".into(),
            run_id: run_id.into(),
            conversation_id: conversation_id.into(),
            delta: None,
            final_content: Some(final_content),
            message_id: Some(message_id),
            error: None,
        }
    }

    pub fn failed(run_id: &str, conversation_id: &str, error: String) -> Self {
        Self {
            kind: "run_failed".into(),
            run_id: run_id.into(),
            conversation_id: conversation_id.into(),
            delta: None,
            final_content: None,
            message_id: None,
            error: Some(error),
        }
    }

    pub fn cancelled(run_id: &str, conversation_id: &str) -> Self {
        Self {
            kind: "run_cancelled".into(),
            run_id: run_id.into(),
            conversation_id: conversation_id.into(),
            delta: None,
            final_content: None,
            message_id: None,
            error: None,
        }
    }
}
