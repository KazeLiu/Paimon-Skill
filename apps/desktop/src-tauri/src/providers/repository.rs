use std::path::PathBuf;

use super::models::{ProviderConfig, DEFAULT_PROVIDER_TIMEOUT_MS, MAX_PROVIDER_TIMEOUT_MS};
use super::responses::responses_url;

pub struct ProviderRepository {
    path: PathBuf,
}

impl ProviderRepository {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn list(&self) -> Result<Vec<ProviderConfig>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let bytes =
            std::fs::read(&self.path).map_err(|error| format!("无法读取模型配置：{error}"))?;
        let mut providers: Vec<ProviderConfig> =
            serde_json::from_slice(&bytes).map_err(|error| format!("模型配置格式无效：{error}"))?;
        let mut found_default = false;
        for provider in &mut providers {
            // 30 seconds was the old implicit default; relay providers commonly need longer
            // queue time before the first streamed token arrives.
            if provider.timeout_ms == 30_000 {
                provider.timeout_ms = DEFAULT_PROVIDER_TIMEOUT_MS;
            }
            if provider.is_default && !found_default {
                found_default = true;
            } else {
                provider.is_default = false;
            }
        }
        if !providers.is_empty() && !found_default {
            providers[0].is_default = true;
        }
        Ok(providers)
    }

    pub fn upsert(&self, provider: ProviderConfig) -> Result<ProviderConfig, String> {
        validate_provider(&provider)?;
        let mut providers = self.list()?;
        let mut provider = provider;
        if providers.is_empty() {
            provider.is_default = true;
        }
        if provider.is_default {
            for existing in &mut providers {
                existing.is_default = false;
            }
        } else if !providers
            .iter()
            .any(|existing| existing.is_default && existing.id != provider.id)
        {
            provider.is_default = true;
        }
        if let Some(existing) = providers
            .iter_mut()
            .find(|existing| existing.id == provider.id)
        {
            *existing = provider.clone();
        } else {
            providers.push(provider.clone());
        }
        if !providers.iter().any(|existing| existing.is_default) {
            if let Some(first) = providers.first_mut() {
                first.is_default = true;
            }
        }
        providers.sort_by(|left, right| left.id.cmp(&right.id));
        self.save(&providers)?;
        Ok(provider)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        let mut providers = self.list()?;
        let removed_default = providers
            .iter()
            .any(|provider| provider.id == id && provider.is_default);
        providers.retain(|provider| provider.id != id);
        if removed_default
            && !providers.is_empty()
            && !providers.iter().any(|provider| provider.is_default)
        {
            providers[0].is_default = true;
        }
        self.save(&providers)
    }

    fn save(&self, providers: &[ProviderConfig]) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建模型配置目录：{error}"))?;
        }
        let temporary = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(providers)
            .map_err(|error| format!("无法序列化模型配置：{error}"))?;
        std::fs::write(&temporary, bytes)
            .map_err(|error| format!("无法写入临时模型配置：{error}"))?;
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|error| format!("无法替换模型配置：{error}"))?;
        }
        std::fs::rename(&temporary, &self.path)
            .map_err(|error| format!("无法保存模型配置：{error}"))
    }
}

fn validate_provider(provider: &ProviderConfig) -> Result<(), String> {
    if provider.id.trim().is_empty() || provider.name.trim().is_empty() {
        return Err("模型配置的名称不能为空".into());
    }
    if provider.model.trim().is_empty() {
        return Err("模型 ID 不能为空".into());
    }
    responses_url(&provider.base_url)?;
    if provider.timeout_ms < 1_000 || provider.timeout_ms > MAX_PROVIDER_TIMEOUT_MS {
        return Err("请求超时必须在 1-600 秒之间".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(id: &str, name: &str) -> ProviderConfig {
        ProviderConfig {
            id: id.into(),
            name: name.into(),
            base_url: "https://api.deepseek.com".into(),
            model: "deepseek-chat".into(),
            secret_ref: format!("provider:{id}"),
            timeout_ms: 30_000,
            streaming: true,
            tool_calling: true,
            enabled: true,
            is_default: false,
        }
    }

    #[test]
    fn inserts_updates_lists_and_deletes_provider_configs() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ProviderRepository::new(dir.path().join("providers.json"));

        repository.upsert(provider("deepseek", "DeepSeek")).unwrap();
        repository.upsert(provider("local", "本地模型")).unwrap();
        repository
            .upsert(provider("deepseek", "DeepSeek 更新"))
            .unwrap();

        let providers = repository.list().unwrap();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].name, "DeepSeek 更新");
        assert_eq!(providers[1].id, "local");
        assert!(
            providers
                .iter()
                .filter(|provider| provider.is_default)
                .count()
                == 1
        );

        repository.delete("deepseek").unwrap();
        let remaining = repository.list().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "local");
        assert!(remaining[0].is_default);
    }

    #[test]
    fn rejects_invalid_provider_config_without_writing_it() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ProviderRepository::new(dir.path().join("providers.json"));
        let mut invalid = provider("", "");
        invalid.base_url = "file:///tmp/model".into();

        let error = repository.upsert(invalid).unwrap_err();

        assert!(error.contains("名称") || error.contains("地址"));
        assert!(repository.list().unwrap().is_empty());
    }

    #[test]
    fn ignores_removed_legacy_request_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("providers.json");
        std::fs::write(
            &path,
            r#"[{"id":"legacy","name":"Legacy","baseUrl":"https://relay.example/v1","model":"model","secretRef":"provider:legacy","timeoutMs":30000,"temperature":0.3,"maxTokens":2048,"streaming":true,"toolCalling":true,"enabled":true}]"#,
        )
        .unwrap();

        let provider = ProviderRepository::new(path).list().unwrap().remove(0);

        assert_eq!(provider.model, "model");
        assert_eq!(provider.base_url, "https://relay.example/v1");
    }

    #[test]
    fn keeps_exactly_one_default_provider_and_promotes_after_delete() {
        let dir = tempfile::tempdir().unwrap();
        let repository = ProviderRepository::new(dir.path().join("providers.json"));
        repository.upsert(provider("first", "第一")).unwrap();
        repository.upsert(provider("second", "第二")).unwrap();
        let mut second = provider("second", "第二");
        second.is_default = true;
        repository.upsert(second).unwrap();
        assert_eq!(
            repository
                .list()
                .unwrap()
                .iter()
                .filter(|item| item.is_default)
                .count(),
            1
        );
        repository.delete("second").unwrap();
        assert!(repository.list().unwrap()[0].is_default);
    }
}
