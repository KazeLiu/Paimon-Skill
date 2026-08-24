use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use url::Url;

pub const MAX_HOTWORDS: usize = 128;
pub const MAX_HOTWORD_CHARS: usize = 30;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Wav,
    Mp3,
    M4a,
    Mp4,
    Aac,
    Flac,
    Ogg,
    Opus,
    Amr,
    Wma,
    Raw,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioInput {
    RemoteUrl {
        /// Must not contain credentials or signed query parameters.
        url: String,
    },
    LocalFile {
        /// Platform-private path. Never serialize this model into diagnostics or
        /// model context; only a provider bridge may consume it.
        path: String,
    },
}

impl AudioInput {
    pub fn validate(&self) -> Result<(), TranscriptionError> {
        match self {
            Self::RemoteUrl { url } => {
                let parsed = Url::parse(url).map_err(|_| {
                    TranscriptionError::invalid_request("音频 URL 不是有效的绝对 URL")
                })?;
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(TranscriptionError::invalid_request(
                        "音频 URL 只允许 http 或 https",
                    ));
                }
                if parsed.username() != "" || parsed.password().is_some() {
                    return Err(TranscriptionError::invalid_request(
                        "音频 URL 不得包含用户名或密码",
                    ));
                }
                if parsed.query().is_some() || parsed.fragment().is_some() {
                    return Err(TranscriptionError::invalid_request(
                        "音频 URL 不得包含查询参数或片段，避免签名和令牌泄漏",
                    ));
                }
            }
            Self::LocalFile { path } if path.trim().is_empty() => {
                return Err(TranscriptionError::invalid_request("本地音频路径不能为空"));
            }
            Self::LocalFile { .. } => {}
        }
        Ok(())
    }
}

/// Opaque reference resolved by the platform secret store. It is safe to
/// persist because it never contains the credential itself.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct CredentialRef(pub String);

impl CredentialRef {
    pub fn new(value: impl Into<String>) -> Result<Self, TranscriptionError> {
        let value = value.into();
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 256 {
            return Err(TranscriptionError::invalid_request(
                "凭据引用必须为 1 到 256 个字符",
            ));
        }
        if trimmed.to_ascii_lowercase().contains("secret")
            || trimmed.starts_with("sk-")
            || trimmed.contains('=')
        {
            return Err(TranscriptionError::invalid_request(
                "凭据引用看起来包含明文凭据",
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }
}

#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    pub audio: AudioInput,
    pub format: AudioFormat,
    pub sample_rate_hz: Option<u32>,
    pub language: String,
    pub include_timestamps: bool,
    pub include_punctuation: bool,
    pub hotwords: Vec<String>,
    pub max_duration_ms: Option<u64>,
    pub credential_ref: CredentialRef,
    pub cancellation: CancellationToken,
}

impl TranscriptionRequest {
    pub fn validate(&self) -> Result<(), TranscriptionError> {
        self.audio.validate()?;
        if self.language.trim().is_empty() || self.language.len() > 32 {
            return Err(TranscriptionError::invalid_request("语言代码无效"));
        }
        if let Some(rate) = self.sample_rate_hz {
            if !(8_000..=192_000).contains(&rate) {
                return Err(TranscriptionError::invalid_request("采样率超出支持范围"));
            }
        }
        if self.hotwords.len() > MAX_HOTWORDS {
            return Err(TranscriptionError::invalid_request("热词数量超过上限"));
        }
        if self
            .hotwords
            .iter()
            .any(|word| word.trim().is_empty() || word.chars().count() > MAX_HOTWORD_CHARS)
        {
            return Err(TranscriptionError::invalid_request("热词为空或过长"));
        }
        if self.max_duration_ms == Some(0) {
            return Err(TranscriptionError::invalid_request("最大时长必须大于零"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub confidence: Option<f32>,
    pub speaker: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub segments: Vec<TranscriptionSegment>,
    pub language: String,
    pub provider_id: String,
    /// A bounded, non-secret identifier suitable for support diagnostics.
    pub request_id: Option<String>,
    pub warnings: Vec<String>,
    pub captured_at: DateTime<Utc>,
}

impl TranscriptionResult {
    pub fn validate(&self) -> Result<(), TranscriptionError> {
        let mut previous_end = 0;
        for segment in &self.segments {
            if segment.start_ms > segment.end_ms || segment.start_ms < previous_end {
                return Err(TranscriptionError::invalid_response(
                    "转写分段时间戳无序或范围无效",
                ));
            }
            if segment
                .confidence
                .is_some_and(|value| !(0.0..=1.0).contains(&value))
            {
                return Err(TranscriptionError::invalid_response("置信度超出 0 到 1"));
            }
            previous_end = segment.end_ms;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionErrorKind {
    InvalidRequest,
    Unavailable,
    Authentication,
    Provider,
    InvalidResponse,
    Timeout,
    Cancelled,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionError {
    pub kind: TranscriptionErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
    /// A bounded and sanitized upstream request identifier only.
    pub request_id: Option<String>,
}

impl TranscriptionError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(
            TranscriptionErrorKind::InvalidRequest,
            "invalid_request",
            message,
            false,
        )
    }

    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::new(
            TranscriptionErrorKind::InvalidResponse,
            "invalid_response",
            message,
            false,
        )
    }

    pub fn unavailable(provider: &str, reason: impl Into<String>) -> Self {
        Self::new(
            TranscriptionErrorKind::Unavailable,
            format!("{provider}_unavailable"),
            reason,
            false,
        )
    }

    pub fn cancelled() -> Self {
        Self::new(
            TranscriptionErrorKind::Cancelled,
            "cancelled",
            "语音转写已取消",
            false,
        )
    }

    pub fn new(
        kind: TranscriptionErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into().chars().take(96).collect(),
            message: message.into().chars().take(1_024).collect(),
            retryable,
            request_id: None,
        }
    }
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for TranscriptionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(audio: AudioInput) -> TranscriptionRequest {
        TranscriptionRequest {
            audio,
            format: AudioFormat::Mp3,
            sample_rate_hz: Some(16_000),
            language: "zh-CN".into(),
            include_timestamps: true,
            include_punctuation: true,
            hotwords: vec!["派蒙".into()],
            max_duration_ms: Some(60_000),
            credential_ref: CredentialRef::new("asr/tencent/default").unwrap(),
            cancellation: CancellationToken::new(),
        }
    }

    #[test]
    fn rejects_signed_or_credential_bearing_audio_urls() {
        for url in [
            "https://user:pass@example.com/audio.mp3",
            "https://example.com/audio.mp3?token=secret",
        ] {
            assert!(request(AudioInput::RemoteUrl { url: url.into() })
                .validate()
                .is_err());
        }
    }

    #[test]
    fn credential_reference_cannot_look_like_a_secret() {
        assert!(CredentialRef::new("sk-live-value").is_err());
        assert!(CredentialRef::new("token=live-value").is_err());
        assert!(CredentialRef::new("asr/tencent/default").is_ok());
    }

    #[test]
    fn result_requires_ordered_segments() {
        let result = TranscriptionResult {
            text: "测试".into(),
            segments: vec![
                TranscriptionSegment {
                    text: "后".into(),
                    start_ms: 2_000,
                    end_ms: 3_000,
                    confidence: None,
                    speaker: None,
                },
                TranscriptionSegment {
                    text: "前".into(),
                    start_ms: 1_000,
                    end_ms: 1_500,
                    confidence: None,
                    speaker: None,
                },
            ],
            language: "zh-CN".into(),
            provider_id: "test".into(),
            request_id: None,
            warnings: vec![],
            captured_at: Utc::now(),
        };
        assert_eq!(
            result.validate().unwrap_err().kind,
            TranscriptionErrorKind::InvalidResponse
        );
    }
}
