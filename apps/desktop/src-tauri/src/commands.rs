use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agent::{
    context::build_messages,
    events::ChatEvent,
    openai_backend::OpenAiModelBackend,
    runtime::{run_agent_loop, AgentEvents, AgentLoopOptions, ToolBackend, AGENT_CANCELLED},
    state::RunState,
};

use crate::prompt::{
    composer::compose_prompt,
    models::{ComposedPrompt, PromptFragment, PromptSettings},
    repository::PromptRepository,
    resources::{default_resource_dir, load_fragments},
};
use crate::providers::{
    models::{CapabilityReport, ProviderConfig},
    openai_compatible::OpenAiCompatibleClient,
    repository::ProviderRepository,
    secrets::{delete_secret, load_secret, save_secret},
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
    Ok(default_resource_dir(Some(
        resource_dir.join("resources/prompts"),
    )))
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
    _app: AppHandle,
    input: SaveProviderInput,
) -> Result<CapabilityReport, String> {
    let config = input.config;
    let api_key = match input.api_key.filter(|key| !key.trim().is_empty()) {
        Some(api_key) => api_key,
        None => load_secret(&config.secret_ref).unwrap_or_default(),
    };
    let client = OpenAiCompatibleClient::new()?;
    Ok(client.test_capabilities(&config, &api_key).await)
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
pub fn delete_conversation(app: AppHandle, id: String) -> Result<(), String> {
    chat_repository(&app)?.delete_conversation(&id)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageInput {
    pub conversation_id: Option<String>,
    pub provider_id: String,
    pub content: String,
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
    if content.is_empty() {
        return Err("消息不能为空".into());
    }

    let repository = chat_repository(&app)?;
    let conversation = match input.conversation_id {
        Some(id) => repository
            .list_conversations()?
            .into_iter()
            .find(|conversation| conversation.id == id)
            .ok_or_else(|| "对话不存在".to_string())?,
        None => repository.create_conversation(
            &input.provider_id,
            &content.chars().take(24).collect::<String>(),
        )?,
    };
    let user_message = repository.add_message(&conversation.id, "user", content)?;

    let provider = provider_repository(&app)?
        .list()?
        .into_iter()
        .find(|provider| provider.id == input.provider_id)
        .ok_or_else(|| "所选模型配置不存在".to_string())?;
    let api_key = load_secret(&provider.secret_ref).unwrap_or_default();
    let prompt_settings = settings_repository(&app)?.load()?;
    let prompt = compose_prompt(&loaded_resources(&app)?, &prompt_settings)?;
    let history = repository.list_messages(&conversation.id)?;
    let messages = build_messages(&prompt.text, &history);

    let run_id = Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    state.insert(run_id.clone(), token.clone());

    let task_app = app.clone();
    let task_run_id = run_id.clone();
    let task_conversation_id = conversation.id.clone();
    tauri::async_runtime::spawn(async move {
        let result = run_chat(
            &task_app,
            &task_run_id,
            &task_conversation_id,
            provider,
            api_key,
            messages,
            token,
        )
        .await;
        if let Err(error) = result {
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
    messages: Vec<crate::providers::models::ChatMessage>,
    token: CancellationToken,
) -> Result<(), String> {
    struct RegistryTools(reqwest::Client);

    #[async_trait::async_trait]
    impl ToolBackend for RegistryTools {
        async fn execute(&self, name: &str, arguments: &str) -> Result<ToolResult, String> {
            crate::tools::registry::execute(&self.0, name, arguments).await
        }
    }

    struct TauriEvents<'a> {
        app: &'a AppHandle,
        run_id: &'a str,
        conversation_id: &'a str,
    }

    impl AgentEvents for TauriEvents<'_> {
        fn tool_started(&self, name: &str) {
            let _ = self.app.emit(
                "chat-tool",
                serde_json::json!({"runId": self.run_id, "toolName": name, "status": "running"}),
            );
        }

        fn tool_finished(&self, name: &str, success: bool) {
            let _ = self.app.emit(
                "chat-tool",
                serde_json::json!({
                    "runId": self.run_id,
                    "toolName": name,
                    "status": if success { "completed" } else { "failed" }
                }),
            );
        }

        fn source_added(&self, source: &SourceRecord) {
            let _ = self.app.emit("chat-source", source);
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
    let backend = OpenAiModelBackend::new(provider, api_key)?;
    let tools = RegistryTools(reqwest::Client::new());
    let events = TauriEvents {
        app,
        run_id,
        conversation_id,
    };
    let result = run_agent_loop(
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
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(error) if error == AGENT_CANCELLED => {
            let _ = app.emit("chat-event", ChatEvent::cancelled(run_id, conversation_id));
            return Ok(());
        }
        Err(error) => return Err(error),
    };

    let message =
        chat_repository(app)?.add_message(conversation_id, "assistant", &result.content)?;
    let _ = app.emit(
        "chat-event",
        ChatEvent::finished(run_id, conversation_id, message.id, result.content),
    );
    Ok(())
}

#[tauri::command]
pub fn cancel_run(state: State<'_, RunState>, run_id: String) -> bool {
    state.cancel(&run_id)
}
