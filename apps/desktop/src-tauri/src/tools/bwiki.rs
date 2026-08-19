use chrono::Utc;
use reqwest::Client;
use serde_json::Value;

use super::{
    models::{PageLink, SourceRecord, WebPageResult},
    static_page::extract_html,
};

const API_URL: &str = "https://wiki.biligame.com/ys/api.php";

pub async fn search(client: &Client, query: &str) -> Result<WebPageResult, String> {
    let response = client
        .get(API_URL)
        .query(&[
            ("action", "opensearch"),
            ("search", query),
            ("limit", "8"),
            ("namespace", "0"),
            ("format", "json"),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| format!("BWiki 搜索失败：{error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("BWiki 搜索返回 HTTP {}", status.as_u16()));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("BWiki 搜索响应无效：{error}"))?;
    let titles = value
        .get(1)
        .and_then(Value::as_array)
        .ok_or_else(|| "BWiki 搜索缺少标题列表".to_string())?;
    let urls = value
        .get(3)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut content = format!("搜索词：{query}\n\n");
    let mut links = Vec::new();
    for (index, title) in titles.iter().enumerate() {
        let Some(title) = title.as_str() else {
            continue;
        };
        let url = urls.get(index).and_then(Value::as_str).unwrap_or_default();
        content.push_str(&format!("- {title}\n"));
        if !url.is_empty() {
            links.push(PageLink {
                title: title.into(),
                url: url.into(),
            });
        }
    }
    Ok(WebPageResult {
        source: SourceRecord {
            title: format!("BWiki 搜索：{query}"),
            url: format!("https://wiki.biligame.com/ys/特殊:搜索?search={query}"),
            source_level: "community_wiki".into(),
            platform: "BWiki".into(),
            captured_at: Utc::now().to_rfc3339(),
        },
        content,
        links,
    })
}

pub async fn read_page(client: &Client, title: &str) -> Result<WebPageResult, String> {
    let response = client
        .get(API_URL)
        .query(&[
            ("action", "parse"),
            ("page", title),
            ("prop", "text|displaytitle|revid"),
            ("format", "json"),
        ])
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| format!("BWiki 页面读取失败：{error}"))?;
    let value: Value = response
        .json()
        .await
        .map_err(|error| format!("BWiki 页面响应无效：{error}"))?;
    let html = value
        .pointer("/parse/text/*")
        .and_then(Value::as_str)
        .ok_or_else(|| "BWiki 页面没有正文".to_string())?;
    let url = format!("https://wiki.biligame.com/ys/{title}");
    let mut result = extract_html(
        &url,
        &format!("<title>{title}</title><body>{html}</body>"),
        "community_wiki",
        "BWiki",
    )?;
    result.source.title = title.into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn recognizes_mediawiki_search_shape() {
        let value = json!([
            "胡桃",
            ["胡桃", "胡桃/攻略"],
            [],
            [
                "https://wiki.biligame.com/ys/胡桃",
                "https://wiki.biligame.com/ys/胡桃/攻略"
            ]
        ]);
        assert_eq!(value[1].as_array().unwrap().len(), 2);
        assert_eq!(value[3][0], "https://wiki.biligame.com/ys/胡桃");
    }
}
