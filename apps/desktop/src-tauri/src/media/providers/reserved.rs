use async_trait::async_trait;

use crate::media::{
    models::{TranscriptionError, TranscriptionRequest, TranscriptionResult},
    transcription::{ProviderAvailability, ProviderDescriptor, TranscriptionProvider},
};

pub struct UnavailableProvider {
    descriptor: ProviderDescriptor,
}

impl UnavailableProvider {
    pub fn new(id: &'static str, display_name: &'static str, reason: &'static str) -> Self {
        Self {
            descriptor: ProviderDescriptor {
                id,
                display_name,
                availability: ProviderAvailability::Reserved,
                reason: Some(reason),
            },
        }
    }

    pub fn relay_placeholder() -> Self {
        Self::new("relay", "自有中转", "尚未提供经过校验的中转 endpoint 配置")
    }
}

#[async_trait]
impl TranscriptionProvider for UnavailableProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        if request.cancellation.is_cancelled() {
            return Err(TranscriptionError::cancelled());
        }
        request.validate()?;
        Err(TranscriptionError::unavailable(
            self.descriptor.id,
            self.descriptor.reason.unwrap_or("供应商尚未接通"),
        ))
    }
}
