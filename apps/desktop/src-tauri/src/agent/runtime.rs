use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::{
    agent::research::ResearchLedger,
    providers::models::{ChatMessage, ToolCall, ToolDefinition},
    tools::{models::SourceRecord, registry::ToolResult},
};

pub const AGENT_CANCELLED: &str = "__agent_cancelled__";
const MAX_TOOL_RESULT_CHARS: usize = 24_000;
const MAX_MODEL_ATTEMPTS: u8 = 3;
const MAX_SEGMENT_DETAILS_CHARS: usize = 2_000;

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
        Self {
            max_tool_rounds: 12,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssistantSegment {
    pub id: String,
    pub kind: String,
    pub content: String,
    pub tool_name: Option<String>,
    pub status: Option<String>,
    pub details: Option<String>,
    pub url: Option<String>,
    pub platform: Option<String>,
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
    pub segments: Vec<AssistantSegment>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub messages: Vec<ChatMessage>,
    #[cfg_attr(not(test), allow(dead_code))]
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

    fn take_diagnostics(&self) -> Vec<crate::providers::models::CapabilityDiagnostic> {
        Vec::new()
    }
}

#[async_trait]
pub trait ToolBackend: Send + Sync {
    async fn execute(&self, name: &str, arguments: &str) -> Result<ToolResult, String>;
}

pub trait AgentEvents: Send + Sync {
    fn model_request_started(
        &self,
        _attempt: u8,
        _messages: &[ChatMessage],
        _tools: &[ToolDefinition],
    ) {
    }
    fn model_request_finished(
        &self,
        _attempt: u8,
        _turn: Option<&ModelTurn>,
        _error: Option<&str>,
    ) {
    }
    fn tool_started(&self, _call: &ToolCall) {}
    fn tool_finished(&self, _outcome: &ToolRunOutcome) {}
    fn source_added(&self, _source: &SourceRecord) {}
    fn model_retrying(&self, _attempt: u8, _error: &str) {}
    fn model_retrying_with_diagnostics(
        &self,
        attempt: u8,
        error: &str,
        _diagnostics: &[crate::providers::models::CapabilityDiagnostic],
    ) {
        self.model_retrying(attempt, error);
    }
    fn tool_recorded(&self, _outcome: &ToolRunOutcome) -> Result<(), String> {
        Ok(())
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub async fn run_agent_loop<M, T, E, D>(
    model: &M,
    tools: &T,
    events: &E,
    messages: Vec<ChatMessage>,
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
    run_agent_loop_with_research(
        model,
        tools,
        events,
        messages,
        definitions,
        options,
        cancellation,
        on_delta,
        None,
    )
    .await
}

pub async fn run_agent_loop_with_research<M, T, E, D>(
    model: &M,
    tools: &T,
    events: &E,
    mut messages: Vec<ChatMessage>,
    definitions: Vec<ToolDefinition>,
    options: AgentLoopOptions,
    cancellation: CancellationToken,
    on_delta: D,
    mut research: Option<&mut ResearchLedger>,
) -> Result<AgentLoopResult, String>
where
    M: ModelBackend,
    T: ToolBackend,
    E: AgentEvents,
    D: Fn(String) + Send + Sync,
{
    let mut tool_runs = Vec::new();
    let mut completed_tool_rounds = 0usize;
    let mut research_followups = 0usize;
    let mut segments = Vec::new();
    let mut text_segment_index = 0usize;
    let mut terminal_research = false;

    loop {
        if cancellation.is_cancelled() {
            return Err(AGENT_CANCELLED.into());
        }

        // Completing Bilibili collection closes the browser workflow, not
        // outstanding wiki or first-party fact checks.
        let request_definitions = definitions
            .iter()
            .filter(|tool| !terminal_research || is_fact_tool(&tool.function.name))
            .cloned()
            .collect::<Vec<_>>();
        let request_messages = messages_with_runtime_tools(
            &messages,
            &request_definitions,
            terminal_research,
        );
        let mut attempt = 1u8;
        let turn = loop {
            events.model_request_started(attempt, &request_messages, &request_definitions);
            let result = tokio::select! {
                _ = cancellation.cancelled() => return Err(AGENT_CANCELLED.into()),
                result = model.next_turn(&request_messages, &request_definitions, &on_delta, &cancellation) => result,
            };
            match result {
                Ok(turn) => {
                    events.model_request_finished(attempt, Some(&turn), None);
                    break turn;
                }
                Err(error) if error == AGENT_CANCELLED => {
                    events.model_request_finished(attempt, None, Some(&error));
                    return Err(error);
                }
                Err(error) if attempt < MAX_MODEL_ATTEMPTS => {
                    events.model_request_finished(attempt, None, Some(&error));
                    attempt += 1;
                    let diagnostics = model.take_diagnostics();
                    events.model_retrying_with_diagnostics(attempt, &error, &diagnostics);
                }
                Err(error) => {
                    events.model_request_finished(attempt, None, Some(&error));
                    return Err(error);
                }
            }
        };
        if !turn.content.trim().is_empty() {
            text_segment_index += 1;
            segments.push(AssistantSegment {
                id: format!("text-{text_segment_index}"),
                kind: "text".into(),
                content: turn.content.clone(),
                tool_name: None,
                status: None,
                details: None,
                url: None,
                platform: None,
            });
        }
        if turn.tool_calls.is_empty() {
            if let Some(ledger) = research.as_deref_mut() {
                if let Some(instruction) = ledger.final_gate_for_tools(&request_definitions) {
                    if research_followups >= 2 {
                        return Err(format!("研究流程未完成：{instruction}"));
                    }
                    research_followups += 1;
                    messages.push(ChatMessage::assistant(&turn.content));
                    messages.push(ChatMessage::user(instruction));
                    continue;
                }
            }
            if turn.content.trim().is_empty() {
                return Err("模型没有返回可显示的文本".into());
            }
            messages.push(ChatMessage::assistant(&turn.content));
            let content = segments
                .iter()
                .filter(|segment| segment.kind == "text")
                .map(|segment| segment.content.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            return Ok(AgentLoopResult {
                content,
                segments,
                messages,
                tool_runs,
            });
        }

        if terminal_research && turn.tool_calls.iter().all(|call| !is_fact_tool(&call.name)) {
            return Err("固定研究流程已完成，模型不应继续调用工具".into());
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
            if terminal_research && !is_fact_tool(&call.name) {
                let outcome = ToolRunOutcome {
                    call_id: call.id,
                    tool_name: call.name,
                    arguments: call.arguments,
                    status: "failed".into(),
                    result: None,
                    error: Some("固定研究流程已完成，已跳过同批次的额外工具调用".into()),
                };
                segments.push(tool_segment(&outcome));
                events.tool_recorded(&outcome)?;
                events.tool_finished(&outcome);
                messages.push(ChatMessage::tool(
                    &outcome.call_id,
                    serde_json::json!({
                        "ok": false,
                        "toolName": outcome.tool_name,
                        "error": outcome.error,
                    })
                    .to_string(),
                ));
                tool_runs.push(outcome);
                continue;
            }
            events.tool_started(&call);
            let execution = tokio::select! {
                _ = cancellation.cancelled() => return Err(AGENT_CANCELLED.into()),
                result = tools.execute(&call.name, &call.arguments) => result,
            };

            match execution {
                Ok(result) => {
                    if result
                        .metadata
                        .get("terminal")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    {
                        terminal_research = true;
                    }
                    if let Some(ledger) = research.as_deref_mut() {
                        ledger.observe(&result);
                    }
                    if let Some(source) = &result.source {
                        events.source_added(source);
                    }
                    let outcome = ToolRunOutcome {
                        call_id: call.id,
                        tool_name: call.name,
                        arguments: call.arguments,
                        status: "completed".into(),
                        result: Some(result),
                        error: None,
                    };
                    segments.push(tool_segment(&outcome));
                    events.tool_recorded(&outcome)?;
                    events.tool_finished(&outcome);
                    messages.push(ChatMessage::tool(
                        &outcome.call_id,
                        tool_result_for_model(
                            outcome.result.as_ref().expect("completed tool result"),
                        )?,
                    ));
                    tool_runs.push(outcome);
                }
                Err(error) => {
                    let outcome = ToolRunOutcome {
                        call_id: call.id,
                        tool_name: call.name,
                        arguments: call.arguments,
                        status: "failed".into(),
                        result: None,
                        error: Some(error),
                    };
                    segments.push(tool_segment(&outcome));
                    events.tool_recorded(&outcome)?;
                    events.tool_finished(&outcome);
                    messages.push(ChatMessage::tool(
                        &outcome.call_id,
                        serde_json::json!({
                            "ok": false,
                            "toolName": outcome.tool_name,
                            "error": outcome.error,
                        })
                        .to_string(),
                    ));
                    tool_runs.push(outcome);
                }
            }
        }
    }
}

fn is_fact_tool(name: &str) -> bool {
    matches!(name, "read_url" | "search_bwiki" | "read_bwiki_page")
}

fn messages_with_runtime_tools(
    messages: &[ChatMessage],
    definitions: &[ToolDefinition],
    terminal_research: bool,
) -> Vec<ChatMessage> {
    let mut context = String::from(
        "[桌面应用本轮执行能力]\nSkill 是领域规则，不会创建工具、执行 shell 命令或提供 MCP 连接。工具可用性以本轮实际发送的 tools 清单为准；不要根据旧回答、参考文档或网页内容推断工具缺失。\n",
    );
    if definitions.is_empty() {
        context.push_str(if terminal_research {
            "B站采集已结束，本轮没有待调用工具。请依据已取得的证据总结，明确未覆盖或读取失败的来源；不要要求用户安装依赖。"
        } else {
            "当前模型配置未启用工具调用，本轮没有可调用工具。涉及实时查询时说明限制，并提示在 设置 → 模型 中启用工具调用、使用支持工具的模型；不要声称已查询，也不要要求安装 OpenCLI。"
        });
    } else {
        context.push_str("本轮可用工具：");
        context.push_str(
            &definitions
                .iter()
                .map(|tool| tool.function.name.as_str())
                .collect::<Vec<_>>()
                .join("、"),
        );
        context.push_str("。调用前按对应 schema 填写参数；读取失败与工具不存在是两种情况，失败时说明实际错误和证据范围。");
        if terminal_research {
            context.push_str("\nB站采集已结束，只停止继续浏览器采集；仍可用上列工具补充 BWiki 或公开网页的事实正文。补齐必要事实后总结，不再启动 B站流程或临时浏览器。其他平台未读取时必须明确未覆盖。");
        }
    }
    let mut request = messages.to_vec();
    if let Some(system) = request.iter_mut().find(|message| message.role == "system") {
        system.content = format!("{context}\n\n{}", system.content);
    } else {
        request.insert(0, ChatMessage::system(context));
    }
    request
}

fn tool_segment(outcome: &ToolRunOutcome) -> AssistantSegment {
    let source = outcome
        .result
        .as_ref()
        .and_then(|result| result.source.as_ref());
    let details = outcome
        .result
        .as_ref()
        .map(|result| result.content.as_str())
        .or(outcome.error.as_deref())
        .map(|details| details.chars().take(MAX_SEGMENT_DETAILS_CHARS).collect());
    AssistantSegment {
        id: format!("tool-{}", outcome.call_id),
        kind: "tool".into(),
        content: String::new(),
        tool_name: Some(outcome.tool_name.clone()),
        status: Some(outcome.status.clone()),
        details,
        url: source.map(|source| source.url.clone()),
        platform: source.map(|source| source.platform.clone()),
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

    struct DiagnosticModel {
        turns: Mutex<VecDeque<Result<ModelTurn, String>>>,
        diagnostics: Mutex<Vec<crate::providers::models::CapabilityDiagnostic>>,
    }

    struct RecordingToolsModel {
        turns: Mutex<VecDeque<Result<ModelTurn, String>>>,
        tool_counts: Mutex<Vec<usize>>,
        tool_names: Mutex<Vec<Vec<String>>>,
        requests: Mutex<Vec<Vec<ChatMessage>>>,
    }

    #[async_trait]
    impl ModelBackend for RecordingToolsModel {
        async fn next_turn(
            &self,
            messages: &[ChatMessage],
            tools: &[ToolDefinition],
            _on_delta: &(dyn Fn(String) + Send + Sync),
            _cancellation: &CancellationToken,
        ) -> Result<ModelTurn, String> {
            self.tool_counts.lock().unwrap().push(tools.len());
            self.tool_names.lock().unwrap().push(
                tools.iter().map(|tool| tool.function.name.clone()).collect(),
            );
            self.requests.lock().unwrap().push(messages.to_vec());
            self.turns.lock().unwrap().pop_front().unwrap()
        }
    }

    #[async_trait]
    impl ModelBackend for DiagnosticModel {
        fn take_diagnostics(&self) -> Vec<crate::providers::models::CapabilityDiagnostic> {
            std::mem::take(&mut *self.diagnostics.lock().unwrap())
        }

        async fn next_turn(
            &self,
            _messages: &[ChatMessage],
            _tools: &[ToolDefinition],
            _on_delta: &(dyn Fn(String) + Send + Sync),
            _cancellation: &CancellationToken,
        ) -> Result<ModelTurn, String> {
            self.turns.lock().unwrap().pop_front().unwrap()
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
        fn tool_started(&self, call: &ToolCall) {
            self.0
                .lock()
                .unwrap()
                .push(format!("started:{}", call.name));
        }

        fn tool_finished(&self, outcome: &ToolRunOutcome) {
            self.0.lock().unwrap().push(format!(
                "finished:{}:{}",
                outcome.tool_name,
                outcome.status == "completed"
            ));
        }

        fn source_added(&self, source: &crate::tools::models::SourceRecord) {
            self.0
                .lock()
                .unwrap()
                .push(format!("source:{}", source.url));
        }

        fn tool_recorded(&self, outcome: &ToolRunOutcome) -> Result<(), String> {
            self.0
                .lock()
                .unwrap()
                .push(format!("recorded:{}:{}", outcome.call_id, outcome.status));
            Ok(())
        }

        fn model_retrying(&self, attempt: u8, error: &str) {
            self.0
                .lock()
                .unwrap()
                .push(format!("retrying:{attempt}:{error}"));
        }
    }

    #[derive(Default)]
    struct DiagnosticEvents(Mutex<Vec<String>>);

    impl AgentEvents for DiagnosticEvents {
        fn model_retrying_with_diagnostics(
            &self,
            attempt: u8,
            error: &str,
            diagnostics: &[crate::providers::models::CapabilityDiagnostic],
        ) {
            self.0.lock().unwrap().push(format!(
                "retry:{attempt}:{error}:{}",
                diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.request_body.as_str())
                    .unwrap_or_default()
            ));
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
            metadata: Default::default(),
        })
    }

    fn terminal_tool_result(name: &str) -> Result<ToolResult, String> {
        let mut result = tool_result(name)?;
        result
            .metadata
            .insert("terminal".into(), serde_json::Value::Bool(true));
        Ok(result)
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
    async fn terminal_research_removes_tools_from_the_summary_turn() {
        let model = RecordingToolsModel {
            turns: Mutex::new(VecDeque::from([
                turn("", vec![call("1", "research_bilibili")]),
                turn("根据评论和字幕总结", vec![]),
            ])),
            tool_counts: Mutex::new(Vec::new()),
            tool_names: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        };
        let tools = FakeTools(Mutex::new(VecDeque::from([terminal_tool_result(
            "research_bilibili",
        )])));
        let definitions = vec![ToolDefinition {
            r#type: "function".into(),
            function: crate::providers::models::ToolFunction {
                name: "research_bilibili".into(),
                description: "固定流程".into(),
                parameters: serde_json::json!({"type":"object"}),
            },
        }];

        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![ChatMessage::user("去B站看看")],
            definitions,
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "根据评论和字幕总结");
        assert_eq!(*model.tool_counts.lock().unwrap(), vec![1, 0]);
    }

    #[tokio::test]
    async fn terminal_research_preserves_fact_tools_until_wiki_evidence_is_read() {
        let model = RecordingToolsModel {
            turns: Mutex::new(VecDeque::from([
                turn("", vec![call("1", "search_bwiki"), call("2", "research_bilibili")]),
                turn("目前只有搜索线索", vec![]),
                turn("", vec![call("3", "read_bwiki_page")]),
                turn("事实和社区结论", vec![]),
            ])),
            tool_counts: Mutex::new(Vec::new()),
            tool_names: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        };
        let tools = FakeTools(Mutex::new(VecDeque::from([
            tool_result("search_bwiki"),
            terminal_tool_result("research_bilibili"),
            tool_result("read_bwiki_page"),
        ])));
        let mut research = ResearchLedger::new("角色配队");
        let result = run_agent_loop_with_research(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![ChatMessage::system("领域规则"), ChatMessage::user("角色配队")],
            crate::tools::registry::definitions(),
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
            Some(&mut research),
        )
        .await
        .unwrap();

        assert!(result.content.ends_with("事实和社区结论"));
        assert_eq!(result.tool_runs.len(), 3);
        let names = model.tool_names.lock().unwrap();
        assert!(names[1].iter().any(|name| name == "read_bwiki_page"));
        assert!(!names[1].iter().any(|name| name.starts_with("browser_")));
        let requests = model.requests.lock().unwrap();
        assert!(requests[1][0].content.contains("read_bwiki_page"));
        assert!(requests[1][0].content.contains("B站采集已结束"));
        assert_eq!(research.final_gate(), None);
    }

    #[tokio::test]
    async fn terminal_research_allows_fact_read_later_in_the_same_batch() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "research_bilibili"), call("2", "read_bwiki_page")]),
            turn("总结", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([
            terminal_tool_result("research_bilibili"),
            tool_result("read_bwiki_page"),
        ])));
        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            crate::tools::registry::definitions(),
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.tool_runs[1].status, "completed");
        assert!(tools.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn runtime_explains_when_tool_calling_is_disabled() {
        let model = RecordingToolsModel {
            turns: Mutex::new(VecDeque::from([turn("无法实时查询", vec![])])),
            tool_counts: Mutex::new(Vec::new()),
            tool_names: Mutex::new(Vec::new()),
            requests: Mutex::new(Vec::new()),
        };
        let mut research = ResearchLedger::new("去B站看看角色配队");
        run_agent_loop_with_research(
            &model,
            &FakeTools(Mutex::new(VecDeque::new())),
            &RecordingEvents::default(),
            vec![ChatMessage::system("领域规则")],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
            Some(&mut research),
        )
        .await
        .unwrap();

        let requests = model.requests.lock().unwrap();
        assert!(requests[0][0].content.contains("未启用工具调用"));
        assert!(requests[0][0].content.contains("设置 → 模型"));
        assert!(!requests[0][0].content.contains("read_bwiki_page"));
    }

    #[tokio::test]
    async fn terminal_research_rejects_a_followup_tool_call() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "research_bilibili")]),
            turn("", vec![call("2", "browser_open_page")]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([terminal_tool_result(
            "research_bilibili",
        )])));

        let error = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, "固定研究流程已完成，模型不应继续调用工具");
    }

    #[tokio::test]
    async fn terminal_research_skips_later_tools_in_the_same_batch() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn(
                "",
                vec![
                    call("1", "research_bilibili"),
                    call("2", "browser_open_page"),
                ],
            ),
            turn("总结", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([terminal_tool_result(
            "research_bilibili",
        )])));

        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "总结");
        assert_eq!(result.tool_runs.len(), 2);
        assert_eq!(result.tool_runs[0].status, "completed");
        assert_eq!(result.tool_runs[1].status, "failed");
        assert!(result.tool_runs[1]
            .error
            .as_deref()
            .unwrap()
            .contains("同批次"));
    }

    #[tokio::test]
    async fn emits_request_diagnostic_immediately_when_retrying() {
        let model = DiagnosticModel {
            turns: Mutex::new(VecDeque::from([
                Err("连接失败".into()),
                turn("完成", vec![]),
            ])),
            diagnostics: Mutex::new(vec![crate::providers::models::CapabilityDiagnostic {
                stage: "request".into(),
                endpoint: "https://relay.example/v1/responses".into(),
                http_status: None,
                content_type: None,
                response_headers: vec![],
                response_body: String::new(),
                parse_error: Some("kind=connect".into()),
                truncated: false,
                request_body: "{\"model\":\"test\"}".into(),
            }]),
        };
        let events = DiagnosticEvents::default();

        let result = run_agent_loop(
            &model,
            &FakeTools(Mutex::new(VecDeque::new())),
            &events,
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "完成");
        let events = events.0.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].contains("连接失败"));
        assert!(events[0].contains("{\"model\":\"test\"}"));
    }

    #[tokio::test]
    async fn preserves_text_and_tool_segments_in_their_actual_order() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("先说明查询方向", vec![call("1", "search")]),
            turn("找到候选页面，继续读取", vec![call("2", "read")]),
            turn("最终总结", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([
            tool_result("search"),
            tool_result("read"),
        ])));

        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![ChatMessage::user("query")],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(
            result
                .segments
                .iter()
                .map(|segment| (
                    segment.kind.as_str(),
                    segment.content.as_str(),
                    segment.tool_name.as_deref()
                ))
                .collect::<Vec<_>>(),
            vec![
                ("text", "先说明查询方向", None),
                ("tool", "", Some("search")),
                ("text", "找到候选页面，继续读取", None),
                ("tool", "", Some("read")),
                ("text", "最终总结", None),
            ]
        );
    }

    #[tokio::test]
    async fn default_limit_allows_more_than_four_tool_rounds() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "read")]),
            turn("", vec![call("2", "read")]),
            turn("", vec![call("3", "read")]),
            turn("", vec![call("4", "read")]),
            turn("", vec![call("5", "read")]),
            turn("完成", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([
            tool_result("read"),
            tool_result("read"),
            tool_result("read"),
            tool_result("read"),
            tool_result("read"),
        ])));

        let result = run_agent_loop(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "完成");
        assert_eq!(result.tool_runs.len(), 5);
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
    async fn research_gate_requires_scroll_and_uses_only_the_final_answer() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "browser_open_page")]),
            turn("这是尚未完成的总结", vec![]),
            turn("", vec![call("2", "browser_scroll_page")]),
            turn("完成后的正式总结", vec![]),
        ])));
        let source = |tool_name: &str| ToolResult {
            tool_name: tool_name.into(),
            content: "页面内容".into(),
            source: Some(SourceRecord {
                title: "视频".into(),
                url: "https://www.bilibili.com/video/BV1abc".into(),
                source_level: "browser_webview".into(),
                platform: "bilibili".into(),
                captured_at: "now".into(),
            }),
            links: vec![],
            metadata: Default::default(),
        };
        let tools = FakeTools(Mutex::new(VecDeque::from([
            Ok(source("browser_open_page")),
            Ok(source("browser_scroll_page")),
        ])));
        let mut research = ResearchLedger::new("视频信息");

        let result = run_agent_loop_with_research(
            &model,
            &tools,
            &RecordingEvents::default(),
            vec![ChatMessage::user("视频信息")],
            crate::tools::registry::definitions(),
            AgentLoopOptions { max_tool_rounds: 4 },
            CancellationToken::new(),
            |_| {},
            Some(&mut research),
        )
        .await
        .unwrap();

        assert_eq!(result.content, "这是尚未完成的总结\n\n完成后的正式总结");
        assert_eq!(result.tool_runs.len(), 2);
    }

    #[tokio::test]
    async fn records_completed_tools_even_if_a_later_model_turn_fails() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "read")]),
            Err("模型连接中断".into()),
            Err("模型连接中断".into()),
            Err("模型连接中断".into()),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([tool_result("read")])));
        let events = RecordingEvents::default();

        let error = run_agent_loop(
            &model,
            &tools,
            &events,
            vec![],
            vec![],
            AgentLoopOptions { max_tool_rounds: 2 },
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, "模型连接中断");
        assert!(events
            .0
            .lock()
            .unwrap()
            .contains(&"recorded:1:completed".into()));
    }

    #[tokio::test]
    async fn retries_one_model_turn_without_replaying_completed_tools() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            turn("", vec![call("1", "read")]),
            Err("临时 503".into()),
            turn("最终总结", vec![]),
        ])));
        let tools = FakeTools(Mutex::new(VecDeque::from([tool_result("read")])));
        let events = RecordingEvents::default();

        let result = run_agent_loop(
            &model,
            &tools,
            &events,
            vec![ChatMessage::user("query")],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.content, "最终总结");
        assert_eq!(result.tool_runs.len(), 1);
        assert_eq!(
            events
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|event| event.starts_with("recorded:"))
                .count(),
            1
        );
        assert!(events
            .0
            .lock()
            .unwrap()
            .contains(&"retrying:2:临时 503".into()));
    }

    #[tokio::test]
    async fn returns_error_only_after_three_model_attempts() {
        let model = FakeModel(Mutex::new(VecDeque::from([
            Err("失败 1".into()),
            Err("失败 2".into()),
            Err("失败 3".into()),
        ])));
        let events = RecordingEvents::default();

        let error = run_agent_loop(
            &model,
            &FakeTools(Mutex::new(VecDeque::new())),
            &events,
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, "失败 3");
        assert_eq!(
            events
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|event| event.starts_with("retrying:"))
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn does_not_retry_a_cancelled_model_request() {
        let model = FakeModel(Mutex::new(VecDeque::from([Err(AGENT_CANCELLED.into())])));
        let events = RecordingEvents::default();

        let error = run_agent_loop(
            &model,
            &FakeTools(Mutex::new(VecDeque::new())),
            &events,
            vec![],
            vec![],
            AgentLoopOptions::default(),
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap_err();

        assert_eq!(error, AGENT_CANCELLED);
        assert!(events.0.lock().unwrap().is_empty());
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
