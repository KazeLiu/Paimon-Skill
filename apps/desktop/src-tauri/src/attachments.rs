use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

pub const MAX_ATTACHMENT_COUNT: usize = 6;
pub const MAX_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_TOTAL_ATTACHMENT_BYTES: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentKind {
    Image,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatAttachment {
    pub name: String,
    pub mime_type: String,
    pub data_url: String,
    pub size: u64,
    pub kind: AttachmentKind,
}

impl ChatAttachment {
    #[cfg(test)]
    pub fn image(
        name: impl Into<String>,
        mime_type: impl Into<String>,
        data_url: impl Into<String>,
        size: u64,
    ) -> Self {
        Self {
            name: name.into(),
            mime_type: mime_type.into(),
            data_url: data_url.into(),
            size,
            kind: AttachmentKind::Image,
        }
    }

    #[cfg(test)]
    pub fn file(
        name: impl Into<String>,
        mime_type: impl Into<String>,
        data_url: impl Into<String>,
        size: u64,
    ) -> Self {
        Self {
            name: name.into(),
            mime_type: mime_type.into(),
            data_url: data_url.into(),
            size,
            kind: AttachmentKind::File,
        }
    }
}

pub fn validate_attachments(attachments: &[ChatAttachment]) -> Result<(), String> {
    if attachments.len() > MAX_ATTACHMENT_COUNT {
        return Err(format!("最多添加 {MAX_ATTACHMENT_COUNT} 个附件"));
    }
    let mut total = 0u64;
    for attachment in attachments {
        if attachment.name.trim().is_empty() {
            return Err("附件名称不能为空".into());
        }
        if attachment.mime_type.trim().is_empty() {
            return Err(format!("附件缺少 MIME 类型：{}", attachment.name));
        }
        if attachment.size > MAX_ATTACHMENT_BYTES {
            return Err(format!("单个附件不能超过 10 MB：{}", attachment.name));
        }
        if matches!(attachment.kind, AttachmentKind::Image)
            && !supported_image_mime(&attachment.mime_type)
        {
            return Err(format!("图片附件类型无效：{}", attachment.name));
        }
        if matches!(attachment.kind, AttachmentKind::File)
            && (!supported_file_name(&attachment.name)
                || !supported_file_mime(&attachment.mime_type))
        {
            return Err(format!("暂不支持该文件类型：{}", attachment.name));
        }
        let prefix = format!("data:{};base64,", attachment.mime_type);
        let payload = attachment
            .data_url
            .strip_prefix(&prefix)
            .ok_or_else(|| format!("附件数据格式无效：{}", attachment.name))?;
        if payload.len() > encoded_limit(MAX_ATTACHMENT_BYTES) {
            return Err(format!("单个附件不能超过 10 MB：{}", attachment.name));
        }
        let decoded = STANDARD
            .decode(payload)
            .map_err(|_| format!("附件 Base64 数据无效：{}", attachment.name))?;
        let decoded_size = decoded.len() as u64;
        if decoded_size > MAX_ATTACHMENT_BYTES {
            return Err(format!("单个附件不能超过 10 MB：{}", attachment.name));
        }
        if attachment.size != decoded_size {
            return Err(format!("附件大小与数据不一致：{}", attachment.name));
        }
        total = total.saturating_add(decoded_size);
        if total > MAX_TOTAL_ATTACHMENT_BYTES {
            return Err("附件总大小不能超过 20 MB".into());
        }
    }
    Ok(())
}

fn supported_file_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "application/pdf"
            | "text/plain"
            | "text/markdown"
            | "text/csv"
            | "application/json"
            | "application/xml"
            | "text/xml"
            | "application/yaml"
            | "text/yaml"
            | "application/msword"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/vnd.ms-powerpoint"
            | "application/vnd.openxmlformats-officedocument.presentationml.presentation"
            | "application/vnd.ms-excel"
            | "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    )
}

fn supported_image_mime(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/png" | "image/jpeg" | "image/webp" | "image/gif"
    )
}

fn supported_file_name(name: &str) -> bool {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    matches!(
        extension.as_deref(),
        Some(
            "pdf"
                | "txt"
                | "md"
                | "csv"
                | "json"
                | "xml"
                | "yaml"
                | "yml"
                | "doc"
                | "docx"
                | "ppt"
                | "pptx"
                | "xls"
                | "xlsx"
        )
    )
}

fn encoded_limit(decoded_limit: u64) -> usize {
    decoded_limit
        .saturating_add(2)
        .saturating_div(3)
        .saturating_mul(4) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_data_url_and_image_mime_type() {
        assert!(validate_attachments(&[ChatAttachment::image(
            "screen.png",
            "image/png",
            "data:image/png;base64,YQ==",
            1,
        )])
        .is_ok());
        assert!(validate_attachments(&[ChatAttachment::image(
            "fake.png",
            "text/plain",
            "data:text/plain;base64,YQ==",
            1,
        )])
        .unwrap_err()
        .contains("图片附件类型无效"));
        assert!(validate_attachments(&[ChatAttachment::image(
            "vector.svg",
            "image/svg+xml",
            "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
            10,
        )])
        .unwrap_err()
        .contains("图片附件类型无效"));
        assert!(validate_attachments(&[ChatAttachment::file(
            "invalid.txt",
            "text/plain",
            "data:text/plain;base64,====",
            1,
        )])
        .unwrap_err()
        .contains("Base64"));
    }
}
