use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Instant,
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::{
    context::build_messages,
    events::ChatEvent,
    openai_backend::OpenAiModelBackend,
    runtime::{
        run_agent_loop_with_research, AgentEvents, AgentLoopOptions, ToolBackend, AGENT_CANCELLED,
    },
    state::RunState,
};
use crate::attachments::{validate_attachments, ChatAttachment};
use crate::diagnostics::{
    export_markdown, DiagnosticsSettings, DiagnosticsSettingsRepository, LogEntry, LogRepository,
};

use crate::prompt::{
    composer::compose_prompt,
    models::{ComposedPrompt, PromptFragment, PromptSettings},
    repository::PromptRepository,
    resources::{load_fragments, resolve_resource_dir},
};
use crate::providers::{
    models::{CapabilityReport, ProviderConfig},
    openai_compatible::OpenAiCompatibleClient,
    repository::ProviderRepository,
    secrets::{delete_secret, load_secret, save_secret},
};
use crate::research::model_tasks::{
    OpenAiCompatibleVisualTaskExecutor, UnavailableVisualTaskExecutor, VisualTaskExecutor,
};
use crate::storage::{
    models::{Conversation, Message, StoredSourceRecord, ToolRunRecord},
    repository::{ChatRepository, ToolRunInput},
};
use crate::tools::{models::SourceRecord, registry::ToolResult};

fn settings_repository(app: &AppHandle) -> Result<PromptRepository, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位应用配置目录：{error}"))?;
    Ok(PromptRepository::new(
        directory.join("prompt-settings.json"),
    ))
}

fn provider_repository(app: &AppHandle) -> Result<ProviderRepository, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位应用配置目录：{error}"))?;
    Ok(ProviderRepository::new(directory.join("providers.json")))
}

fn log_repository(app: &AppHandle) -> Result<LogRepository, String> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| format!("无法定位应用日志目录：{error}"))?;
    let settings = diagnostics_settings_repository(app)?.load()?;
    Ok(LogRepository::with_verbose(
        log_directory.join("diagnostics.json"),
        settings.verbose,
    ))
}

pub(crate) fn append_research_workflow_log(
    app: &AppHandle,
    level: &str,
    phase: &str,
    message: &str,
    details: Option<serde_json::Value>,
) {
    let Ok(repository) = log_repository(app) else {
        return;
    };
    if !repository.is_verbose() {
        return;
    }
    let _ = repository.append(
        level,
        "bilibili_workflow",
        message,
        details.map(|mut value| {
            if let Some(object) = value.as_object_mut() {
                object.insert("phase".into(), serde_json::Value::String(phase.into()));
            }
            value
        }),
    );
}

fn diagnostics_settings_repository(
    app: &AppHandle,
) -> Result<DiagnosticsSettingsRepository, String> {
    let directory = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("无法定位应用配置目录：{error}"))?;
    Ok(DiagnosticsSettingsRepository::new(
        directory.join("diagnostics-settings.json"),
    ))
}

pub fn chat_repository(app: &AppHandle) -> Result<ChatRepository, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("无法定位应用数据目录：{error}"))?;
    Ok(ChatRepository::new(directory.join("paimon.db")))
}

fn resource_directory(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|error| format!("无法定位应用资源目录：{error}"))?;
    Ok(resolve_resource_dir(resource_dir))
}

fn loaded_resources(app: &AppHandle) -> Result<Vec<PromptFragment>, String> {
    load_fragments(&resource_directory(app)?)
}

#[tauri::command]
pub fn get_prompt_settings(app: AppHandle) -> Result<PromptSettings, String> {
    settings_repository(&app)?.load()
}

#[tauri::command]
pub fn save_prompt_settings(
    app: AppHandle,
    settings: PromptSettings,
) -> Result<ComposedPrompt, String> {
    let resources = loaded_resources(&app)?;
    let composed = compose_prompt(&resources, &settings)?;
    settings_repository(&app)?.save(&settings)?;
    Ok(composed)
}

#[tauri::command]
pub fn reset_prompt_settings(app: AppHandle) -> Result<PromptSettings, String> {
    settings_repository(&app)?.reset()?;
    Ok(PromptSettings::default())
}

#[tauri::command]
pub fn list_prompt_resources(app: AppHandle) -> Result<Vec<PromptFragment>, String> {
    loaded_resources(&app)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    pub config: ProviderConfig,
    pub has_api_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProviderInput {
    pub config: ProviderConfig,
    pub api_key: Option<String>,
}

fn provider_view(config: ProviderConfig) -> ProviderView {
    let has_api_key = load_secret(&config.secret_ref).is_ok();
    ProviderView {
        config,
        has_api_key,
    }
}

fn provider_api_key(input: &SaveProviderInput) -> Result<String, String> {
    let api_key = input
        .api_key
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .map(str::to_owned)
        .or_else(|| load_secret(&input.config.secret_ref).ok())
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err("请填写 API Key，或先保存包含 API Key 的配置".into());
    }
    Ok(api_key)
}

fn redact_known_secret(value: &str, secret: &str) -> String {
    let value = crate::diagnostics::redact_sensitive_text(value);
    if secret.trim().is_empty() {
        value
    } else {
        value.replace(secret, "[REDACTED]")
    }
}

fn redact_capability_report(report: &mut CapabilityReport, api_key: &str) {
    report.basic_request.message = redact_known_secret(&report.basic_request.message, api_key);
    if let Some(diagnostic) = report.basic_request.diagnostic.as_mut() {
        diagnostic.endpoint = redact_known_secret(&diagnostic.endpoint, api_key);
        diagnostic.response_body = redact_known_secret(&diagnostic.response_body, api_key);
        diagnostic.request_body = redact_known_secret(&diagnostic.request_body, api_key);
        diagnostic.parse_error = diagnostic
            .parse_error
            .as_deref()
            .map(|error| redact_known_secret(error, api_key));
    }
}

#[tauri::command]
pub fn list_providers(app: AppHandle) -> Result<Vec<ProviderView>, String> {
    Ok(provider_repository(&app)?
        .list()?
        .into_iter()
        .map(provider_view)
        .collect())
}

#[tauri::command]
pub fn save_provider(app: AppHandle, input: SaveProviderInput) -> Result<ProviderView, String> {
    let mut config = input.config;
    if config.secret_ref.trim().is_empty() {
        config.secret_ref = format!("provider:{}", config.id);
    }
    if let Some(api_key) = input.api_key.filter(|key| !key.trim().is_empty()) {
        save_secret(&config.secret_ref, &api_key)?;
    }
    let saved = provider_repository(&app)?.upsert(config)?;
    Ok(provider_view(saved))
}

#[tauri::command]
pub fn delete_provider(app: AppHandle, id: String) -> Result<(), String> {
    let providers = provider_repository(&app)?.list()?;
    if let Some(config) = providers.into_iter().find(|provider| provider.id == id) {
        delete_secret(&config.secret_ref)?;
    }
    provider_repository(&app)?.delete(&id)
}

#[tauri::command]
pub async fn test_provider(
    app: AppHandle,
    input: SaveProviderInput,
) -> Result<CapabilityReport, String> {
    let api_key = provider_api_key(&input)?;
    let config = input.config;
    let client = OpenAiCompatibleClient::new()?;
    let mut report = client.test_capabilities(&config, &api_key).await;
    redact_capability_report(&mut report, &api_key);
    let level = if report.basic_request.supported {
        "info"
    } else {
        "error"
    };
    let message = if report.basic_request.supported {
        format!("模型连接测试完成：{}", config.name)
    } else {
        format!("模型连接测试失败：{}", report.basic_request.message)
    };
    let _ = log_repository(&app).and_then(|repository| {
        repository
            .append(
                level,
                "model_test",
                message,
                Some(serde_json::json!({
                    "provider": {
                        "name": &config.name,
                        "baseUrl": &config.base_url,
                        "model": &config.model,
                        "timeoutMs": config.timeout_ms,
                        "streaming": config.streaming,
                        "toolCalling": config.tool_calling
                    },
                    "report": &report
                })),
            )
            .map(|_| ())
    });
    Ok(report)
}

#[tauri::command]
pub async fn list_provider_models(input: SaveProviderInput) -> Result<Vec<String>, String> {
    let api_key = provider_api_key(&input)?;
    OpenAiCompatibleClient::new()?
        .list_models(&input.config, &api_key)
        .await
}

#[tauri::command]
pub fn list_logs(app: AppHandle) -> Result<Vec<LogEntry>, String> {
    log_repository(&app)?.list()
}

#[tauri::command]
pub fn get_diagnostics_settings(app: AppHandle) -> Result<DiagnosticsSettings, String> {
    diagnostics_settings_repository(&app)?.load()
}

#[tauri::command]
pub fn set_diagnostics_settings(
    app: AppHandle,
    settings: DiagnosticsSettings,
) -> Result<DiagnosticsSettings, String> {
    diagnostics_settings_repository(&app)?.save(&settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn clear_logs(app: AppHandle) -> Result<(), String> {
    log_repository(&app)?.clear()
}

#[tauri::command]
pub fn export_logs(app: AppHandle, ids: Option<Vec<String>>) -> Result<String, String> {
    let entries = log_repository(&app)?.list()?;
    let entries = if let Some(ids) = ids {
        let ids = ids.into_iter().collect::<HashSet<_>>();
        entries
            .into_iter()
            .filter(|entry| ids.contains(&entry.id))
            .collect()
    } else {
        entries
    };
    Ok(export_markdown(&entries))
}

#[tauri::command]
pub fn list_conversations(app: AppHandle) -> Result<Vec<Conversation>, String> {
    chat_repository(&app)?.list_conversations()
}

#[tauri::command]
pub fn list_messages(app: AppHandle, conversation_id: String) -> Result<Vec<Message>, String> {
    chat_repository(&app)?.list_messages(&conversation_id)
}

#[tauri::command]
pub fn list_tool_runs(
    app: AppHandle,
    conversation_id: String,
) -> Result<Vec<ToolRunRecord>, String> {
    chat_repository(&app)?.list_tool_runs(&conversation_id)
}

#[tauri::command]
pub fn list_sources(
    app: AppHandle,
    conversation_id: String,
) -> Result<Vec<StoredSourceRecord>, String> {
    chat_repository(&app)?.list_sources(&conversation_id)
}

#[tauri::command]
pub fn rename_conversation(app: AppHandle, id: String, title: String) -> Result<(), String> {
    chat_repository(&app)?.rename_conversation(&id, &title)
}

#[tauri::command]
pub async fn delete_conversation(
    app: AppHandle,
    state: State<'_, RunState>,
    id: String,
) -> Result<(), String> {
    state.cancel_conversation(&id);
    if !state
        .wait_for_conversation(&id, std::time::Duration::from_secs(5))
        .await
    {
        return Err("对话仍在生成，请稍候再试".into());
    }
    chat_repository(&app)?.delete_conversation(&id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageInput {
    pub conversation_id: Option<String>,
    pub provider_id: String,
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default)]
    pub retry_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    pub run_id: String,
    pub conversation: Conversation,
    pub user_message: Message,
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, RunState>,
    input: SendMessageInput,
) -> Result<SendMessageResult, String> {
    let content = input.content.trim();
    if input.retry_message_id.is_some() && input.conversation_id.is_none() {
        return Err("重试消息必须属于已有对话".into());
    }
    if input.retry_message_id.is_none() {
        validate_attachments(&input.attachments)?;
        if content.is_empty() && input.attachments.is_empty() {
            return Err("消息不能为空".into());
        }
    }

    let provider = provider_repository(&app)?
        .list()?
        .into_iter()
        .find(|provider| provider.id == input.provider_id)
        .ok_or_else(|| "所选模型配置不存在".to_string())?;
    let api_key = load_secret(&provider.secret_ref).unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err("所选模型缺少 API Key，请先在设置中保存".into());
    }
    let prompt_settings = settings_repository(&app)?.load()?;
    let prompt = compose_prompt(&loaded_resources(&app)?, &prompt_settings)?;

    let repository = chat_repository(&app)?;
    let mut conversation = match input.conversation_id.clone() {
        Some(id) => repository
            .list_conversations()?
            .into_iter()
            .find(|conversation| conversation.id == id)
            .ok_or_else(|| "对话不存在".to_string())?,
        None => repository.create_conversation(
            &input.provider_id,
            &if content.is_empty() {
                input
                    .attachments
                    .first()
                    .map(|attachment| attachment.name.clone())
                    .unwrap_or_else(|| "新对话".into())
            } else {
                content.chars().take(24).collect::<String>()
            },
        )?,
    };
    let switching_provider = conversation.provider_id != input.provider_id;
    if switching_provider && input.retry_message_id.is_none() {
        return Err(
            "该对话绑定的模型与当前选择不一致；如需换模型，请在失败消息中选择模型后重试".into(),
        );
    }
    let user_message = if let Some(retry_message_id) = input.retry_message_id.as_deref() {
        let history = repository.list_messages(&conversation.id)?;
        let existing = history
            .last()
            .filter(|message| message.id == retry_message_id && message.role == "user")
            .cloned()
            .ok_or_else(|| "只能重试当前对话最后一条失败的用户消息".to_string())?;
        validate_attachments(&existing.attachments)?;
        existing
    } else {
        repository.add_message_with_attachments(
            &conversation.id,
            "user",
            content,
            &input.attachments,
        )?
    };
    if switching_provider {
        conversation =
            repository.update_conversation_provider(&conversation.id, &input.provider_id)?;
    }

    let history = repository.list_messages(&conversation.id)?;
    let messages = build_messages(&prompt.text, &history);

    let run_id = Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    state.insert(run_id.clone(), conversation.id.clone(), token.clone());

    let task_app = app.clone();
    let task_run_id = run_id.clone();
    let task_conversation_id = conversation.id.clone();
    let task_provider = provider.clone();
    let task_api_key = api_key.clone();
    let research_query = user_message.content.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_chat(
            &task_app,
            &task_run_id,
            &task_conversation_id,
            provider,
            task_api_key,
            research_query,
            messages,
            token,
        )
        .await;
        if let Err(error) = result {
            let error = redact_known_secret(&error, &api_key);
            let _ = log_repository(&task_app).and_then(|repository| {
                repository
                    .append(
                        "error",
                        "chat",
                        &error,
                        Some(serde_json::json!({
                            "runId": &task_run_id,
                            "conversationId": &task_conversation_id,
                            "provider": {
                                "name": &task_provider.name,
                                "baseUrl": &task_provider.base_url,
                                "model": &task_provider.model,
                            }
                        })),
                    )
                    .map(|_| ())
            });
            let _ = task_app.emit(
                "chat-event",
                ChatEvent::failed(&task_run_id, &task_conversation_id, error),
            );
        }
        task_app.state::<RunState>().remove(&task_run_id);
    });

    Ok(SendMessageResult {
        run_id,
        conversation,
        user_message,
    })
}

async fn run_chat(
    app: &AppHandle,
    run_id: &str,
    conversation_id: &str,
    provider: ProviderConfig,
    api_key: String,
    research_query: String,
    messages: Vec<crate::providers::models::ChatMessage>,
    token: CancellationToken,
) -> Result<(), String> {
    struct RegistryTools {
        client: reqwest::Client,
        app: AppHandle,
        visual: Arc<dyn VisualTaskExecutor>,
    }

    #[async_trait::async_trait]
    impl ToolBackend for RegistryTools {
        async fn execute(&self, name: &str, arguments: &str) -> Result<ToolResult, String> {
            if crate::browser::agent_tools::is_supported(name) {
                crate::browser::agent_tools::execute(&self.app, name, arguments).await
            } else if crate::research::agent_tools::is_supported(name) {
                crate::research::agent_tools::execute_with_visual(
                    &self.app,
                    name,
                    arguments,
                    Some(self.visual.as_ref()),
                )
                .await
            } else {
                crate::tools::registry::execute(&self.client, name, arguments).await
            }
        }
    }

    struct TauriEvents<'a> {
        app: &'a AppHandle,
        run_id: &'a str,
        conversation_id: &'a str,
        provider_name: &'a str,
        provider_base_url: &'a str,
        provider_model: &'a str,
        model_request_started_at: Mutex<Option<Instant>>,
        tool_started_at: Mutex<HashMap<String, Instant>>,
    }

    impl AgentEvents for TauriEvents<'_> {
        fn model_request_started(
            &self,
            attempt: u8,
            messages: &[crate::providers::models::ChatMessage],
            tools: &[crate::providers::models::ToolDefinition],
        ) {
            if let Ok(mut started) = self.model_request_started_at.lock() {
                *started = Some(Instant::now());
            }
            let Ok(repository) = log_repository(self.app) else {
                return;
            };
            if !repository.is_verbose() {
                return;
            }
            let _ = repository.append(
                "info",
                "model_api",
                format!("模型 API 请求开始（第 {attempt} 次尝试）"),
                Some(serde_json::json!({
                    "runId": self.run_id,
                    "conversationId": self.conversation_id,
                    "attempt": attempt,
                    "provider": {
                        "name": self.provider_name,
                        "baseUrl": self.provider_base_url,
                        "model": self.provider_model,
                    },
                    "request": {
                        "messageCount": messages.len(),
                        "messages": messages,
                        "toolCount": tools.len(),
                        "tools": tools,
                    }
                })),
            );
        }

        fn model_request_finished(
            &self,
            attempt: u8,
            turn: Option<&crate::agent::runtime::ModelTurn>,
            error: Option<&str>,
        ) {
            let elapsed_ms = self
                .model_request_started_at
                .lock()
                .ok()
                .and_then(|mut started| started.take())
                .map(|started| started.elapsed().as_millis());
            let Ok(repository) = log_repository(self.app) else {
                return;
            };
            if !repository.is_verbose() {
                return;
            }
            let _ = repository.append(
                if error.is_some() { "error" } else { "info" },
                "model_api",
                if let Some(error) = error {
                    format!("模型 API 请求失败（第 {attempt} 次尝试）：{error}")
                } else {
                    format!("模型 API 响应完成（第 {attempt} 次尝试）")
                },
                Some(serde_json::json!({
                    "runId": self.run_id,
                    "conversationId": self.conversation_id,
                    "attempt": attempt,
                    "provider": {
                        "name": self.provider_name,
                        "baseUrl": self.provider_base_url,
                        "model": self.provider_model,
                    },
                    "elapsedMs": elapsed_ms,
                    "response": turn.map(|turn| serde_json::json!({
                        "content": turn.content,
                        "toolCalls": turn.tool_calls,
                    })),
                    "error": error,
                })),
            );
        }

        fn tool_started(&self, call: &crate::providers::models::ToolCall) {
            if let Ok(mut started) = self.tool_started_at.lock() {
                started.insert(call.id.clone(), Instant::now());
            }
            let _ = self.app.emit(
                "chat-tool",
                serde_json::json!({
                    "conversationId": self.conversation_id,
                    "runId": self.run_id,
                    "callId": call.id,
                    "toolName": call.name,
                    "status": "running",
                    "createdAt": chrono::Utc::now().to_rfc3339()
                }),
            );
            let _ = log_repository(self.app).and_then(|repository| {
                let verbose = repository.is_verbose();
                repository
                    .append(
                        "info",
                        "tool",
                        format!("工具开始：{}", call.name),
                        Some(serde_json::json!({
                            "runId": self.run_id,
                            "conversationId": self.conversation_id,
                            "callId": call.id,
                            "toolName": call.name,
                            "argumentChars": call.arguments.chars().count(),
                            "arguments": verbose.then_some(call.arguments.clone()),
                        })),
                    )
                    .map(|_| ())
            });
        }

        fn tool_finished(&self, outcome: &crate::agent::runtime::ToolRunOutcome) {
            let elapsed_ms = self
                .tool_started_at
                .lock()
                .ok()
                .and_then(|mut started| started.remove(&outcome.call_id))
                .map(|started| started.elapsed().as_millis());
            let source = outcome
                .result
                .as_ref()
                .and_then(|result| result.source.as_ref());
            let details = outcome
                .result
                .as_ref()
                .map(|result| result.content.as_str())
                .or(outcome.error.as_deref())
                .map(|value| value.chars().take(2_000).collect::<String>());
            let _ = self.app.emit(
                "chat-tool",
                serde_json::json!({
                    "conversationId": self.conversation_id,
                    "runId": self.run_id,
                    "callId": outcome.call_id,
                    "toolName": outcome.tool_name,
                    "status": outcome.status,
                    "details": details,
                    "url": source.map(|value| value.url.as_str()),
                    "platform": source.map(|value| value.platform.as_str()),
                    "createdAt": chrono::Utc::now().to_rfc3339()
                }),
            );
            let _ = log_repository(self.app).and_then(|repository| {
                let verbose = repository.is_verbose();
                repository
                    .append(
                        if outcome.status == "completed" { "info" } else { "error" },
                        "tool",
                        format!("工具{}：{}", if outcome.status == "completed" { "完成" } else { "失败" }, outcome.tool_name),
                        Some(serde_json::json!({
                            "runId": self.run_id,
                            "conversationId": self.conversation_id,
                            "callId": outcome.call_id,
                            "toolName": outcome.tool_name,
                            "status": outcome.status,
                            "elapsedMs": elapsed_ms,
                            "arguments": verbose.then_some(outcome.arguments.clone()),
                            "resultChars": outcome.result.as_ref().map(|result| result.content.chars().count()),
                            "sourcePlatform": source.map(|value| value.platform.as_str()),
                            "error": outcome.error,
                            "result": verbose.then(|| outcome.result.as_ref().map(|result| serde_json::json!({
                                "content": result.content,
                                "source": result.source,
                                "links": result.links,
                                "metadata": result.metadata,
                            }))),
                        })),
                    )
                    .map(|_| ())
            });
        }

        fn source_added(&self, source: &SourceRecord) {
            let _ = self.app.emit(
                "chat-source",
                serde_json::json!({
                    "conversationId": self.conversation_id,
                    "runId": self.run_id,
                    "source": source
                }),
            );
        }

        fn model_retrying(&self, attempt: u8, error: &str) {
            self.model_retrying_with_diagnostics(attempt, error, &[]);
        }

        fn model_retrying_with_diagnostics(
            &self,
            attempt: u8,
            error: &str,
            diagnostics: &[crate::providers::models::CapabilityDiagnostic],
        ) {
            let _ = self.app.emit(
                "chat-event",
                ChatEvent::retrying(
                    self.run_id,
                    self.conversation_id,
                    attempt,
                    error.to_string(),
                ),
            );
            let _ = log_repository(self.app).and_then(|repository| {
                repository
                    .append(
                        "warn",
                        "model_retry",
                        format!("模型请求失败，开始第 {attempt} 次尝试"),
                        Some(serde_json::json!({
                            "runId": self.run_id,
                            "conversationId": self.conversation_id,
                            "attempt": attempt,
                            "error": error,
                            "diagnostics": diagnostics,
                        })),
                    )
                    .map(|_| ())
            });
        }

        fn tool_recorded(
            &self,
            outcome: &crate::agent::runtime::ToolRunOutcome,
        ) -> Result<(), String> {
            chat_repository(self.app)?.record_tool_run(ToolRunInput {
                conversation_id: self.conversation_id,
                run_id: self.run_id,
                call_id: &outcome.call_id,
                tool_name: &outcome.tool_name,
                arguments: &outcome.arguments,
                status: &outcome.status,
                result: outcome.result.as_ref(),
                error: outcome.error.as_deref(),
            })?;
            Ok(())
        }
    }

    let tool_definitions = if provider.tool_calling {
        crate::tools::registry::definitions()
    } else {
        Vec::new()
    };
    let visual: Arc<dyn VisualTaskExecutor> =
        match OpenAiCompatibleVisualTaskExecutor::new(provider.clone(), api_key.clone()) {
            Ok(executor) => Arc::new(executor),
            Err(_) => Arc::new(UnavailableVisualTaskExecutor),
        };
    let backend = OpenAiModelBackend::new(provider.clone(), api_key.clone())?;
    let tools = RegistryTools {
        client: reqwest::Client::new(),
        app: app.clone(),
        visual,
    };
    let events = TauriEvents {
        app,
        run_id,
        conversation_id,
        provider_name: &provider.name,
        provider_base_url: &provider.base_url,
        provider_model: &provider.model,
        model_request_started_at: Mutex::new(None),
        tool_started_at: Mutex::new(HashMap::new()),
    };
    let query_chars = research_query.chars().count();
    let mut research = crate::agent::research::ResearchLedger::new(&research_query);
    let _ = log_repository(app).and_then(|repository| {
        repository
            .append(
                "info",
                "chat_start",
                "开始模型研究任务",
                Some(serde_json::json!({
                    "runId": run_id,
                    "conversationId": conversation_id,
                    "provider": {
                        "name": &provider.name,
                        "baseUrl": &provider.base_url,
                        "model": &provider.model,
                        "timeoutMs": provider.timeout_ms,
                        "streaming": provider.streaming,
                        "toolCalling": provider.tool_calling,
                    },
                    "context": {
                        "messageCount": messages.len(),
                        "toolDefinitionCount": tool_definitions.len(),
                        "queryChars": query_chars,
                    }
                })),
            )
            .map(|_| ())
    });
    let result = run_agent_loop_with_research(
        &backend,
        &tools,
        &events,
        messages,
        tool_definitions,
        AgentLoopOptions::default(),
        token,
        |delta| {
            let _ = app.emit(
                "chat-event",
                ChatEvent::delta(run_id, conversation_id, delta),
            );
        },
        Some(&mut research),
    )
    .await;
    for mut diagnostic in backend.take_diagnostics() {
        diagnostic.endpoint = redact_known_secret(&diagnostic.endpoint, &api_key);
        diagnostic.response_body = redact_known_secret(&diagnostic.response_body, &api_key);
        diagnostic.request_body = redact_known_secret(&diagnostic.request_body, &api_key);
        diagnostic.response_headers = diagnostic
            .response_headers
            .into_iter()
            .map(|header| redact_known_secret(&header, &api_key))
            .collect();
        diagnostic.parse_error = diagnostic
            .parse_error
            .take()
            .map(|error| redact_known_secret(&error, &api_key));
        let _ = log_repository(app).and_then(|repository| {
            repository
                .append(
                    "error",
                    "model_request",
                    format!("模型请求失败：{}", diagnostic.parse_error.as_deref().unwrap_or("未提供错误详情")),
                    Some(serde_json::json!({
                        "runId": run_id,
                        "conversationId": conversation_id,
                        "provider": { "name": &provider.name, "baseUrl": &provider.base_url, "model": &provider.model },
                        "diagnostic": diagnostic,
                    })),
                )
                .map(|_| ())
        });
    }
    let result = match result {
        Ok(result) => result,
        Err(error) if error == AGENT_CANCELLED => {
            close_temporary_browser_tabs(app);
            let _ = app.emit("chat-event", ChatEvent::cancelled(run_id, conversation_id));
            return Ok(());
        }
        Err(error) => {
            close_temporary_browser_tabs(app);
            return Err(error);
        }
    };
    close_temporary_browser_tabs(app);

    let result_text_chars = result.content.chars().count();
    let result_segment_count = result.segments.len();
    let result_tool_count = result.tool_runs.len();

    let message = chat_repository(app)?.add_message_with_segments(
        conversation_id,
        "assistant",
        &result.content,
        &[],
        Some(run_id),
        &result.segments,
    )?;
    let _ = app.emit(
        "chat-event",
        ChatEvent::finished(
            run_id,
            conversation_id,
            message.id,
            result.content.clone(),
            result.segments.clone(),
        ),
    );
    let _ = log_repository(app).and_then(|repository| {
        repository
            .append(
                "info",
                "chat_complete",
                "模型研究任务完成",
                Some(serde_json::json!({
                    "runId": run_id,
                    "conversationId": conversation_id,
                    "provider": { "name": &provider.name, "model": &provider.model },
                    "result": {
                        "textChars": result_text_chars,
                        "segmentCount": result_segment_count,
                        "toolRunCount": result_tool_count,
                    }
                })),
            )
            .map(|_| ())
    });
    Ok(())
}

fn close_temporary_browser_tabs(app: &AppHandle) {
    let state = app.state::<crate::browser::state::BrowserState>();
    for id in state.temporary_ids() {
        if let Some(window) = app.get_webview_window(&id) {
            let _ = window.close();
        }
        let _ = state.remove(&id);
    }
}

#[tauri::command]
pub fn cancel_run(state: State<'_, RunState>, run_id: String) -> bool {
    state.cancel(&run_id)
}
