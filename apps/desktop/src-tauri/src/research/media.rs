use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KeyFrameRequest {
    pub timestamp_ms: u64,
    pub reason: String,
}
