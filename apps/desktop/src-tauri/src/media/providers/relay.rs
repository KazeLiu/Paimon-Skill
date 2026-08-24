use async_trait::async_trait;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::media::{
    models::{
        AudioInput, CredentialRef, TranscriptionError, TranscriptionErrorKind,
        TranscriptionRequest, TranscriptionResult,
    },
    transcription::{ProviderAvailability, ProviderDescriptor, TranscriptionProvider},
};

const MAX_RESPONSE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelayConfig {
    pub endpoint: String,
    /// Opaque keyring reference; not the bearer token itself.
    pub credential_ref: String,
}

impl RelayConfig {
    pub fn validate(&self) -> Result<Url, TranscriptionError> {
        let url = Url::parse(&self.endpoint)
            .map_err(|_| TranscriptionError::invalid_request("中转 endpoint 无效"))?;
        if url.scheme() != "https" && !is_loopback(&url) {
            return Err(TranscriptionError::invalid_request(
                "中转 endpoint 必须使用 HTTPS；仅本机 loopback 可使用 HTTP",
            ));
        }
        if url.username() != "" || url.password().is_some() || url.query().is_some() {
            return Err(TranscriptionError::invalid_request(
                "中转 endpoint 不得包含凭据或查询参数",
            ));
        }
        CredentialRef::new(self.credential_ref.clone())?;
        Ok(url)
    }
}

fn is_loopback(url: &Url) -> bool {
    matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
}

pub struct RelayProvider {
    http: reqwest::Client,
    endpoint: Url,
    bearer_token: String,
}

impl RelayProvider {
    pub const ID: &'static str = "relay";

    pub const fn descriptor() -> ProviderDescriptor {
        ProviderDescriptor {
            id: Self::ID,
            display_name: "自有中转",
            availability: ProviderAvailability::Available,
            reason: None,
        }
    }

    /// `bearer_token` must come from the platform secret store. This provider
    /// never serializes or logs it.
    pub fn new(config: &RelayConfig, bearer_token: String) -> Result<Self, TranscriptionError> {
        let endpoint = config.validate()?;
        if bearer_token.trim().is_empty() {
            return Err(TranscriptionError::invalid_request("中转凭据为空"));
        }
        let http = reqwest::Client::builder()
            .user_agent("PaimonDesktop/0.1")
            .build()
            .map_err(|error| {
                TranscriptionError::new(
                    TranscriptionErrorKind::Provider,
                    "relay_client",
                    format!("无法创建中转客户端：{error}"),
                    false,
                )
            })?;
        Ok(Self {
            http,
            endpoint,
            bearer_token,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayRequest<'a> {
    audio_url: &'a str,
    format: &'a str,
    sample_rate_hz: Option<u32>,
    language: &'a str,
    include_timestamps: bool,
    include_punctuation: bool,
    hotwords: &'a [String],
    max_duration_ms: Option<u64>,
}

#[async_trait]
impl TranscriptionProvider for RelayProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        Self::descriptor()
    }

    async fn transcribe(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResult, TranscriptionError> {
        request.validate()?;
        if request.cancellation.is_cancelled() {
            return Err(TranscriptionError::cancelled());
        }
        let audio_url = match &request.audio {
            AudioInput::RemoteUrl { url } => url.as_str(),
            AudioInput::LocalFile { .. } => {
                return Err(TranscriptionError::invalid_request(
                    "自有中转当前只接受无签名的公开音频 URL",
                ))
            }
        };
        let format = serde_json::to_value(request.format)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "unknown".into());
        let body = RelayRequest {
            audio_url,
            format: &format,
            sample_rate_hz: request.sample_rate_hz,
            language: &request.language,
            include_timestamps: request.include_timestamps,
            include_punctuation: request.include_punctuation,
            hotwords: &request.hotwords,
            max_duration_ms: request.max_duration_ms,
        };
        let response = tokio::select! {
            _ = request.cancellation.cancelled() => return Err(TranscriptionError::cancelled()),
            result = self.http
                .post(self.endpoint.clone())
                .header(AUTHORIZATION, format!("Bearer {}", self.bearer_token))
                .header(CONTENT_TYPE, "application/json")
                .header(ACCEPT, "application/json")
                .json(&body)
                .send() => result,
        }
        .map_err(|error| {
            TranscriptionError::new(
                TranscriptionErrorKind::Provider,
                "relay_request",
                format!("中转请求失败：{error}"),
                true,
            )
        })?;
        let status = response.status();
        let declared = response.content_length().unwrap_or(0);
        if declared > MAX_RESPONSE_BYTES {
            return Err(TranscriptionError::invalid_response("中转响应超过大小上限"));
        }
        let bytes = response.bytes().await.map_err(|error| {
            TranscriptionError::new(
                TranscriptionErrorKind::Provider,
                "relay_body",
                format!("无法读取中转响应：{error}"),
                true,
            )
        })?;
        if bytes.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(TranscriptionError::invalid_response("中转响应超过大小上限"));
        }
        if !status.is_success() {
            return Err(TranscriptionError::new(
                TranscriptionErrorKind::Provider,
                format!("relay_http_{}", status.as_u16()),
                format!("中转返回 HTTP {}", status.as_u16()),
                status.is_server_error(),
            ));
        }
        let mut result: TranscriptionResult = serde_json::from_slice(&bytes)
            .map_err(|_| TranscriptionError::invalid_response("中转响应不是有效的转写结果"))?;
        result.provider_id = Self::ID.into();
        result.request_id = sanitize_request_id(result.request_id.take());
        result.validate()?;
        Ok(result)
    }
}

fn sanitize_request_id(value: Option<String>) -> Option<String> {
    value.map(|value| {
        value
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
            .take(96)
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_requires_https_except_loopback_and_rejects_query_secrets() {
        let config = |endpoint: &str| RelayConfig {
            endpoint: endpoint.into(),
            credential_ref: "asr/relay/default".into(),
        };
        assert!(config("http://example.com/asr").validate().is_err());
        assert!(config("http://127.0.0.1:8080/asr").validate().is_ok());
        assert!(config("https://example.com/asr?token=x")
            .validate()
            .is_err());
    }

    #[test]
    fn request_id_is_bounded_and_strips_punctuation() {
        assert_eq!(
            sanitize_request_id(Some("req/../../secret?token=x".into())).unwrap(),
            "reqsecrettokenx"
        );
    }
}
