use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceRecord {
    pub title: String,
    pub url: String,
    pub source_level: String,
    pub platform: String,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WebPageResult {
    pub source: SourceRecord,
    pub content: String,
    pub links: Vec<PageLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageLink {
    pub title: String,
    pub url: String,
}
