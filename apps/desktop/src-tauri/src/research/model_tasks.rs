use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use futures_util::StreamExt;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    attachments::{AttachmentKind, ChatAttachment},
    providers::{
        models::{ChatMessage, ProviderConfig},
        openai_compatible::OpenAiCompatibleClient,
    },
};

pub const MAX_VISUAL_IMAGES: usize = 8;
pub const MAX_VISUAL_URL_CHARS: usize = 2_048;
pub const MAX_VISUAL_INSTRUCTION_CHARS: usize = 2_000;
pub const MAX_VISUAL_OUTPUT_CHARS: usize = 12_000;
pub const MAX_VISUAL_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_VISUAL_IMAGE_REDIRECTS: usize = 5;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelTaskKind {
    TextSummary,
    VisualDescription,
    Transcription,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelTaskRequest {
    pub kind: ModelTaskKind,
    pub instruction: String,
}

/// A non-sensitive reference to an image that can be passed to an online
/// multimodal model. The URL is expected to have already been redacted for
/// credentials and short-lived browser tokens.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisualImageRef {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisualTaskRequest {
    pub instruction: String,
    pub images: Vec<VisualImageRef>,
    pub max_output_chars: usize,
}

impl VisualTaskRequest {
    pub fn bounded(instruction: impl Into<String>, images: Vec<VisualImageRef>) -> Option<Self> {
        let instruction = instruction.into();
        let instruction = instruction
            .chars()
            .take(MAX_VISUAL_INSTRUCTION_CHARS)
            .collect::<String>();
        let images = images
            .into_iter()
            .filter(|image| {
                let url = image.url.trim();
                !url.is_empty() && !contains_sensitive_reference(url)
            })
            .take(MAX_VISUAL_IMAGES)
            .map(|mut image| {
                let trimmed_url = image.url.trim();
                if trimmed_url.starts_with("data:") {
                    image.url = trimmed_url.to_owned();
                } else {
                    image.url = trimmed_url.chars().take(MAX_VISUAL_URL_CHARS).collect();
                }
                image.alt = image
                    .alt
                    .map(|value| value.chars().take(240).collect::<String>());
                image
            })
            .collect::<Vec<_>>();
        if instruction.trim().is_empty() || images.is_empty() {
            return None;
        }
        Some(Self {
            instruction,
            images,
            max_output_chars: MAX_VISUAL_OUTPUT_CHARS,
        })
    }
}

fn contains_sensitive_reference(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "cookie=",
        "xsec_token=",
        "authorization=",
        "access_token=",
        "api_key=",
        "apikey=",
        "signature=",
    ]
    .iter()
    .any(|key| lower.contains(key))
        || lower.contains("user:pass@")
        || lower.contains("authorization:")
        || Url::parse(value)
            .map(|url| {
                !url.username().is_empty()
                    || url.password().is_some()
                    || url.query_pairs().any(|(key, _)| {
                        [
                            "cookie",
                            "xsec_token",
                            "authorization",
                            "access_token",
                            "api_key",
                            "apikey",
                            "signature",
                            "sign",
                        ]
                        .iter()
                        .any(|sensitive| key.eq_ignore_ascii_case(sensitive))
                    })
            })
            .unwrap_or(false)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VisualTaskResult {
    pub content: String,
    pub analyzed_images: usize,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualTaskError {
    pub code: String,
    pub message: String,
}

impl VisualTaskError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for VisualTaskError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for VisualTaskError {}

/// The desktop and future Android implementations can bridge this trait to
/// the configured online model. The research adapter never needs to know the
/// provider protocol or platform-specific WebView details.
#[async_trait]
pub trait VisualTaskExecutor: Send + Sync {
    async fn execute(
        &self,
        request: VisualTaskRequest,
    ) -> Result<VisualTaskResult, VisualTaskError>;
}

/// Explicitly reports that no visual model has been configured. This is used
/// as a safe fallback and must not imply that visual analysis was performed.
pub struct UnavailableVisualTaskExecutor;

#[async_trait]
impl VisualTaskExecutor for UnavailableVisualTaskExecutor {
    async fn execute(
        &self,
        _request: VisualTaskRequest,
    ) -> Result<VisualTaskResult, VisualTaskError> {
        Err(VisualTaskError::new(
            "visual_unavailable",
            "未配置在线视觉模型",
        ))
    }
}

/// Bridges the configured OpenAI-compatible Responses model to the
/// platform-neutral visual task contract. Images are fetched for the current
/// task only and converted to bounded data URLs; the source URL is never sent
/// to the model and is never included in an error message.
pub struct OpenAiCompatibleVisualTaskExecutor {
    client: OpenAiCompatibleClient,
    image_http: reqwest::Client,
    provider: ProviderConfig,
    api_key: String,
}

impl OpenAiCompatibleVisualTaskExecutor {
    pub fn new(provider: ProviderConfig, api_key: String) -> Result<Self, String> {
        Self::build(provider, api_key, false)
    }

    fn build(
        provider: ProviderConfig,
        api_key: String,
        allow_loopback: bool,
    ) -> Result<Self, String> {
        if !provider.enabled {
            return Err("视觉模型配置未启用".into());
        }
        if provider.model.trim().is_empty() {
            return Err("视觉模型名称不能为空".into());
        }
        if api_key.trim().is_empty() {
            return Err("视觉模型缺少 API Key".into());
        }
        let image_http = reqwest::Client::builder()
            .user_agent("PaimonDesktop/0.1")
            .timeout(std::time::Duration::from_millis(provider.timeout_ms))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= MAX_VISUAL_IMAGE_REDIRECTS {
                    return attempt.error("too many visual image redirects");
                }
                if is_allowed_image_url(attempt.url(), allow_loopback) {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .map_err(|error| format!("无法创建视觉图片客户端：{error}"))?;
        Ok(Self {
            client: OpenAiCompatibleClient::new()?,
            image_http,
            provider,
            api_key,
        })
    }

    #[cfg(test)]
    fn new_for_tests(provider: ProviderConfig, api_key: String) -> Result<Self, String> {
        Self::build(provider, api_key, true)
    }

    async fn fetch_image(
        &self,
        image: &VisualImageRef,
        index: usize,
    ) -> Result<ChatAttachment, VisualTaskError> {
        let reference = image.url.trim();
        if reference.starts_with("data:") {
            return parse_visual_data_url(reference, index);
        }
        let parsed = Url::parse(reference)
            .map_err(|_| VisualTaskError::new("visual_image_invalid", "图片地址无效"))?;
        if !is_allowed_image_url(&parsed, cfg!(test))
            || contains_sensitive_reference(parsed.as_str())
        {
            return Err(VisualTaskError::new(
                "visual_image_rejected",
                "图片地址不是可安全读取的公开 HTTP(S) 地址",
            ));
        }
        let response = self
            .image_http
            .get(parsed)
            .send()
            .await
            .map_err(|_| VisualTaskError::new("visual_image_fetch", "无法读取图片"))?;
        if !response.status().is_success() {
            return Err(VisualTaskError::new(
                "visual_image_fetch",
                "图片服务器未返回成功状态",
            ));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_VISUAL_IMAGE_BYTES as u64)
        {
            return Err(VisualTaskError::new(
                "visual_image_too_large",
                "图片超过视觉任务大小限制",
            ));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !content_type.is_empty() && !is_supported_visual_mime(&content_type) {
            return Err(VisualTaskError::new(
                "visual_image_type",
                "图片服务器返回了不支持的内容类型",
            ));
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|_| VisualTaskError::new("visual_image_fetch", "无法读取图片内容"))?;
            if bytes.len().saturating_add(chunk.len()) > MAX_VISUAL_IMAGE_BYTES {
                return Err(VisualTaskError::new(
                    "visual_image_too_large",
                    "图片超过视觉任务大小限制",
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.is_empty() {
            return Err(VisualTaskError::new(
                "visual_image_too_large",
                "图片超过视觉任务大小限制",
            ));
        }
        let mime_type = if is_supported_visual_mime(&content_type) {
            content_type
        } else {
            infer_visual_mime(&image.url).ok_or_else(|| {
                VisualTaskError::new("visual_image_type", "图片类型不是支持的格式")
            })?
        };
        let size = bytes.len() as u64;
        Ok(ChatAttachment {
            name: format!("research-image-{index}.{}", mime_extension(&mime_type)),
            mime_type: mime_type.clone(),
            data_url: format!("data:{mime_type};base64,{}", STANDARD.encode(bytes)),
            size,
            kind: AttachmentKind::Image,
        })
    }
}

fn parse_visual_data_url(value: &str, index: usize) -> Result<ChatAttachment, VisualTaskError> {
    let (mime_type, payload) = ["image/jpeg", "image/png", "image/webp"]
        .into_iter()
        .find_map(|mime_type| {
            value
                .strip_prefix(&format!("data:{mime_type};base64,"))
                .map(|payload| (mime_type, payload))
        })
        .ok_or_else(|| {
            VisualTaskError::new(
                "visual_image_type",
                "内嵌图片必须是 JPEG、PNG 或 WebP Base64 数据",
            )
        })?;
    if payload.is_empty() || payload.len() > encoded_visual_image_limit() {
        return Err(VisualTaskError::new(
            "visual_image_too_large",
            "内嵌图片为空或超过视觉任务大小限制",
        ));
    }
    if payload.len() % 4 != 0 {
        return Err(VisualTaskError::new(
            "visual_image_invalid",
            "内嵌图片 Base64 数据无效",
        ));
    }
    let bytes = STANDARD
        .decode(payload)
        .map_err(|_| VisualTaskError::new("visual_image_invalid", "内嵌图片 Base64 数据无效"))?;
    if bytes.is_empty() || bytes.len() > MAX_VISUAL_IMAGE_BYTES {
        return Err(VisualTaskError::new(
            "visual_image_too_large",
            "内嵌图片为空或超过视觉任务大小限制",
        ));
    }
    // Decoding and encoding again must yield the exact input. This rejects
    // whitespace, missing/excess padding and non-canonical trailing bits.
    if STANDARD.encode(&bytes) != payload {
        return Err(VisualTaskError::new(
            "visual_image_invalid",
            "内嵌图片 Base64 数据无效",
        ));
    }
    let size = bytes.len() as u64;
    let canonical_payload = STANDARD.encode(bytes);
    Ok(ChatAttachment {
        name: format!("research-keyframe-{index}.{}", mime_extension(mime_type)),
        mime_type: mime_type.to_owned(),
        data_url: format!("data:{mime_type};base64,{canonical_payload}"),
        size,
        kind: AttachmentKind::Image,
    })
}

fn encoded_visual_image_limit() -> usize {
    MAX_VISUAL_IMAGE_BYTES.saturating_add(2) / 3 * 4
}

#[async_trait]
impl VisualTaskExecutor for OpenAiCompatibleVisualTaskExecutor {
    async fn execute(
        &self,
        request: VisualTaskRequest,
    ) -> Result<VisualTaskResult, VisualTaskError> {
        let request =
            VisualTaskRequest::bounded(request.instruction, request.images).ok_or_else(|| {
                VisualTaskError::new("visual_request_invalid", "视觉请求为空或超出限制")
            })?;
        let mut attachments = Vec::with_capacity(request.images.len());
        for (index, image) in request.images.iter().enumerate() {
            attachments.push(self.fetch_image(image, index + 1).await?);
        }
        let analyzed_images = attachments.len();
        let instruction = sanitize_visual_instruction(&request.instruction);
        let messages = vec![
            ChatMessage::system(
                "你是网页图片事实提取器。只描述图片中可观察到的文字、数字、表格、角色面板、圣遗物、武器、配队和步骤；明确区分 OCR/页面事实与推测，不要编造不可见内容。",
            ),
            ChatMessage::user_with_attachments(instruction, attachments),
        ];
        let result = self
            .client
            .complete(&self.provider, &self.api_key, &messages, &[])
            .await
            .map_err(|_| VisualTaskError::new("visual_provider", "在线视觉模型请求失败"))?;
        let content = result
            .content
            .chars()
            .take(request.max_output_chars.min(MAX_VISUAL_OUTPUT_CHARS))
            .collect::<String>();
        if content.trim().is_empty() {
            return Err(VisualTaskError::new(
                "visual_provider_empty",
                "在线视觉模型没有返回可用内容",
            ));
        }
        Ok(VisualTaskResult {
            content,
            analyzed_images,
            warnings: Vec::new(),
        })
    }
}

fn is_supported_visual_mime(value: &str) -> bool {
    matches!(
        value,
        "image/png" | "image/jpeg" | "image/webp" | "image/gif"
    )
}

fn is_allowed_image_url(url: &Url, allow_loopback: bool) -> bool {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if allow_loopback && matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return true;
    }
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return false;
    }
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified())
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            !(ip.is_loopback() || ip.is_unspecified() || ip.is_unique_local())
        }
        Err(_) => true,
    }
}

fn sanitize_visual_instruction(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if [
        "cookie",
        "authorization",
        "xsec_token",
        "access_token",
        "api_key",
        "apikey",
        "signature",
        "bearer ",
    ]
    .iter()
    .any(|term| lower.contains(term))
    {
        "请提取图片中与用户问题相关的可观察事实。".into()
    } else {
        crate::diagnostics::redact_sensitive_text(value)
            .chars()
            .take(MAX_VISUAL_INSTRUCTION_CHARS)
            .collect()
    }
}

fn infer_visual_mime(value: &str) -> Option<String> {
    let path = Url::parse(value).ok()?.path().to_ascii_lowercase();
    [
        (".png", "image/png"),
        (".jpg", "image/jpeg"),
        (".jpeg", "image/jpeg"),
        (".webp", "image/webp"),
        (".gif", "image/gif"),
    ]
    .iter()
    .find_map(|(suffix, mime)| path.ends_with(suffix).then(|| (*mime).to_owned()))
}

fn mime_extension(value: &str) -> &'static str {
    match value {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    }
}

pub fn query_needs_visual(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    [
        "图片",
        "图中",
        "截图",
        "面板",
        "圣遗物",
        "武器",
        "配队",
        "表格",
        "ocr",
        "视觉",
        "image",
        "screenshot",
        "visual",
        "panel",
        "artifact",
        "weapon",
        "team",
    ]
    .iter()
    .any(|term| query.contains(term))
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    use super::*;

    fn provider(base_url: String) -> ProviderConfig {
        ProviderConfig {
            id: "vision-relay".into(),
            name: "Vision Relay".into(),
            base_url,
            model: "vision-model".into(),
            secret_ref: "provider:vision-relay".into(),
            timeout_ms: 2_000,
            streaming: false,
            tool_calling: false,
            enabled: true,
        }
    }

    fn read_http_request(mut stream: &std::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4_096];
        let expected_length = loop {
            let count = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..count]);
            let text = String::from_utf8_lossy(&request);
            if let Some(header_end) = text.find("\r\n\r\n") {
                let content_length = text[..header_end]
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                break header_end + 4 + content_length;
            }
        };
        while request.len() < expected_length {
            let count = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..count]);
        }
        String::from_utf8(request).unwrap()
    }

    #[test]
    fn visual_request_is_bounded_and_drops_empty_images() {
        let request = VisualTaskRequest::bounded(
            "识别图片中的圣遗物面板",
            vec![
                VisualImageRef {
                    url: "https://img.example/1.png".into(),
                    alt: Some("a".into()),
                },
                VisualImageRef {
                    url: " ".into(),
                    alt: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(request.images.len(), 1);
        assert_eq!(request.max_output_chars, MAX_VISUAL_OUTPUT_CHARS);
    }

    #[test]
    fn visual_request_rejects_sensitive_image_references() {
        let request = VisualTaskRequest::bounded(
            "识别图片",
            vec![
                VisualImageRef {
                    url: "https://img.example/a.png?xsec_token=secret".into(),
                    alt: None,
                },
                VisualImageRef {
                    url: "https://img.example/b.png".into(),
                    alt: None,
                },
            ],
        )
        .unwrap();
        assert_eq!(request.images.len(), 1);
        assert!(request.images[0].url.ends_with("b.png"));
    }

    #[test]
    fn visual_request_preserves_bounded_data_url_for_keyframes() {
        let data_url = format!("data:image/jpeg;base64,{}", STANDARD.encode(b"keyframe"));
        let request = VisualTaskRequest::bounded(
            "看画面",
            vec![VisualImageRef {
                url: data_url.clone(),
                alt: None,
            }],
        )
        .unwrap();
        assert_eq!(request.images[0].url, data_url);
    }

    #[test]
    fn visual_data_url_accepts_supported_mime_and_reencodes_canonically() {
        let attachment = parse_visual_data_url(
            &format!("data:image/webp;base64,{}", STANDARD.encode(b"frame")),
            2,
        )
        .unwrap();
        assert_eq!(attachment.name, "research-keyframe-2.webp");
        assert_eq!(attachment.mime_type, "image/webp");
        assert_eq!(attachment.size, 5);
        assert_eq!(attachment.data_url, "data:image/webp;base64,ZnJhbWU=");
    }

    #[test]
    fn visual_data_url_rejects_unsupported_mime_and_malformed_base64() {
        let unsupported = parse_visual_data_url("data:image/gif;base64,ZnJhbWU=", 1).unwrap_err();
        assert_eq!(unsupported.code, "visual_image_type");
        let malformed = parse_visual_data_url("data:image/png;base64,not-base64", 1).unwrap_err();
        assert_eq!(malformed.code, "visual_image_invalid");
        let whitespace = parse_visual_data_url("data:image/png;base64,Zn JhbWU=", 1).unwrap_err();
        assert_eq!(whitespace.code, "visual_image_invalid");
    }

    #[test]
    fn visual_data_url_rejects_non_canonical_base64_and_oversized_payload() {
        let non_canonical = parse_visual_data_url("data:image/png;base64,Zh==", 1).unwrap_err();
        assert_eq!(non_canonical.code, "visual_image_invalid");

        let oversized_payload = "A".repeat(encoded_visual_image_limit() + 4);
        let oversized =
            parse_visual_data_url(&format!("data:image/png;base64,{oversized_payload}"), 1)
                .unwrap_err();
        assert_eq!(oversized.code, "visual_image_too_large");
    }

    #[test]
    fn visual_data_url_accepts_exact_eight_megabyte_boundary() {
        let bytes = vec![0_u8; MAX_VISUAL_IMAGE_BYTES];
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        let attachment = parse_visual_data_url(&data_url, 1).unwrap();
        assert_eq!(attachment.size, MAX_VISUAL_IMAGE_BYTES as u64);
    }

    #[test]
    fn visual_intent_does_not_trigger_for_plain_text_questions() {
        assert!(!query_needs_visual("这篇笔记讲了什么？"));
        assert!(query_needs_visual("帮我看图中的配队"));
    }

    #[tokio::test]
    async fn unavailable_executor_is_explicit() {
        let executor = UnavailableVisualTaskExecutor;
        let request = VisualTaskRequest::bounded(
            "看图",
            vec![VisualImageRef {
                url: "https://img.example/a.png".into(),
                alt: None,
            }],
        )
        .unwrap();
        assert_eq!(
            executor.execute(request).await.unwrap_err().code,
            "visual_unavailable"
        );
    }

    #[test]
    fn production_image_gate_rejects_private_network_and_credentials() {
        assert!(!is_allowed_image_url(
            &Url::parse("http://127.0.0.1/private.png").unwrap(),
            false
        ));
        assert!(!is_allowed_image_url(
            &Url::parse("http://192.168.1.2/private.png").unwrap(),
            false
        ));
        assert!(!is_allowed_image_url(
            &Url::parse("https://user:password@img.example/a.png").unwrap(),
            false
        ));
        assert!(is_allowed_image_url(
            &Url::parse("https://img.example/a.png").unwrap(),
            false
        ));
    }

    #[test]
    fn visual_executor_requires_enabled_model_and_secret() {
        let mut config = provider("https://relay.example/v1".into());
        config.enabled = false;
        assert!(OpenAiCompatibleVisualTaskExecutor::new(config, "secret".into()).is_err());
        let config = provider("https://relay.example/v1".into());
        assert!(OpenAiCompatibleVisualTaskExecutor::new(config, " ".into()).is_err());
    }

    #[tokio::test]
    async fn responses_bridge_fetches_images_and_sends_only_bounded_data_urls() {
        let image_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let image_address = image_listener.local_addr().unwrap();
        let image_server = std::thread::spawn(move || {
            let (mut stream, _) = image_listener.accept().unwrap();
            let request = read_http_request(&stream);
            let bytes = b"small-png-fixture";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(bytes).unwrap();
            request
        });

        let model_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let model_address = model_listener.local_addr().unwrap();
        let model_server = std::thread::spawn(move || {
            let (mut stream, _) = model_listener.accept().unwrap();
            let request = read_http_request(&stream);
            let response = r#"{"output":[{"type":"message","content":[{"type":"output_text","text":"图片中可见角色面板"}]}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
            request
        });

        let source_url = format!("http://{image_address}/private-source.png");
        let executor = OpenAiCompatibleVisualTaskExecutor::new_for_tests(
            provider(format!("http://{model_address}/v1")),
            "provider-api-secret".into(),
        )
        .unwrap();
        let result = executor
            .execute(
                VisualTaskRequest::bounded(
                    "看面板 Cookie: cookie-secret xsec_token=page-secret",
                    vec![VisualImageRef {
                        url: source_url.clone(),
                        alt: Some("角色面板".into()),
                    }],
                )
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "图片中可见角色面板");
        assert_eq!(result.analyzed_images, 1);
        let image_request = image_server.join().unwrap();
        assert!(image_request.starts_with("GET /private-source.png"));
        let model_request = model_server.join().unwrap();
        let (_, model_body) = model_request.split_once("\r\n\r\n").unwrap();
        assert!(model_body.contains("data:image/png;base64,"));
        assert!(!model_body.contains(&source_url));
        assert!(!model_body.contains("cookie-secret"));
        assert!(!model_body.contains("page-secret"));
        assert!(!model_body.contains("provider-api-secret"));
    }

    #[tokio::test]
    async fn provider_failure_does_not_expose_source_image_url() {
        let image_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let image_address = image_listener.local_addr().unwrap();
        let image_server = std::thread::spawn(move || {
            let (mut stream, _) = image_listener.accept().unwrap();
            let _ = read_http_request(&stream);
            let bytes = b"image";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: image/png\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(bytes).unwrap();
        });
        let model_listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let model_address = model_listener.local_addr().unwrap();
        let model_server = std::thread::spawn(move || {
            let (mut stream, _) = model_listener.accept().unwrap();
            let _ = read_http_request(&stream);
            let response = r#"{"error":{"message":"upstream unavailable"}}"#;
            write!(
                stream,
                "HTTP/1.1 503 Service Unavailable\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response.len(),
                response
            )
            .unwrap();
        });
        let source_url = format!("http://{image_address}/sensitive-path.png");
        let executor = OpenAiCompatibleVisualTaskExecutor::new_for_tests(
            provider(format!("http://{model_address}/v1")),
            "api-secret".into(),
        )
        .unwrap();
        let error = executor
            .execute(
                VisualTaskRequest::bounded(
                    "看图",
                    vec![VisualImageRef {
                        url: source_url.clone(),
                        alt: None,
                    }],
                )
                .unwrap(),
            )
            .await
            .unwrap_err();
        image_server.join().unwrap();
        model_server.join().unwrap();
        assert_eq!(error.code, "visual_provider");
        assert!(!error.message.contains(&source_url));
        assert!(!error.message.contains("api-secret"));
    }
}
