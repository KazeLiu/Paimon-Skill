use std::{collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{
    models::{TranscriptionError, TranscriptionRequest, TranscriptionResult},
    providers::{
        AliyunProvider, RelayConfig, RelayProvider, TencentProvider, UnavailableProvider,
        VolcengineProvider,
    },
};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAvailability {
    Available,
    Reserved,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub availability: ProviderAvailability,
    pub reason: Option<&'static str>,
}

pub fn provider_descriptor(id: &str) -> Option<ProviderDescriptor> {
    match id {
        TencentProvider::ID => Some(TencentProvider::descriptor()),
        AliyunProvider::ID => Some(AliyunProvider::descriptor()),
        VolcengineProvider::ID => Some(VolcengineProvider::descriptor()),
        // A relay becomes available only after an endpoint and keyring-backed
        // token are registered in `ProviderRegistry`.
        RelayProvider::ID => Some(ProviderDescriptor {
            id: RelayProvider::ID,
            display_name: "自有中转",
            availability: ProviderAvailability::Reserved,
            reason: Some("尚未配置经过校验的中转 endpoint"),
        }),
        _ => None,
    }
}

#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    fn descriptor(&self) -> ProviderDescriptor;

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError>;
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn TranscriptionProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(TencentProvider));
        registry.register(Arc::new(AliyunProvider));
        registry.register(Arc::new(VolcengineProvider));
        registry.register(Arc::new(UnavailableProvider::relay_placeholder()));
        registry
    }

    pub fn register(&mut self, provider: Arc<dyn TranscriptionProvider>) {
        self.providers
            .insert(provider.descriptor().id.to_owned(), provider);
    }

    /// Replace the unconfigured relay placeholder with a provider backed by a
    /// platform-resolved secret. The token is consumed in memory only.
    pub fn register_relay(
        &mut self,
        config: &RelayConfig,
        bearer_token: String,
    ) -> Result<(), TranscriptionError> {
        let provider = RelayProvider::new(config, bearer_token)?;
        self.register(Arc::new(provider));
        Ok(())
    }

    pub fn resolve(&self, id: &str) -> Result<Arc<dyn TranscriptionProvider>, TranscriptionError> {
        self.providers.get(id).cloned().ok_or_else(|| {
            TranscriptionError::unavailable(id, format!("未知的语音转写供应商：{id}"))
        })
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .values()
            .map(|provider| provider.descriptor())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::providers::RelayConfig;

    #[test]
    fn registry_routes_known_providers_and_rejects_unknown_ids() {
        let registry = ProviderRegistry::new();
        assert_eq!(
            registry.resolve("tencent").unwrap().descriptor().id,
            "tencent"
        );
        assert_eq!(
            registry.resolve("aliyun").unwrap().descriptor().id,
            "aliyun"
        );
        assert_eq!(
            registry.resolve("volcengine").unwrap().descriptor().id,
            "volcengine"
        );
        assert_eq!(registry.resolve("relay").unwrap().descriptor().id, "relay");
        assert!(registry.resolve("unknown").is_err());
    }

    #[test]
    fn native_providers_do_not_claim_false_availability() {
        let registry = ProviderRegistry::new();
        for id in ["tencent", "aliyun", "volcengine"] {
            assert_eq!(
                registry.resolve(id).unwrap().descriptor().availability,
                ProviderAvailability::Reserved
            );
        }
    }

    #[test]
    fn relay_is_reserved_until_configured_then_has_an_available_descriptor() {
        let mut registry = ProviderRegistry::new();
        assert_eq!(
            registry.resolve("relay").unwrap().descriptor().availability,
            ProviderAvailability::Reserved
        );
        registry
            .register_relay(
                &RelayConfig {
                    endpoint: "https://relay.example/asr".into(),
                    credential_ref: "asr/relay/default".into(),
                },
                "test-token".into(),
            )
            .unwrap();
        assert_eq!(
            registry.resolve("relay").unwrap().descriptor().availability,
            ProviderAvailability::Available
        );
    }
}
