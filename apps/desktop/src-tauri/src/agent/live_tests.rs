use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::{
    openai_backend::OpenAiModelBackend,
    research::ResearchLedger,
    runtime::{run_agent_loop_with_research, AgentEvents, AgentLoopOptions, ToolBackend},
};
use crate::{
    prompt::{composer::compose_prompt, models::PromptSettings, resources::load_fragments},
    providers::{models::ChatMessage, repository::ProviderRepository, secrets::load_secret},
    tools::registry::{self, ToolResult},
};

struct FactTools(reqwest::Client);

#[async_trait]
impl ToolBackend for FactTools {
    async fn execute(&self, name: &str, arguments: &str) -> Result<ToolResult, String> {
        registry::execute(&self.0, name, arguments).await
    }
}

struct QuietEvents;
impl AgentEvents for QuietEvents {}

// Explicitly opt in: this uses the configured endpoint and Windows credential
// without logging the key, changing settings, or writing chat history.
#[tokio::test]
#[ignore = "requires PAIMON_LIVE_TEST_CONFIG_DIR and a configured online model"]
async fn desktop_skill_live_regression() {
    let config_dir = PathBuf::from(
        std::env::var("PAIMON_LIVE_TEST_CONFIG_DIR").expect("set the live-test config directory"),
    );
    let provider = ProviderRepository::new(config_dir.join("providers.json"))
        .list()
        .unwrap()
        .into_iter()
        .find(|provider| provider.enabled && provider.is_default)
        .expect("an enabled default model is required");
    assert!(
        provider.tool_calling,
        "enable tool calling for the live test"
    );
    let api_key = load_secret(&provider.secret_ref).expect("saved model credential is required");
    let backend = OpenAiModelBackend::new(provider, api_key).unwrap();
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../resources/prompts");
    let fragments = load_fragments(&resources).unwrap();
    let prompt = compose_prompt(&fragments, &PromptSettings::default()).unwrap();
    let tools = FactTools(reqwest::Client::new());
    let definitions = registry::definitions()
        .into_iter()
        .filter(|tool| {
            matches!(
                tool.function.name.as_str(),
                "search_bwiki" | "read_bwiki_page" | "read_url"
            )
        })
        .collect::<Vec<_>>();

    for (query, expects_wiki) in [
        (
            "胡桃的元素类型是什么？请读取 BWiki 的具体角色条目后再回答。",
            true,
        ),
        ("只把 hello 翻译成中文，不需要查询网页。", false),
    ] {
        let mut research = ResearchLedger::new(query);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(90),
            run_agent_loop_with_research(
                &backend,
                &tools,
                &QuietEvents,
                vec![ChatMessage::system(&prompt.text), ChatMessage::user(query)],
                definitions.clone(),
                AgentLoopOptions { max_tool_rounds: 4 },
                CancellationToken::new(),
                |_| {},
                Some(&mut research),
            ),
        )
        .await
        .expect("live regression timed out")
        .unwrap_or_else(|_| {
            panic!("live model or tool request failed; credentials were not logged")
        });

        if expects_wiki {
            assert!(result
                .tool_runs
                .iter()
                .any(|run| run.tool_name == "read_bwiki_page" && run.status == "completed"));
            assert!(result.content.contains('火'));
            assert!(result.content.contains("wiki.biligame.com"));
            assert!(!result.content.contains("安装 OpenCLI"));
        } else {
            assert!(result.tool_runs.is_empty());
            assert!(result.content.contains("你好"));
        }
        println!(
            "fresh conversation: wiki={expects_wiki}, completed tools={}",
            result.tool_runs.len()
        );
    }
}
