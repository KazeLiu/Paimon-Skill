use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    providers::models::{ChatMessage, ToolCall, ToolDefinition},
    tools::{models::SourceRecord, registry::ToolResult},
};

pub const AGENT_CANCELLED: &str = "__agent_cancelled__";
const MAX_TOOL_RESULT_CHARS: usize = 24_000;

#[derive(Debug, Clone)]
pub struct ModelTurn {
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Copy)]
pub struct AgentLoopOptions {
    pub max_tool_rounds: usize,
}

impl Default for AgentLoopOptions {
    fn default() -> Self {
        Self { max_tool_rounds: 4 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRunOutcome {
    pub call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub status: String,
    pub result: Option<ToolResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AgentLoopResult {
    pub content: String,
    pub messages: Vec<ChatMessage>,
    pub tool_runs: Vec<ToolRunOutcome>,
}

#[async_trait]
pub trait ModelBackend: Send + Sync {
    async fn next_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        on_delta: &(dyn Fn(String) + Send + Sync),
        cancellation: &CancellationToken,
    ) -> Result<ModelTurn, String>;
}

#[async_trait]
pub trait ToolBackend: Send + Sync {
    async fn execute(&self, name: &str, arguments: &str) -> Result<ToolResult, String>;
}

pub trait AgentEvents: Send + Sync {
    fn tool_started(&self, _name: &str) {}
    fn tool_finished(&self, _name: &str, _success: bool) {}
    fn source_added(&self, _source: &SourceRecord) {}
}

pub async fn run_agent_loop<M, T, E, D>(
    model: &M,
    tools: &T,
    events: &E,
    mut messages: Vec<ChatMessage>,
    definitions: Vec<ToolDefinition>,
    options: AgentLoopOptions,
    cancellation: CancellationToken,
    on_delta: D,
) -> Result<AgentLoopResult, String>
where
    M: ModelBackend,
    T: ToolBackend,
    E: AgentEvents,
    D: Fn(String) + Send + Sync,
{
    let mut displayed_content = String::new();
    let mut tool_runs = Vec::new();
    let mut completed_tool_rounds = 0usize;

    loop {
        if cancellation.is_cancelled() {
            return Err(AGENT_CANCELLED.into());
        }

        let turn = tokio::select! {
            _ = cancellation.cancelled() => return Err(AGENT_CANCELLED.into()),
            result = model.next_turn(&messages, &definitions, &on_delta, &cancellation) => result?,
        };
        displayed_content.push_str(&turn.content);

        if turn.tool_calls.is_empty() {
            if displayed_content.trim().is_empty() {
                return Err("模型没有返回可显示的文本".into());
            }
            messages.push(ChatMessage::assistant(&turn.content));
            return Ok(AgentLoopResult {
                content: displayed_content,
                messages,
                tool_runs,
            });
        }

        if completed_tool_rounds >= options.max_tool_rounds {
            return Err(format!(
                "已达到最大工具调用轮次（{}），模型仍在请求工具",
                options.max_tool_rounds
            ));
        }
        completed_tool_rounds += 1;
        messages.push(ChatMessage::assistant_with_tool_calls(
            &turn.content,
            &turn.tool_calls,
        ));

        for call in turn.tool_calls {
            if cancellation.is_cancelled() {
                return Err(AGENT_CANCELLED.into());
            }
            events.tool_started(&call.name);
            let execution = tokio::select! {
                _ = cancellation.cancelled() => return Err(AGENT_CANCELLED.into()),
                result = tools.execute(&call.name, &call.arguments) => result,
            };

            match execution {
                Ok(result) => {
                    if let Some(source) = &result.source {
                        events.source_added(source);
                    }
                    events.tool_finished(&call.name, true);
                    messages.push(ChatMessage::tool(&call.id, tool_result_for_model(&result)?));
                    tool_runs.push(ToolRunOutcome {
                        call_id: call.id,
                        tool_name: call.name,
                        arguments: call.arguments,
                        status: "completed".into(),
                        result: Some(result),
                        error: None,
                    });
                }
                Err(error) => {
                    events.tool_finished(&call.name, false);
                    messages.push(ChatMessage::tool(
                        &call.id,
                        serde_json::json!({
                            "ok": false,
                            "toolName": call.name,
                            "error": error,
                        })
                        .to_string(),
                    ));
                    tool_runs.push(ToolRunOutcome {
                        call_id: call.id,
                        tool_name: call.name,
                        arguments: call.arguments,
                        status: "failed".into(),
                        result: None,
                        error: Some(error),
                    });
                }
            }
        }
    }
}

fn tool_result_for_model(result: &ToolResult) -> Result<String, String> {
    let mut bounded = result.clone();
    let truncated = bounded.content.chars().count() > MAX_TOOL_RESULT_CHARS;
    if truncated {
        bounded.content = bounded
            .content
            .chars()
            .take(MAX_TOOL_RESULT_CHARS)
            .collect();
        bounded.content.push_str("\n[工具正文已按上下文上限截断]");
    }
    bounded.links.truncate(50);
    serde_json::to_string(&bounded).map_err(|error| format!("工具结果序列化失败：{error}"))
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Mutex};

    use async_trait::async_trait;
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        providers::models::{ChatMessage, ToolCall, ToolDefinition},
        tools::registry::ToolResult,
    };

    struct FakeModel(Mutex<VecDeque<Result<ModelTurn, String>>>);

    #[async_trait]
    impl ModelBackend for FakeModel {
        async fn next_turn(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            on_delta: &(dyn Fn(String) + Send + Sync),
            _cancellation: &CancellationToken,
        ) -> Result<ModelTurn, String> {
            let turn = self.0.lock().unwrap().pop_front().unwrap()?;
            if !turn.content.is_empty() {
                on_delta(turn.content.clone());
            }
            Ok(turn)
        }
    }

    struct FakeTools(Mutex<VecDeque<Result<ToolResult, String>>>);

    #[async_trait]
    impl ToolBackend for FakeTools {
        async fn execute(&self, _name: &str, _arguments: &str) -> Result<ToolResult, String> {
            self.0.lock().unwrap().pop_front().unwrap()
        }
    }

    struct BlockingModel;

    #[async_trait]
    impl ModelBackend for BlockingModel {
        async fn next_turn(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _on_delta: &(dyn Fn(String) + Send + Sync),
            _cancellation: &CancellationToken,
        ) -> Result<ModelTurn, String> {
            std::future::pending().await
        }
    }

    struct BlockingTools;

    #[async_trait]
    impl ToolBackend for BlockingTools {
        async fn execute(&self, _name: &str, _arguments: &str) -> Result<ToolResult, String> {
            std::future::pending().await
        }
    }

    #[derive(Default)]
    struct RecordingEvents(Mutex<Vec<String>>);

    impl AgentEvents for RecordingEvents {
        fn tool_started(&self, name: &str) {
            self.0.lock().unwrap().push(format!("started:{name}"));
        }

        fn tool_finished(&self, name: &str, success: bool) {
            self.0
                .lock()
                .unwrap()
                .push(format!("finished:{name}:{success}"));
        }

        fn source_added(&self, source: &crate::tools::models::SourceRecord) {
            self.0
                .lock()
                .unwrap()
                .push(format!("source:{}", source.url));
        }
    }

    fn turn(content: &str, calls: Vec<ToolCall>) -> Result<ModelTurn, String> {
        Ok(ModelTurn {
            content: content.into(),
            tool_calls: calls,
        })
    }

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: "{}".into(),
        }
    }

    fn tool_result(name: &str) -> Result<ToolResult, String> {
        Ok(ToolResult {
            tool_name: name.into(),
            content: "result".into(),
            source: None,
            links: vec![],
        })
    }

    #[tokio::test]
    async fn executes_multiple_tool_rounds_before_returning_final_text() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "search")]),
            turn("", vec![call("2", "read")]),
            turn("最终总结", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([
            tool_result("search"),
            tool_result("read"),
        ])));
        let events = RecordingEvents::default();
        let result = run_agent_loop(
            &model,
            &tools,
            &events,
            vec![ChatMessage::user("query")],
            vec![],
            AgentLoopOptions { max_tool_rounds: 4 },
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "最终总结");
        assert_eq!(result.tool_runs.len(), 2);
        assert!(result
            .messages
            .iter()
            .any(|message| message.role == "tool" && message.content.contains("result")));
        assert_eq!(
            result.messages.last().unwrap(),
            &ChatMessage::assistant("最终总结")
        );
    }

    #[tokio::test]
    async fn returns_tool_failures_to_the_model_instead_of_aborting() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "read")]),
            turn("该网页读取失败，我改用已有资料回答。", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([Err("timeout".into())])));
        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions { max_tool_rounds: 2 },
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.tool_runs[0].status, "failed");
        assert!(result
            .messages
            .iter()
            .any(|message| message.role == "tool" && message.content.contains("timeout")));
        assert_eq!(
            result.messages.last().unwrap(),
            &ChatMessage::assistant("该网页读取失败，我改用已有资料回答。")
        );
    }

    #[tokio::test]
    async fn stops_when_tool_round_limit_is_reached() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "search")]),
            turn("", vec![call("2", "search")]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([tool_result("search")])));
        let error = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions { max_tool_rounds: 1 },
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

        assert!(error.contains("最大工具调用轮次"));
    }

    #[tokio::test]
    async fn cancellation_is_observed_between_model_and_tool_turns() {
        let token = CancellationToken::new();
        token.cancel();
        let error = run_agent_loop(
            &FakeModel(Mutex::new(VecDeque::new())),
            &FakeTools(Mutex::new(VecDeque::new())),
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions { max_tool_rounds: 2 },
            token,
            |_| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, AGENT_CANCELLED);
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_in_flight_model_request() {
        let token = CancellationToken::new();
        let canceller = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            canceller.cancel();
        });

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            run_agent_loop(
                &BlockingModel,
                &FakeTools(Mutex::new(VecDeque::new())),
                &RecordingEvents::default(),
                vec![],
                vec![],
                AgentLoopOptions::default(),
                token,
                |_| {},
            ),
        )
        .await
        .expect("agent cancellation timed out");

        assert_eq!(result.unwrap_err(), AGENT_CANCELLED);
    }

    #[tokio::test]
    async fn cancellation_interrupts_an_in_flight_tool() {
        let token = CancellationToken::new();
        let canceller = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            canceller.cancel();
        });
        let model = FakeModel(Mutex::new(VecDeque::from([turn(
            "",
            vec![call("1", "read_url")],
        )])));

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            run_agent_loop(
                &model,
                &BlockingTools,
                &RecordingEvents::default(),
                vec![],
                vec![],
                AgentLoopOptions::default(),
                token,
                |_| {},
            ),
        )
        .await
        .expect("tool cancellation timed out");

        assert_eq!(result.unwrap_err(), AGENT_CANCELLED);
    }
}
