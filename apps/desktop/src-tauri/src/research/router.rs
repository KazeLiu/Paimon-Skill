use url::Url;

use super::models::{ResearchError, ResearchRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    Generic,
    Zhihu,
    Xiaohongshu,
    Bilibili,
}

pub fn route(request: &ResearchRequest) -> Result<AdapterKind, ResearchError> {
    let url = Url::parse(&request.url)
        .map_err(|error| ResearchError::new("invalid_url", format!("网址格式无效：{error}")))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let source = request
        .source_id
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if source == "zhihu" || host.ends_with("zhihu.com") {
        return Ok(AdapterKind::Zhihu);
    }
    if source == "xiaohongshu" || host.ends_with("xiaohongshu.com") || host.ends_with("xhslink.com")
    {
        return Ok(AdapterKind::Xiaohongshu);
    }
    if source == "bilibili" || source == "bilibili-video" || host.ends_with("bilibili.com") {
        return Ok(AdapterKind::Bilibili);
    }
    Ok(AdapterKind::Generic)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(source_id: Option<&str>, url: &str) -> ResearchRequest {
        ResearchRequest {
            source_id: source_id.map(str::to_owned),
            url: url.into(),
            query: "测试".into(),
            wants_comments: false,
            wants_visual: false,
        }
    }

    #[test]
    fn routes_platforms_by_source_id_or_host() {
        assert_eq!(
            route(&request(None, "https://www.zhihu.com/question/1")).unwrap(),
            AdapterKind::Zhihu
        );
        assert_eq!(
            route(&request(Some("xiaohongshu"), "https://example.com/note")).unwrap(),
            AdapterKind::Xiaohongshu
        );
        assert_eq!(
            route(&request(None, "https://www.bilibili.com/video/BV1")).unwrap(),
            AdapterKind::Bilibili
        );
        assert_eq!(
            route(&request(None, "https://example.com/article")).unwrap(),
            AdapterKind::Generic
        );
    }

    #[test]
    fn rejects_malformed_urls_before_adapter_selection() {
        assert_eq!(
            route(&request(None, "not a url")).unwrap_err().code,
            "invalid_url"
        );
    }
}
