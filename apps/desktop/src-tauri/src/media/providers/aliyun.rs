use async_trait::async_trait;

use crate::media::{
    models::{TranscriptionError, TranscriptionRequest, TranscriptionResult},
    transcription::{ProviderAvailability, ProviderDescriptor, TranscriptionProvider},
};

pub struct AliyunProvider;

impl AliyunProvider {
    pub const ID: &'static str = "aliyun";

    pub const fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            id: Self::ID,
            display_name: "阿里云录音文件识别",
            availability: ProviderAvailability::Reserved,
            reason: Some("已核对能力与端侧安全边界，POP/Token 协议尚未实现"),
        }
    }
}

#[async_trait]
impl TranscriptionProvider for AliyunProvider {
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
