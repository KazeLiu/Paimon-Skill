use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PromptFragment {
    pub id: String,
    pub title: String,
    pub description: String,
    pub content: String,
    pub enabled_by_default: bool,
    pub order: u32,
}

#[cfg(test)]
impl PromptFragment {
    pub fn new(id: &str, content: &str, enabled_by_default: bool, order: u32) -> Self {
        Self {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            content: content.into(),
            enabled_by_default,
            order,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct PromptSettings {
    pub overrides: BTreeMap<String, String>,
    pub disabled_resource_ids: Vec<String>,
    pub custom_suffix: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComposedPrompt {
    pub text: String,
    pub resource_ids: Vec<String>,
}
