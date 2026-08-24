use async_trait::async_trait;

use crate::media::{
    models::{TranscriptionError, TranscriptionRequest, TranscriptionResult},
    transcription::{ProviderAvailability, ProviderDescriptor, TranscriptionProvider},
};

pub struct VolcengineProvider;

impl VolcengineProvider {
    pub const ID: &'static str = "volcengine";

    pub const fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            id: Self::ID,
            display_name: "火山引擎录音文件识别",
            availability: ProviderAvailability::Reserved,
            reason: Some("已核对 HTTP submit/query 契约，凭据与配置链路尚未实现"),
        }
    }
}

#[async_trait]
impl TranscriptionProvider for VolcengineProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        Self::descriptor()
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
            Self::ID,
            Self::descriptor().reason.unwrap(),
        ))
    }
}
