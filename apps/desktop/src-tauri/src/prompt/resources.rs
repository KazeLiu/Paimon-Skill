use std::path::{Path, PathBuf};

use super::models::PromptFragment;

const SKILL_RESOURCE_ID: &str = "genshin-build-wiki";

pub fn resolve_resource_dir(resource_root: PathBuf) -> PathBuf {
    let candidates = [
        resource_root.join("prompts"),
        resource_root.join("resources").join("prompts"),
        resource_root.join("_up_").join("resources").join("prompts"),
    ];

    candidates
        .into_iter()
        .find(|candidate| candidate.join("base-system.md").is_file())
        .unwrap_or_else(|| resource_root.join("resources").join("prompts"))
}

pub fn load_fragments(resource_dir: &Path) -> Result<Vec<PromptFragment>, String> {
    let definitions = [
        (
            "base-system",
            "基础系统规则",
            "应用安全、事实核查与网页内容边界。",
            "base-system.md",
            10,
        ),
        (
            "paimon-persona",
            "派蒙语气",
            "角色语气、称呼和输出风格。",
            "paimon-persona.md",
            20,
        ),
        (
            "tool-policy",
            "工具使用规则",
            "工具权限、来源记录和失败处理。",
            "tool-policy.md",
            50,
        ),
    ];

    let mut fragments = Vec::with_capacity(definitions.len() + 1);
    for (id, title, description, filename, order) in definitions {
        let path = resource_dir.join(filename);
        fragments.push(read_fragment(&path, id, title, description, true, order)?);
    }

    let skill_path = resource_dir
        .parent()
        .unwrap_or(resource_dir)
        .join("rules")
        .join(SKILL_RESOURCE_ID)
        .join("SKILL.md");
    if skill_path.exists() {
        fragments.push(read_fragment(
            &skill_path,
            SKILL_RESOURCE_ID,
            "原神攻略规则",
            "来自仓库 SKILL.md 正文的领域规则；执行方式由桌面端工具规则适配。",
            true,
            40,
        )?);
    }

    Ok(fragments)
}

fn read_fragment(
    path: &Path,
    id: &str,
    title: &str,
    description: &str,
    enabled_by_default: bool,
    order: u32,
) -> Result<PromptFragment, String> {
    let mut content = std::fs::read_to_string(path)
        .map_err(|error| format!("无法读取提示词资源 {id}：{error}"))?;
    if id == SKILL_RESOURCE_ID {
        content = skill_body(&content).to_owned();
    }
    Ok(PromptFragment {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        content,
        enabled_by_default,
        order,
    })
}

fn skill_body(content: &str) -> &str {
    let content = content.trim_start_matches('\u{feff}');
    let Some(metadata) = content
        .strip_prefix("---\r\n")
        .or_else(|| content.strip_prefix("---\n"))
    else {
        return content;
    };
    for delimiter in ["\n---\r\n", "\n---\n"] {
        if let Some(end) = metadata.find(delimiter) {
            return metadata[end + delimiter.len()..].trim_start();
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::{load_fragments, resolve_resource_dir, skill_body};
    use crate::prompt::{composer::compose_prompt, models::PromptSettings};

    fn packaged_fragments() -> Vec<crate::prompt::models::PromptFragment> {
        let root = tempfile::tempdir().unwrap();
        let prompts = root.path().join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();
        let maintained = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for filename in ["base-system.md", "paimon-persona.md", "tool-policy.md"] {
            std::fs::copy(
                maintained.join("../resources/prompts").join(filename),
                prompts.join(filename),
            )
            .unwrap();
        }
        let rules = root.path().join("rules/genshin-build-wiki");
        std::fs::create_dir_all(&rules).unwrap();
        std::fs::copy(
            maintained.join("../../../skills/genshin-build-wiki/SKILL.md"),
            rules.join("SKILL.md"),
        )
        .unwrap();
        load_fragments(&prompts).unwrap()
    }

    #[test]
    fn loads_skill_body_without_discovery_frontmatter() {
        let fragments = packaged_fragments();
        let skill = fragments
            .iter()
            .find(|fragment| fragment.id == "genshin-build-wiki")
            .unwrap();
        assert!(skill.content.starts_with("# 原神攻略与图鉴查询"));
        assert!(skill.content.contains("第一方资料"));
    }

    #[test]
    fn skill_body_handles_windows_line_endings_and_utf8_bom() {
        assert_eq!(
            skill_body("\u{feff}---\r\nname: sample\r\n---\r\n\r\n# 正文"),
            "# 正文"
        );
    }

    #[test]
    fn skill_body_keeps_plain_markdown_and_unclosed_metadata() {
        for content in ["# 正文\n---\n内容", "---\nname: sample\n# 未闭合"] {
            assert_eq!(skill_body(content), content);
        }
    }

    #[test]
    fn desktop_tool_policy_follows_shared_skill_and_survives_disabling_it() {
        let fragments = packaged_fragments();
        let prompt = compose_prompt(&fragments, &PromptSettings::default()).unwrap();
        assert_eq!(prompt.resource_ids.last().unwrap(), "tool-policy");
        assert!(prompt.text.contains("read_bwiki_page"));
        let settings = PromptSettings {
            disabled_resource_ids: vec!["genshin-build-wiki".into()],
            ..PromptSettings::default()
        };
        let prompt = compose_prompt(&fragments, &settings).unwrap();
        assert!(!prompt
            .resource_ids
            .iter()
            .any(|id| id == "genshin-build-wiki"));
        assert!(prompt.text.contains("read_bwiki_page"));
    }

    #[test]
    fn resolves_resources_from_tauri_dev_up_directory() {
        let root = tempfile::tempdir().unwrap();
        let expected = root.path().join("_up_").join("resources").join("prompts");
        std::fs::create_dir_all(&expected).unwrap();
        std::fs::write(expected.join("base-system.md"), "base").unwrap();

        assert_eq!(resolve_resource_dir(root.path().to_path_buf()), expected);
    }

    #[test]
    fn resolves_resources_when_resource_dir_already_points_at_resources() {
        let root = tempfile::tempdir().unwrap();
        let expected = root.path().join("prompts");
        std::fs::create_dir_all(&expected).unwrap();
        std::fs::write(expected.join("base-system.md"), "base").unwrap();

        assert_eq!(resolve_resource_dir(root.path().to_path_buf()), expected);
    }
}
