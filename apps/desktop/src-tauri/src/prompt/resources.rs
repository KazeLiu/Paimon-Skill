use std::path::{Path, PathBuf};

use super::models::PromptFragment;

const RESOURCE_DIR: &str = "resources/prompts";
const SKILL_RESOURCE_ID: &str = "genshin-build-wiki";

pub fn default_resource_dir(resource_dir: Option<PathBuf>) -> PathBuf {
    resource_dir.unwrap_or_else(|| PathBuf::from(RESOURCE_DIR))
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
            30,
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
            "来自仓库 skills/genshin-build-wiki/SKILL.md 的可复用规则。",
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
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("无法读取提示词资源 {id}：{error}"))?;
    Ok(PromptFragment {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        content,
        enabled_by_default,
        order,
    })
}
