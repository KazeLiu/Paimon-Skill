pub mod agent_tools;
pub mod commands;
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
}
