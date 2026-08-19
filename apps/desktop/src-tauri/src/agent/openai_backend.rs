use std::collections::BTreeMap;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::providers::{
    models::{ChatMessage, ProviderConfig, StreamEvent, ToolCall, ToolDefinition},
    openai_compatible::OpenAiCompatibleClient,
};

use super::runtime::{ModelBackend, ModelTurn, AGENT_CANCELLED};

#[derive(Debug, Default)]
struct PartialToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[derive(Debug, Default)]
pub struct StreamToolCalls(BTreeMap<usize, PartialToolCall>);

impl StreamToolCalls {
    pub fn push(
        &mut self,
        index: usize,
        id: Option<String>,
        name: Option<String>,
        arguments: &str,
    ) {
        let call = self.0.entry(index).or_default();
        if id.is_some() {
            call.id = id;
        }
        if name.is_some() {
            call.name = name;
        }
        call.arguments.push_str(arguments);
    }

    pub fn finish(self) -> Result<Vec<ToolCall>, String> {
        self.0
            .into_iter()
            .map(|(index, call)| {
                Ok(ToolCall {
                    id: call
                        .id
                        .ok_or_else(|| format!("第 {index} 个流式工具调用缺少调用 ID"))?,
                    name: call
                        .name
                        .ok_or_else(|| format!("第 {index} 个流式工具调用缺少工具名"))?,
                    arguments: call.arguments,
                })
            })
            .collect()
    }
}

pub struct OpenAiModelBackend {
    client: OpenAiCompatibleClient,
    provider: ProviderConfig,
    api_key: String,
}

impl OpenAiModelBackend {
    pub fn new(provider: ProviderConfig, api_key: String) -> Result<Self, String> {
        Ok(Self {
            client: OpenAiCompatibleClient::new()?,
            provider,
            api_key,
        })
    }
}

#[async_trait]
impl ModelBackend for OpenAiModelBackend {
    async fn next_turn(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        on_delta: &(dyn Fn(String) + Send + Sync),
        cancellation: &CancellationToken,
    ) -> Result<ModelTurn, String> {
        if !self.provider.streaming {
            let result = tokio::select! {
                _ = cancellation.cancelled() => return Err(AGENT_CANCELLED.into()),
                result = self.client.complete(&self.provider, &self.api_key, messages, tools) => result?,
            };
            if !result.content.is_empty() {
                on_delta(result.content.clone());
            }
            return Ok(ModelTurn {
                content: result.content,
                tool_calls: result.tool_calls,
            });
        }

        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let client = self.client.clone();
        let provider = self.provider.clone();
        let api_key = self.api_key.clone();
        let messages = messages.to_vec();
        let tools = tools.to_vec();
        let stream_task = tauri::async_runtime::spawn(async move {
            client
                .stream(&provider, &api_key, &messages, &tools, &sender)
                .await
        });

        let mut content = String::new();
        let mut calls = StreamToolCalls::default();
        loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    stream_task.abort();
                    return Err(AGENT_CANCELLED.into());
                }
                event = receiver.recv() => {
                    match event {
                        Some(StreamEvent::ContentDelta(delta)) => {
                            content.push_str(&delta);
                            on_delta(delta);
                        }
                        Some(StreamEvent::ToolCallDelta { index, id, name, arguments }) => {
                            calls.push(index, id, name, &arguments);
                        }
                        Some(StreamEvent::Done) | None => break,
                    }
                }
            }
        }
        stream_task
            .await
            .map_err(|error| format!("模型流任务异常：{error}"))??;
        Ok(ModelTurn {
            content,
            tool_calls: calls.finish()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_tool_calls_are_assembled_by_index_even_when_names_match() {
        let mut calls = StreamToolCalls::default();
        calls.push(
            0,
            Some("call-a".into()),
            Some("read_url".into()),
            "{\"url\":",
        );
        calls.push(
            1,
            Some("call-b".into()),
            Some("read_url".into()),
            "{\"url\":",
        );
        calls.push(0, None, None, "\"https://a.example\"}");
        calls.push(1, None, None, "\"https://b.example\"}");

        let calls = calls.finish().unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call-a");
        assert!(calls[0].arguments.contains("a.example"));
        assert!(calls[1].arguments.contains("b.example"));
    }

    #[test]
    fn incomplete_stream_tool_call_is_rejected() {
        let mut calls = StreamToolCalls::default();
        calls.push(0, None, Some("read_url".into()), "{}");

        assert!(calls.finish().unwrap_err().contains("缺少调用 ID"));
    }
}
