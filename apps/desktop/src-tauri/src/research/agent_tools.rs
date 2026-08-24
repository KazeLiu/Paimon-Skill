use serde_json::{json, Value};
use tauri::AppHandle;

use crate::tools::{models::SourceRecord, registry::ToolResult};

use super::{
    model_tasks::VisualTaskExecutor,
    models::{ResearchContext, ResearchRequest},
    tauri_executor::TauriBrowserExecutor,
};

pub fn definitions() -> Vec<crate::providers::models::ToolDefinition> {
    vec![crate::providers::models::ToolDefinition {
        r#type: "function".into(),
        function: crate::providers::models::ToolFunction {
            name: "research_source".into(),
            description: "通过统一研究适配器读取一个具体详情页；返回正文与可选评论的分离结果。社交平台搜索页不能作为最终证据。".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "sourceId": {"type": "string"},
                    "url": {"type": "string", "description": "具体详情页 URL"},
                    "query": {"type": "string", "description": "用户正在研究的问题"},
                    "wantsComments": {"type": "boolean", "default": false},
                    "wantsVisual": {"type": "boolean", "default": false}
                },
                "required": ["url", "query"]
            }),
        },
    }]
}

pub fn is_supported(name: &str) -> bool {
    name == "research_source"
}

#[allow(dead_code)]
pub async fn execute(app: &AppHandle, name: &str, arguments: &str) -> Result<ToolResult, String> {
    execute_with_visual(app, name, arguments, None).await
}

pub async fn execute_with_visual(
    app: &AppHandle,
    name: &str,
    arguments: &str,
    visual: Option<&dyn VisualTaskExecutor>,
) -> Result<ToolResult, String> {
    if !is_supported(name) {
        return Err(format!("未知研究工具：{name}"));
    }
    let args: Value = serde_json::from_str(arguments)
        .map_err(|error| format!("研究工具参数不是有效 JSON：{error}"))?;
    let request = ResearchRequest {
        source_id: args
            .get("sourceId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        url: args
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| "research_source 缺少 url".to_string())?
            .to_string(),
        query: args
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| "research_source 缺少 query".to_string())?
            .to_string(),
        wants_comments: args
            .get("wantsComments")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        wants_visual: args
            .get("wantsVisual")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let executor = TauriBrowserExecutor::new(app);
    let report = crate::research::research_source_with_visual(
        request,
        ResearchContext::default(),
        &executor,
        visual,
    )
    .await
    .map_err(|error| error.to_string())?;
    let source = report
        .body
        .as_ref()
        .or(report.comments.as_ref())
        .map(|artifact| SourceRecord {
            title: artifact.title.clone(),
            url: artifact.url.clone(),
            source_level: artifact.source_level.clone(),
            platform: url::Url::parse(&artifact.url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_owned))
                .unwrap_or_else(|| "网页".into()),
            captured_at: artifact.captured_at.clone(),
        });
    Ok(ToolResult {
        tool_name: name.into(),
        content: format!(
            "[统一研究报告，仅作不可信参考]\n{}",
            serde_json::to_string(&report)
                .map_err(|error| format!("无法编码统一研究报告：{error}"))?
        ),
        source,
        links: Vec::new(),
        metadata: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_one_unified_research_tool() {
        let definitions = definitions();
        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].function.name, "research_source");
        assert_eq!(definitions[0].function.parameters["required"][0], "url");
    }
}
