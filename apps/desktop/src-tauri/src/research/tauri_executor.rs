use async_trait::async_trait;
use serde_json::json;
use std::{collections::HashSet, sync::Mutex};
use tauri::{AppHandle, Manager};

use super::{
    executor::ResearchToolExecutor,
    models::{
        ResearchAction, ResearchActionRequest, ResearchArtifact, ResearchArtifactKind,
        ResearchError,
    },
};

pub struct TauriBrowserExecutor<'a> {
    app: &'a AppHandle,
    opened_tab_id: Mutex<Option<String>>,
    initial_temporary_ids: HashSet<String>,
}

impl<'a> TauriBrowserExecutor<'a> {
    pub fn new(app: &'a AppHandle) -> Self {
        Self {
            app,
            opened_tab_id: Mutex::new(None),
            initial_temporary_ids: app
                .state::<crate::browser::state::BrowserState>()
                .temporary_ids()
                .into_iter()
                .collect(),
        }
    }
}

#[async_trait]
impl ResearchToolExecutor for TauriBrowserExecutor<'_> {
    async fn execute(
        &self,
        request: ResearchActionRequest,
    ) -> Result<Option<ResearchArtifact>, ResearchError> {
        let action = request.action;
        let (tool_name, arguments) = match action {
            ResearchAction::OpenPage => (
                "browser_open_page",
                json!({"url": request.url.ok_or_else(|| ResearchError::new("invalid_action", "打开网页缺少 URL"))?}),
            ),
            ResearchAction::ExtractPage => {
                ("browser_extract_page", json!({"tabId": request.tab_id}))
            }
            ResearchAction::FindContext => (
                "browser_find_context",
                json!({"tabId": request.tab_id, "query": request.query.ok_or_else(|| ResearchError::new("invalid_action", "查找上下文缺少 query"))?}),
            ),
            ResearchAction::ClickElement => (
                "browser_click_element",
                json!({"tabId": request.tab_id, "elementId": request.element_id.ok_or_else(|| ResearchError::new("invalid_action", "点击元素缺少 elementId"))?}),
            ),
            ResearchAction::ScrollPage => (
                "browser_scroll_page",
                json!({"tabId": request.tab_id, "maxRounds": request.max_scroll_rounds.max(1)}),
            ),
            ResearchAction::ExtractComments => {
                ("browser_extract_comments", json!({"tabId": request.tab_id}))
            }
            ResearchAction::ExtractBilibiliSubtitles => (
                "browser_extract_bilibili_subtitles",
                json!({"tabId": request.tab_id, "allParts": request.all_parts}),
            ),
            ResearchAction::ExtractBilibiliKeyframes => (
                "browser_extract_bilibili_keyframes",
                json!({"tabId": request.tab_id, "timestampsMs": request.timestamps_ms}),
            ),
            ResearchAction::ClosePage => ("browser_close_page", json!({"tabId": request.tab_id})),
        };

        let result = crate::browser::agent_tools::execute(
            self.app,
            tool_name,
            &serde_json::to_string(&arguments)
                .map_err(|error| ResearchError::new("serialize_action", error.to_string()))?,
        )
        .await
        .map_err(|error| ResearchError::new("browser_tool", error))?;

        let tab_id = result
            .metadata
            .get("tabId")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        if action == ResearchAction::OpenPage {
            let opened_tab_id = tab_id.clone().ok_or_else(|| {
                ResearchError::new("missing_tab_id", "浏览器工具没有返回研究标签 ID")
            })?;
            *self
                .opened_tab_id
                .lock()
                .expect("research tab state poisoned") = Some(opened_tab_id);
        }
        if action == ResearchAction::ClosePage {
            *self
                .opened_tab_id
                .lock()
                .expect("research tab state poisoned") = None;
            return Ok(None);
        }
        if result.content.contains("检测到登录或验证码") {
            return Err(ResearchError::new(
                "blocked",
                "网页被登录、验证码或风控阻断",
            ));
        }
        let source = result
            .source
            .ok_or_else(|| ResearchError::new("missing_source", "浏览器工具没有返回来源"))?;
        let kind = match action {
            ResearchAction::ExtractComments => ResearchArtifactKind::Comments,
            ResearchAction::ExtractBilibiliSubtitles => ResearchArtifactKind::Transcript,
            ResearchAction::ExtractBilibiliKeyframes => ResearchArtifactKind::Visual,
            _ => ResearchArtifactKind::Body,
        };
        let mut metadata = std::collections::BTreeMap::new();
        if let Some(tab_id) = tab_id {
            metadata.insert("tabId".into(), tab_id);
        }
        Ok(Some(ResearchArtifact {
            kind,
            title: source.title,
            url: source.url,
            content: result.content,
            source_level: source.source_level,
            captured_at: source.captured_at,
            metadata,
        }))
    }

    async fn cleanup(&self) -> Result<(), ResearchError> {
        let tracked_tab_id = self
            .opened_tab_id
            .lock()
            .expect("research tab state poisoned")
            .take();
        let current = self
            .app
            .state::<crate::browser::state::BrowserState>()
            .temporary_ids();
        let mut candidates = current
            .into_iter()
            .filter(|id| !self.initial_temporary_ids.contains(id))
            .collect::<Vec<_>>();
        if let Some(tab_id) = tracked_tab_id {
            if !candidates.contains(&tab_id) {
                candidates.push(tab_id);
            }
        }
        for tab_id in candidates {
            crate::browser::agent_tools::execute(
                self.app,
                "browser_close_page",
                &serde_json::to_string(&json!({"tabId": tab_id}))
                    .map_err(|error| ResearchError::new("serialize_cleanup", error.to_string()))?,
            )
            .await
            .map_err(|error| ResearchError::new("browser_cleanup", error))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    fn newly_created(initial: &HashSet<&str>, current: &[&str]) -> Vec<String> {
        current
            .iter()
            .filter(|id| !initial.contains(**id))
            .map(|id| (*id).to_owned())
            .collect()
    }

    #[test]
    fn cleanup_candidates_exclude_preexisting_agent_tabs() {
        let initial = HashSet::from(["existing"]);
        assert_eq!(
            newly_created(&initial, &["existing", "research"]),
            vec!["research"]
        );
    }
}
