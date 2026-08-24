use async_trait::async_trait;

use crate::media::{
    models::{TranscriptionError, TranscriptionRequest, TranscriptionResult},
    transcription::{ProviderAvailability, ProviderDescriptor, TranscriptionProvider},
};

/// Tencent Cloud recording-file ASR is intentionally reserved until the
/// crate declares audited HMAC-SHA256/SHA256 dependencies and a TC3 contract
/// test can be added. See docs/research/asr-provider-matrix.md.
pub struct TencentProvider;

impl TencentProvider {
    pub const ID: &'static str = "tencent";

    pub const fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            id: Self::ID,
            display_name: "腾讯云录音文件识别",
            availability: ProviderAvailability::Reserved,
            reason: Some("TC3-HMAC-SHA256 尚未接入；禁止用未经审计的自制签名替代"),
        }
    }
}

#[async_trait]
impl TranscriptionProvider for TencentProvider {
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
