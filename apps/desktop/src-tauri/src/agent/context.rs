use crate::{providers::models::ChatMessage, storage::models::Message};

/// Maximum content retained for one message, including the system message.
pub const MAX_MESSAGE_CHARS: usize = 12_000;
/// Maximum content retained across the system prompt and historical messages.
pub const MAX_CONTEXT_CHARS: usize = 48_000;
/// Maximum raw attachment bytes retained across historical messages.
pub const MAX_CONTEXT_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;
const TRUNCATION_SUFFIX: &str = "\n[该消息已按上下文上限截断]";

pub fn build_messages(system_prompt: &str, history: &[Message]) -> Vec<ChatMessage> {
    build_messages_with_attachment_budget(system_prompt, history, MAX_CONTEXT_ATTACHMENT_BYTES)
}

fn build_messages_with_attachment_budget(
    system_prompt: &str,
    history: &[Message],
    attachment_budget: u64,
) -> Vec<ChatMessage> {
    let system_content = bound_message_content(system_prompt);
    let system_chars = system_content.chars().count();
    let system = ChatMessage::system(system_content);
    let history_budget = MAX_CONTEXT_CHARS.saturating_sub(system_chars);
    let mut selected = Vec::new();
    let mut used_chars = 0usize;
    let mut used_attachment_bytes = 0u64;

    // Walk backwards so the newest messages win. We only split a message when
    // the message itself exceeds its hard per-message limit.
    for message in history.iter().rev() {
        let role = match message.role.as_str() {
            "user" | "assistant" => message.role.as_str(),
            _ => continue,
        };
        let content = bound_message_content(&message.content);
        let chars = content.chars().count();
        if used_chars.saturating_add(chars) > history_budget {
            break;
        }
        used_chars += chars;
        selected.push(match role {
            "user" => {
                let attachments = message
                    .attachments
                    .iter()
                    .filter(|attachment| {
                        if crate::attachments::validate_attachments(std::slice::from_ref(
                            attachment,
                        ))
                        .is_err()
                        {
                            return false;
                        }
                        let next = used_attachment_bytes.saturating_add(attachment.size);
                        if next > attachment_budget {
                            return false;
                        }
                        used_attachment_bytes = next;
                        true
                    })
                    .cloned()
                    .collect();
                ChatMessage::user_with_attachments(content, attachments)
            }
            _ => ChatMessage::assistant(content),
        });
    }
    selected.reverse();

    let mut messages = Vec::with_capacity(selected.len() + 1);
    messages.push(system);
    messages.extend(selected);
    messages
}

fn bound_message_content(content: &str) -> String {
    if content.chars().count() <= MAX_MESSAGE_CHARS {
        return content.to_owned();
    }

    let suffix_len = TRUNCATION_SUFFIX.chars().count();
    let keep = MAX_MESSAGE_CHARS.saturating_sub(suffix_len);
    let mut bounded: String = content.chars().take(keep).collect();
    bounded.push_str(TRUNCATION_SUFFIX);
    bounded
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
                attachments: vec![],
                run_id: None,
                segments: vec![],
                created_at: "1".into(),
            },
            Message {
                id: "2".into(),
                conversation_id: "c".into(),
                role: "assistant".into(),
                content: "回答".into(),
                attachments: vec![],
                run_id: None,
                segments: vec![],
                created_at: "2".into(),
            },
        ];

        let messages = build_messages("custom system", &history);

        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "custom system");
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    fn message(id: &str, role: &str, content: &str) -> Message {
        Message {
            id: id.into(),
            conversation_id: "c".into(),
            role: role.into(),
            content: content.into(),
            attachments: vec![],
            run_id: None,
            segments: vec![],
            created_at: id.into(),
        }
    }

    #[test]
    fn drops_old_history_at_message_boundaries_and_keeps_recent_messages() {
        let history = vec![
            message("1", "user", &"旧一".repeat(MAX_MESSAGE_CHARS)),
            message("2", "assistant", &"旧二".repeat(MAX_MESSAGE_CHARS)),
            message("3", "user", &"旧三".repeat(MAX_MESSAGE_CHARS)),
            message("4", "assistant", &"旧四".repeat(MAX_MESSAGE_CHARS)),
            message("5", "user", "最新问题"),
            message("6", "assistant", "最新回答"),
        ];

        let messages = build_messages("system", &history);
        assert_eq!(messages.first().unwrap().content, "system");
        assert_eq!(messages.last().unwrap().content, "最新回答");
        assert_eq!(messages[messages.len() - 2].content, "最新问题");
        assert_eq!(messages.len(), 6);
        assert!(!messages
            .iter()
            .any(|message| message.content.starts_with("旧一")));
        assert!(messages
            .iter()
            .any(|message| message.content.starts_with("旧二")));
        assert!(
            messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum::<usize>()
                <= MAX_CONTEXT_CHARS
        );
    }

    #[test]
    fn truncates_an_oversized_single_message_within_the_per_message_limit() {
        let content = "字".repeat(MAX_MESSAGE_CHARS + 100);
        let messages = build_messages("system", &[message("1", "user", &content)]);
        let bounded = &messages[1].content;

        assert_eq!(bounded.chars().count(), MAX_MESSAGE_CHARS);
        assert!(bounded.ends_with(TRUNCATION_SUFFIX));
    }

    #[test]
    fn system_prompt_is_always_retained_and_total_history_stays_bounded() {
        let system = "系统".repeat(MAX_CONTEXT_CHARS);
        let history = vec![
            message("1", "user", &"a".repeat(MAX_MESSAGE_CHARS)),
            message("2", "assistant", &"b".repeat(MAX_MESSAGE_CHARS)),
            message("3", "user", &"c".repeat(MAX_MESSAGE_CHARS)),
            message("4", "assistant", &"d".repeat(MAX_MESSAGE_CHARS)),
        ];
        let messages = build_messages(&system, &history);

        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.starts_with("系统"));
        assert!(messages[0].content.ends_with(TRUNCATION_SUFFIX));
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[1].content, "b".repeat(MAX_MESSAGE_CHARS));
        assert_eq!(messages[3].content, "d".repeat(MAX_MESSAGE_CHARS));
        assert_eq!(
            messages
                .iter()
                .map(|message| message.content.chars().count())
                .sum::<usize>(),
            MAX_CONTEXT_CHARS
        );
    }

    #[test]
    fn keeps_recent_attachments_within_the_context_attachment_budget() {
        let mut older = message("1", "user", "旧附件");
        older.attachments = vec![crate::attachments::ChatAttachment::image(
            "old.png",
            "image/png",
            "data:image/png;base64,YQ==",
            1,
        )];
        let mut newest = message("2", "user", "新附件");
        newest.attachments = vec![crate::attachments::ChatAttachment::image(
            "new.png",
            "image/png",
            "data:image/png;base64,YQ==",
            1,
        )];

        let messages = build_messages_with_attachment_budget("system", &[older, newest], 1);

        assert!(messages[1].attachments.is_empty());
        assert_eq!(messages[2].attachments[0].name, "new.png");
    }

    #[test]
    fn drops_historical_attachments_with_inconsistent_declared_sizes() {
        let mut corrupt = message("1", "user", "损坏附件");
        corrupt.attachments = vec![crate::attachments::ChatAttachment::file(
            "corrupt.txt",
            "text/plain",
            "data:text/plain;base64,YWJj",
            1,
        )];

        let messages = build_messages("system", &[corrupt]);

        assert!(messages[1].attachments.is_empty());
    }
}
