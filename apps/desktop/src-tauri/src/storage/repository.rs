use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use super::models::{Conversation, Message};

pub struct ChatRepository {
    path: PathBuf,
}

impl ChatRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn initialize(&self) -> Result<(), String> {
        let connection = self.connection()?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    provider_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                 );
                 CREATE TABLE IF NOT EXISTS messages (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    role TEXT NOT NULL,
                    content TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_messages_conversation_created
                    ON messages(conversation_id, created_at);",
            )
            .map_err(|error| format!("无法初始化会话数据库：{error}"))
    }

    pub fn create_conversation(
        &self,
        provider_id: &str,
        title: &str,
    ) -> Result<Conversation, String> {
        let connection = self.connection()?;
        let now = Utc::now().to_rfc3339();
        let conversation = Conversation {
            id: Uuid::new_v4().to_string(),
            title: title.trim().to_owned(),
            provider_id: provider_id.to_owned(),
            created_at: now.clone(),
            updated_at: now,
        };
        connection
            .execute(
                "INSERT INTO conversations (id, title, provider_id, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    conversation.id,
                    conversation.title,
                    conversation.provider_id,
                    conversation.created_at,
                    conversation.updated_at
                ],
            )
            .map_err(|error| format!("无法创建对话：{error}"))?;
        Ok(conversation)
    }

    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, title, provider_id, created_at, updated_at
                 FROM conversations ORDER BY updated_at DESC, created_at DESC",
            )
            .map_err(|error| format!("无法读取对话列表：{error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    provider_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|error| format!("无法查询对话列表：{error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析对话列表：{error}"))
    }

    pub fn add_message(
        &self,
        conversation_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Message, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始消息事务：{error}"))?;
        let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
        let message = Message {
            id: Uuid::new_v4().to_string(),
            conversation_id: conversation_id.to_owned(),
            role: role.to_owned(),
            content: content.to_owned(),
            created_at: now.clone(),
        };
        transaction
            .execute(
                "INSERT INTO messages (id, conversation_id, role, content, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    message.id,
                    message.conversation_id,
                    message.role,
                    message.content,
                    message.created_at
                ],
            )
            .map_err(|error| format!("无法保存消息：{error}"))?;
        transaction
            .execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![now, conversation_id],
            )
            .map_err(|error| format!("无法更新对话时间：{error}"))?;
        transaction
            .commit()
            .map_err(|error| format!("无法提交消息：{error}"))?;
        Ok(message)
    }

    pub fn list_messages(&self, conversation_id: &str) -> Result<Vec<Message>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, conversation_id, role, content, created_at
                 FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|error| format!("无法读取消息：{error}"))?;
        let rows = statement
            .query_map([conversation_id], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })
            .map_err(|error| format!("无法查询消息：{error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析消息：{error}"))
    }

    pub fn rename_conversation(&self, id: &str, title: &str) -> Result<(), String> {
        let connection = self.connection()?;
        connection
            .execute(
                "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                params![title.trim(), Utc::now().to_rfc3339(), id],
            )
            .map_err(|error| format!("无法重命名对话：{error}"))?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), String> {
        let connection = self.connection()?;
        connection
            .execute("DELETE FROM conversations WHERE id = ?1", [id])
            .map_err(|error| format!("无法删除对话：{error}"))?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection, String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建数据库目录：{error}"))?;
        }
        let connection =
            Connection::open(&self.path).map_err(|error| format!("无法打开会话数据库：{error}"))?;
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("无法启用数据库外键：{error}"))?;
        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_conversations_and_messages_in_display_order() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ChatRepository::new(dir.path().join("paimon.db"));
        repository.initialize().unwrap();

        let conversation = repository
            .create_conversation("deepseek", "芙宁娜配装")
            .unwrap();
        repository
            .add_message(&conversation.id, "user", "怎么配装？")
            .unwrap();
        repository
            .add_message(&conversation.id, "assistant", "先说结论")
            .unwrap();

        let conversations = repository.list_conversations().unwrap();
        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].title, "芙宁娜配装");

        let messages = repository.list_messages(&conversation.id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].content, "先说结论");
    }

    #[test]
    fn renames_and_cascade_deletes_a_conversation() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ChatRepository::new(dir.path().join("paimon.db"));
        repository.initialize().unwrap();
        let conversation = repository.create_conversation("local", "新对话").unwrap();
        repository
            .add_message(&conversation.id, "user", "hello")
            .unwrap();

        repository
            .rename_conversation(&conversation.id, "本地模型测试")
            .unwrap();
        assert_eq!(
            repository.list_conversations().unwrap()[0].title,
            "本地模型测试"
        );

        repository.delete_conversation(&conversation.id).unwrap();
        assert!(repository.list_conversations().unwrap().is_empty());
        assert!(repository
            .list_messages(&conversation.id)
            .unwrap()
            .is_empty());
    }
}
