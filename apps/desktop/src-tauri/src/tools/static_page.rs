use chrono::Utc;
use reqwest::header::CONTENT_TYPE;
use scraper::{Html, Selector};

use super::{
    models::{PageLink, SourceRecord, WebPageResult},
    url_policy::validate_url,
};

const MAX_RESPONSE_BYTES: usize = 2_000_000;
const MAX_CONTENT_CHARS: usize = 120_000;

pub fn extract_html(
    raw_url: &str,
    html: &str,
    source_level: &str,
    platform: &str,
) -> Result<WebPageResult, String> {
    let url = validate_url(raw_url)?;
    let document = Html::parse_document(html);
    let title_selector = Selector::parse("title").expect("valid title selector");
    let body_selector = Selector::parse("body").expect("valid body selector");
    let semantic_selector =
        Selector::parse("article, main, h1, h2, h3, h4, h5, h6, p, li, blockquote, pre, td, th")
            .expect("valid semantic selector");
    let link_selector = Selector::parse("a[href]").expect("valid link selector");

    let title = document
        .select(&title_selector)
        .next()
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .map(|title| clean_text(&title))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| url.host_str().unwrap_or("网页").to_owned());

    let semantic_text = document
        .select(&semantic_selector)
        .map(|node| node.text().collect::<Vec<_>>().join(" "))
        .map(|text| clean_text(&text))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    let content = if semantic_text.is_empty() {
        document
            .select(&body_selector)
            .next()
            .map(|body| body.text().collect::<Vec<_>>().join(" "))
            .map(|text| clean_text(&text))
            .unwrap_or_default()
    } else {
        semantic_text.join("\n")
    };
    let content = truncate(&content, MAX_CONTENT_CHARS);

    let links = document
        .select(&link_selector)
        .filter_map(|node| {
            let href = node.value().attr("href")?;
            let absolute = url.join(href).ok()?;
            validate_url(absolute.as_str()).ok()?;
            let title = clean_text(&node.text().collect::<Vec<_>>().join(" "));
            if title.is_empty() {
                None
            } else {
                Some(PageLink {
                    title,
                    url: absolute.to_string(),
                })
            }
        })
        .take(100)
        .collect();

    Ok(WebPageResult {
        source: SourceRecord {
            title,
            url: url.to_string(),
            source_level: source_level.into(),
            platform: platform.into(),
            captured_at: Utc::now().to_rfc3339(),
        },
        content,
        links,
    })
}

pub async fn read_url(
    client: &reqwest::Client,
    raw_url: &str,
    source_level: &str,
    platform: &str,
) -> Result<WebPageResult, String> {
    let url = validate_url(raw_url)?;
    let response = client
        .get(url.clone())
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|error| format!("网页请求失败：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("网页返回 HTTP {}", status.as_u16()));
    }
    if let Some(content_type) = response.headers().get(CONTENT_TYPE) {
        let content_type = content_type.to_str().unwrap_or_default();
        if !content_type.contains("text/html") && !content_type.contains("text/plain") {
            return Err(format!("网页内容类型不受支持：{content_type}"));
        }
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| format!("读取网页失败：{error}"))?;
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err("网页响应超过 2 MB 限制".into());
    }
    let html = String::from_utf8_lossy(&bytes);
    extract_html(url.as_str(), &html, source_level, platform)
}

fn clean_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::extract_html;

    #[test]
    fn extracts_title_visible_text_and_absolute_links() {
        let result = extract_html(
            "https://example.com/wiki",
            "<html><head><title>测试页</title><style>hidden</style></head><body><h1>标题</h1><script>bad()</script><p>正文 内容</p><a href=\"/next\">下一页</a></body></html>",
            "community_wiki",
            "example",
        )
        .unwrap();

        assert_eq!(result.source.title, "测试页");
        assert!(result.content.contains("标题"));
        assert!(result.content.contains("正文 内容"));
        assert!(!result.content.contains("bad()"));
        assert_eq!(result.links[0].url, "https://example.com/next");
    }
}
