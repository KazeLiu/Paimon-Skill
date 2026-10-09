pub mod agent_tools;
pub mod commands;
pub mod login;
pub mod models;
pub mod policy;
pub mod scripts;
pub mod state;
pub mod task;

#[cfg(test)]
mod agent_tools_tests {
    use super::{
        agent_tools::{resolve_tab_id, snapshot_to_tool_result},
        models::{BrowserTab, PageLink, PageSnapshot},
        state::BrowserState,
    };

    fn tab(id: &str, profile_id: &str) -> BrowserTab {
        BrowserTab {
            id: id.into(),
            title: id.into(),
            url: format!("https://{id}.example.com"),
            loading: false,
            profile_id: profile_id.into(),
        }
    }

    #[test]
    fn agent_browser_tool_defaults_to_the_active_existing_tab() {
        let state = BrowserState::default();
        state.insert(tab("first", "default"));
        state.insert(tab("active", "default"));

        assert_eq!(resolve_tab_id(&state, None).unwrap(), "active");
        assert_eq!(resolve_tab_id(&state, Some("first")).unwrap(), "first");
        assert!(resolve_tab_id(&state, Some("missing")).is_err());
    }

    #[test]
    fn agent_browser_snapshot_becomes_a_persistable_tool_source() {
        let result = snapshot_to_tool_result(
            "browser_extract_page",
            &tab("active", "default"),
            PageSnapshot {
                title: "动态页面".into(),
                url: "https://example.com/comments".into(),
                text: "评论正文".into(),
                links: vec![PageLink {
                    text: "下一页".into(),
                    url: "https://example.com/page/2".into(),
                }],
                login_challenge: true,
                challenge_reason: Some("captcha".into()),
                truncated: true,
                structure: None,
                snapshot_generation: None,
                page_fingerprint: None,
                interactive_total: 0,
                shadow_root_count: 0,
            },
        );

        assert_eq!(result.tool_name, "browser_extract_page");
        assert!(result.content.contains("评论正文"));
        assert!(result.content.contains("active"));
        assert!(result.content.contains("default"));
        assert!(result.content.contains("captcha"));
        assert!(result.content.contains("截断"));
        assert_eq!(result.source.as_ref().unwrap().title, "动态页面");
        assert_eq!(result.source.as_ref().unwrap().platform, "example.com");
        assert_eq!(result.links[0].title, "下一页");
    }

    #[test]
    fn agent_browser_definitions_expose_semantic_lookup_and_click() {
        let names = super::agent_tools::definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect::<Vec<_>>();

        assert!(names.contains(&"browser_find_context".to_string()));
        assert!(names.contains(&"browser_click_element".to_string()));
    }

    #[test]
    fn browser_snapshot_keeps_a_valid_bounded_semantic_tree_for_the_model() {
        let structure = serde_json::json!({
            "tree": {
                "type": "document",
                "children": (0..300).map(|index| serde_json::json!({
                    "type": "button",
                    "elementId": format!("page-{index}"),
                    "name": "很长的按钮标签".repeat(50),
                })).collect::<Vec<_>>(),
            }
        });
        let result = snapshot_to_tool_result(
            "browser_extract_page",
            &tab("active", "default"),
            PageSnapshot {
                title: "动态页面".into(),
                url: "https://example.com".into(),
                text: "正文".repeat(10_000),
                links: Vec::new(),
                login_challenge: false,
                challenge_reason: None,
                truncated: false,
                structure: Some(structure),
                snapshot_generation: Some(1),
                page_fingerprint: None,
                interactive_total: 300,
                shadow_root_count: 0,
            },
        );

        assert!(result.content.chars().count() < 24_000);
        let encoded = result.content.split("语义 DOM：\n").nth(1).unwrap().trim();
        assert!(serde_json::from_str::<serde_json::Value>(encoded).is_ok());
    }

    #[test]
    fn bilibili_search_snapshot_is_marked_discovery_only() {
        let result = snapshot_to_tool_result(
            "browser_open_page",
            &tab("active", "default"),
            PageSnapshot {
                title: "搜索结果".into(),
                url: "https://search.bilibili.com/all?keyword=配队".into(),
                text: "候选视频".into(),
                links: vec![],
                login_challenge: false,
                challenge_reason: None,
                truncated: false,
                structure: None,
                snapshot_generation: None,
                page_fingerprint: None,
                interactive_total: 0,
                shadow_root_count: 0,
            },
        );

        assert!(result.content.contains("只能用于发现候选视频"));
        assert!(result.content.contains("research_bilibili"));
        assert!(result
            .content
            .contains("不能把搜索页或详情 DOM 当作最终证据"));
    }
}
