use super::models::{ComposedPrompt, PromptFragment, PromptSettings};

pub fn compose_prompt(
    fragments: &[PromptFragment],
    settings: &PromptSettings,
) -> Result<ComposedPrompt, String> {
    let mut ordered = fragments.to_vec();
    ordered.sort_by_key(|fragment| fragment.order);

    let mut resource_ids = Vec::new();
    let mut sections = Vec::new();

    for fragment in ordered {
        if !fragment.enabled_by_default || settings.disabled_resource_ids.contains(&fragment.id) {
            continue;
        }

        let content = settings
            .overrides
            .get(&fragment.id)
            .cloned()
            .unwrap_or(fragment.content)
            .trim()
            .to_owned();

        if !content.is_empty() {
            resource_ids.push(fragment.id);
            sections.push(content);
        }
    }

    let suffix = settings.custom_suffix.trim();
    if !suffix.is_empty() {
        sections.push(suffix.to_owned());
    }

    if sections.is_empty() {
        return Err("系统提示词不能为空".into());
    }

    Ok(ComposedPrompt {
        text: sections.join("\n\n"),
        resource_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::compose_prompt;
    use crate::prompt::models::{PromptFragment, PromptSettings};

    #[test]
    fn composes_enabled_fragments_in_declared_order() {
        let fragments = vec![
            PromptFragment::new("base", "base", true, 10),
            PromptFragment::new("persona", "persona", true, 20),
            PromptFragment::new("skill", "skill", true, 30),
        ];
        let settings = PromptSettings::default();

        let result = compose_prompt(&fragments, &settings).unwrap();

        assert_eq!(result.text, "base\n\npersona\n\nskill");
        assert_eq!(result.resource_ids, vec!["base", "persona", "skill"]);
    }

    #[test]
    fn skips_disabled_resource_and_appends_custom_suffix() {
        let fragments = vec![
            PromptFragment::new("base", "base", true, 10),
            PromptFragment::new("skill", "skill", true, 30),
        ];
        let settings = PromptSettings {
            disabled_resource_ids: vec!["skill".into()],
            custom_suffix: "custom".into(),
            ..PromptSettings::default()
        };

        let result = compose_prompt(&fragments, &settings).unwrap();

        assert_eq!(result.text, "base\n\ncustom");
        assert_eq!(result.resource_ids, vec!["base"]);
    }

    #[test]
    fn applies_user_override_without_changing_resource_order() {
        let fragments = vec![
            PromptFragment::new("base", "default base", true, 10),
            PromptFragment::new("persona", "persona", true, 20),
        ];
        let settings = PromptSettings {
            overrides: [("base".into(), "custom base".into())].into(),
            ..PromptSettings::default()
        };

        let result = compose_prompt(&fragments, &settings).unwrap();

        assert_eq!(result.text, "custom base\n\npersona");
        assert_eq!(result.resource_ids, vec!["base", "persona"]);
    }

    #[test]
    fn rejects_an_empty_composed_prompt() {
        let fragments = vec![PromptFragment::new("base", "", true, 10)];

        let error = compose_prompt(&fragments, &PromptSettings::default()).unwrap_err();

        assert_eq!(error, "系统提示词不能为空");
    }
}
