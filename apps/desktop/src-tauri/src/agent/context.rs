use crate::{providers::models::ChatMessage, storage::models::Message};

pub fn build_messages(system_prompt: &str, history: &[Message]) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(history.len() + 1);
    messages.push(ChatMessage::system(system_prompt));
    messages.extend(
        history
            .iter()
            .filter_map(|message| match message.role.as_str() {
                "user" => Some(ChatMessage::user(&message.content)),
                "assistant" => Some(ChatMessage::assistant(&message.content)),
                _ => None,
            }),
    );
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepends_composed_system_prompt_and_preserves_chat_order() {
        let history = vec![
            Message {
                id: "1".into(),
                conversation_id: "c".into(),
                role: "user".into(),
                content: "问题".into(),
                created_at: "1".into(),
            },
            Message {
                id: "2".into(),
                conversation_id: "c".into(),
                role: "assistant".into(),
                content: "回答".into(),
                created_at: "2".into(),
            },
        ];

        let messages = build_messages("custom system", &history);

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "custom system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }
}
