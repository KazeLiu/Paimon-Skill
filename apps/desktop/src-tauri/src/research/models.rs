use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRequest {
    pub source_id: Option<String>,
    pub url: String,
    pub query: String,
    #[serde(default)]
    pub wants_comments: bool,
    #[serde(default)]
    pub wants_visual: bool,
}

#[derive(Debug, Clone)]
pub struct ResearchContext {
    pub cancellation: CancellationToken,
    pub max_scroll_rounds: u8,
}

impl Default for ResearchContext {
    fn default() -> Self {
        Self {
            cancellation: CancellationToken::new(),
            max_scroll_rounds: 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchArtifactKind {
    Body,
    Comments,
    Transcript,
    Visual,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchArtifact {
    pub kind: ResearchArtifactKind,
    pub title: String,
    pub url: String,
    pub content: String,
    pub source_level: String,
    pub captured_at: String,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReport {
    pub source_id: Option<String>,
    pub title: String,
    pub url: String,
    pub body: Option<ResearchArtifact>,
    pub comments: Option<ResearchArtifact>,
    pub transcript: Option<ResearchArtifact>,
    pub visual: Option<ResearchArtifact>,
    pub warnings: Vec<String>,
}

impl ResearchReport {
    pub fn empty(request: &ResearchRequest) -> Self {
        Self {
            source_id: request.source_id.clone(),
            title: String::new(),
            url: request.url.clone(),
            body: None,
            comments: None,
            transcript: None,
            visual: None,
            warnings: Vec::new(),
        }
    }

    pub fn add_artifact(&mut self, artifact: ResearchArtifact) {
        if self.title.is_empty() {
            self.title = artifact.title.clone();
        }
        self.url = artifact.url.clone();
        match artifact.kind {
            ResearchArtifactKind::Body => self.body = Some(artifact),
            ResearchArtifactKind::Comments => self.comments = Some(artifact),
            ResearchArtifactKind::Transcript => self.transcript = Some(artifact),
            ResearchArtifactKind::Visual => self.visual = Some(artifact),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchAction {
    OpenPage,
    ExtractPage,
    FindContext,
    ClickElement,
    ScrollPage,
    ExtractComments,
    ExtractBilibiliSubtitles,
    ExtractBilibiliKeyframes,
    ClosePage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchActionRequest {
    pub action: ResearchAction,
    pub url: Option<String>,
    pub tab_id: Option<String>,
    pub query: Option<String>,
    pub element_id: Option<String>,
    pub max_scroll_rounds: u8,
    pub all_parts: bool,
    pub timestamps_ms: Vec<u64>,
}

impl ResearchActionRequest {
    pub fn open(url: String) -> Self {
        Self {
            action: ResearchAction::OpenPage,
            url: Some(url),
            tab_id: None,
            query: None,
            element_id: None,
            max_scroll_rounds: 0,
            all_parts: false,
            timestamps_ms: Vec::new(),
        }
    }

    pub fn tab(action: ResearchAction, tab_id: Option<String>) -> Self {
        Self {
            action,
            url: None,
            tab_id,
            query: None,
            element_id: None,
            max_scroll_rounds: 0,
            all_parts: false,
            timestamps_ms: Vec::new(),
        }
    }

    pub fn find_context(tab_id: Option<String>, query: String) -> Self {
        Self {
            action: ResearchAction::FindContext,
            url: None,
            tab_id,
            query: Some(query),
            element_id: None,
            max_scroll_rounds: 0,
            all_parts: false,
            timestamps_ms: Vec::new(),
        }
    }

    pub fn click_element(tab_id: Option<String>, element_id: String) -> Self {
        Self {
            action: ResearchAction::ClickElement,
            url: None,
            tab_id,
            query: None,
            element_id: Some(element_id),
            max_scroll_rounds: 0,
            all_parts: false,
            timestamps_ms: Vec::new(),
        }
    }

    pub fn bilibili_subtitles(tab_id: Option<String>, all_parts: bool) -> Self {
        Self {
            action: ResearchAction::ExtractBilibiliSubtitles,
            url: None,
            tab_id,
            query: None,
            element_id: None,
            max_scroll_rounds: 0,
            all_parts,
            timestamps_ms: Vec::new(),
        }
    }

    pub fn bilibili_keyframes(tab_id: Option<String>, timestamps_ms: Vec<u64>) -> Self {
        Self {
            action: ResearchAction::ExtractBilibiliKeyframes,
            url: None,
            tab_id,
            query: None,
            element_id: None,
            max_scroll_rounds: 0,
            all_parts: false,
            timestamps_ms,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchError {
    pub code: String,
    pub message: String,
}

impl ResearchError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ResearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ResearchError {}

impl From<String> for ResearchError {
    fn from(message: String) -> Self {
        Self::new("research_error", message)
    }
}
