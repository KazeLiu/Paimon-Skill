use std::path::PathBuf;

use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::tools::registry::ToolResult;

use super::models::{Conversation, Message, StoredSourceRecord, ToolRunRecord};

pub struct ToolRunInput<'a> {
    pub conversation_id: &'a str,
    pub run_id: &'a str,
    pub call_id: &'a str,
    pub tool_name: &'a str,
    pub arguments: &'a str,
    pub status: &'a str,
    pub result: Option<&'a ToolResult>,
    pub error: Option<&'a str>,
}

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
                    ON messages(conversation_id, created_at);
                 CREATE TABLE IF NOT EXISTS tool_runs (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    call_id TEXT NOT NULL,
                    tool_name TEXT NOT NULL,
                    arguments TEXT NOT NULL,
                    status TEXT NOT NULL,
                    result_json TEXT,
                    error TEXT,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
                 );
                 CREATE INDEX IF NOT EXISTS idx_tool_runs_conversation_created
                    ON tool_runs(conversation_id, created_at);
                 CREATE TABLE IF NOT EXISTS sources (
                    id TEXT PRIMARY KEY,
                    conversation_id TEXT NOT NULL,
                    run_id TEXT NOT NULL,
                    call_id TEXT NOT NULL,
                    title TEXT NOT NULL,
                    url TEXT NOT NULL,
                    source_level TEXT NOT NULL,
                    platform TEXT NOT NULL,
                    captured_at TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE,
                    UNIQUE(conversation_id, url)
                 );
                 CREATE INDEX IF NOT EXISTS idx_sources_conversation_created
                    ON sources(conversation_id, created_at);",
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

    pub fn record_tool_run(&self, input: ToolRunInput<'_>) -> Result<ToolRunRecord, String> {
        let mut connection = self.connection()?;
        let transaction = connection
            .transaction()
            .map_err(|error| format!("无法开始工具记录事务：{error}"))?;
        let result_json = input
            .result
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| format!("无法序列化工具结果：{error}"))?;
        let record = ToolRunRecord {
            id: Uuid::new_v4().to_string(),
            conversation_id: input.conversation_id.to_owned(),
            run_id: input.run_id.to_owned(),
            call_id: input.call_id.to_owned(),
            tool_name: input.tool_name.to_owned(),
            arguments: input.arguments.to_owned(),
            status: input.status.to_owned(),
            result: input.result.cloned(),
            error: input.error.map(str::to_owned),
            created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        };
        transaction
            .execute(
                "INSERT INTO tool_runs
                 (id, conversation_id, run_id, call_id, tool_name, arguments, status, result_json, error, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    record.id,
                    record.conversation_id,
                    record.run_id,
                    record.call_id,
                    record.tool_name,
                    record.arguments,
                    record.status,
                    result_json,
                    record.error,
                    record.created_at,
                ],
            )
            .map_err(|error| format!("无法保存工具记录：{error}"))?;
        if let Some(source) = input.result.and_then(|result| result.source.as_ref()) {
            transaction
                .execute(
                    "INSERT INTO sources
                     (id, conversation_id, run_id, call_id, title, url, source_level, platform, captured_at, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(conversation_id, url) DO UPDATE SET
                       run_id = excluded.run_id,
                       call_id = excluded.call_id,
                       title = excluded.title,
                       source_level = excluded.source_level,
                       platform = excluded.platform,
                       captured_at = excluded.captured_at,
                       created_at = excluded.created_at",
                    params![
                        Uuid::new_v4().to_string(),
                        input.conversation_id,
                        input.run_id,
                        input.call_id,
                        source.title,
                        source.url,
                        source.source_level,
                        source.platform,
                        source.captured_at,
                        record.created_at,
                    ],
                )
                .map_err(|error| format!("无法保存来源记录：{error}"))?;
        }
        transaction
            .commit()
            .map_err(|error| format!("无法提交工具记录：{error}"))?;
        Ok(record)
    }

    pub fn list_tool_runs(&self, conversation_id: &str) -> Result<Vec<ToolRunRecord>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, conversation_id, run_id, call_id, tool_name, arguments, status,
                        result_json, error, created_at
                 FROM tool_runs WHERE conversation_id = ?1 ORDER BY created_at ASC, rowid ASC",
            )
            .map_err(|error| format!("无法读取工具记录：{error}"))?;
        let rows = statement
            .query_map([conversation_id], |row| {
                let result_json: Option<String> = row.get(7)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    result_json,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })
            .map_err(|error| format!("无法查询工具记录：{error}"))?;
        rows.map(|row| {
            let (
                id,
                conversation_id,
                run_id,
                call_id,
                tool_name,
                arguments,
                status,
                result_json,
                error,
                created_at,
            ) = row.map_err(|error| format!("无法解析工具记录：{error}"))?;
            let result = result_json
                .map(|json| serde_json::from_str(&json))
                .transpose()
                .map_err(|error| format!("工具结果数据已损坏：{error}"))?;
            Ok(ToolRunRecord {
                id,
                conversation_id,
                run_id,
                call_id,
                tool_name,
                arguments,
                status,
                result,
                error,
                created_at,
            })
        })
        .collect()
    }

    pub fn list_sources(&self, conversation_id: &str) -> Result<Vec<StoredSourceRecord>, String> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT id, conversation_id, run_id, call_id, title, url, source_level, platform,
                        captured_at, created_at
                 FROM sources WHERE conversation_id = ?1 ORDER BY created_at ASC, rowid ASC",
            )
            .map_err(|error| format!("无法读取来源记录：{error}"))?;
        let rows = statement
            .query_map([conversation_id], |row| {
                Ok(StoredSourceRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    run_id: row.get(2)?,
                    call_id: row.get(3)?,
                    title: row.get(4)?,
                    url: row.get(5)?,
                    source_level: row.get(6)?,
                    platform: row.get(7)?,
                    captured_at: row.get(8)?,
                    created_at: row.get(9)?,
                })
            })
            .map_err(|error| format!("无法查询来源记录：{error}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("无法解析来源记录：{error}"))
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
    use crate::tools::models::SourceRecord;

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

    #[test]
    fn persists_complete_tool_runs_and_sources_then_cascade_deletes_them() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ChatRepository::new(dir.path().join("paimon.db"));
        repository.initialize().unwrap();
        let conversation = repository
            .create_conversation("deepseek", "查询资料")
            .unwrap();
        let source = SourceRecord {
            title: "芙宁娜".into(),
            url: "https://wiki.example/furina".into(),
            source_level: "community_wiki".into(),
            platform: "BWiki".into(),
            captured_at: "2026-08-19T10:00:00Z".into(),
        };
        let result = ToolResult {
            tool_name: "read_bwiki_page".into(),
            content: "完整网页正文".into(),
            source: Some(source.clone()),
            links: vec![],
        };

        repository
            .record_tool_run(ToolRunInput {
                conversation_id: &conversation.id,
                run_id: "run-1",
                call_id: "call-1",
                tool_name: "read_bwiki_page",
                arguments: r#"{"title":"芙宁娜"}"#,
                status: "completed",
                result: Some(&result),
                error: None,
            })
            .unwrap();
        repository
            .record_tool_run(ToolRunInput {
                conversation_id: &conversation.id,
                run_id: "run-1",
                call_id: "call-2",
                tool_name: "read_url",
                arguments: r#"{"url":"https://invalid.example"}"#,
                status: "failed",
                result: None,
                error: Some("连接失败"),
            })
            .unwrap();

        let tool_runs = repository.list_tool_runs(&conversation.id).unwrap();
        assert_eq!(tool_runs.len(), 2);
        assert_eq!(tool_runs[0].call_id, "call-1");
        assert_eq!(tool_runs[0].result.as_ref().unwrap(), &result);
        assert_eq!(tool_runs[1].error.as_deref(), Some("连接失败"));
        let sources = repository.list_sources(&conversation.id).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].conversation_id, conversation.id);
        assert_eq!(sources[0].run_id, "run-1");
        assert_eq!(sources[0].call_id, "call-1");
        assert_eq!(sources[0].url, source.url);

        repository.delete_conversation(&conversation.id).unwrap();
        assert!(repository
            .list_tool_runs(&conversation.id)
            .unwrap()
            .is_empty());
        assert!(repository
            .list_sources(&conversation.id)
            .unwrap()
            .is_empty());
    }
}
