use url::Url;

const BLOCKED_CLICK_TERMS: &[&str] = &[
    "登录",
    "注册",
    "发布",
    "发送",
    "提交",
    "保存",
    "删除",
    "购买",
    "支付",
    "下单",
    "确认订单",
    "点赞",
    "投币",
    "收藏",
    "关注",
    "订阅",
    "分享",
    "转发",
    "举报",
    "退出",
    "授权",
    "同意",
    "login",
    "sign in",
    "sign up",
    "publish",
    "post",
    "submit",
    "save",
    "delete",
    "remove",
    "buy",
    "purchase",
    "pay",
    "like",
    "follow",
    "subscribe",
    "share",
    "send",
    "confirm",
    "authorize",
    "download",
];

pub fn normalize_browser_url(input: &str) -> Result<Url, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("网址不能为空".into());
    }

    let candidate = if input.contains("://")
        || input.starts_with("javascript:")
        || input.starts_with("file:")
        || input.starts_with("data:")
        || input.starts_with("about:")
    {
        input.to_string()
    } else {
        format!("https://{input}")
    };
    let url = Url::parse(&candidate).map_err(|error| format!("网址格式无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("浏览器只允许打开 HTTP 或 HTTPS 网页".into());
    }
    if url.host_str().is_none() {
        return Err("网址缺少主机名".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("网址不能包含用户名或密码".into());
    }
    Ok(url)
}

pub fn validate_source_connection_url(input: &str) -> Result<Url, String> {
    const ALLOWED_HOSTS: &[&str] = &[
        "baike.mihoyo.com",
        "wiki.biligame.com",
        "genshin-impact.fandom.com",
        "ambr.top",
        "www.xiaohongshu.com",
        "www.zhihu.com",
        "www.reddit.com",
        "www.bilibili.com",
    ];
    let url = normalize_browser_url(input)?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !ALLOWED_HOSTS.contains(&host.as_str()) {
        return Err("连接检测仅允许固定适配的信息源".into());
    }
    Ok(url)
}

pub fn validate_click_target(target: &str) -> Result<String, String> {
    let target = target.trim();
    let length = target.chars().count();
    if length == 0 || length > 80 {
        return Err("点击目标文本长度必须在 1 到 80 个字符之间".into());
    }

    let normalized = target.to_lowercase();
    if BLOCKED_CLICK_TERMS
        .iter()
        .any(|term| normalized.contains(term))
    {
        return Err("只读浏览器拒绝执行登录、发布、提交或其他有副作用的点击".into());
    }
    Ok(target.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_url_defaults_to_https() {
        assert_eq!(
            normalize_browser_url("example.com/path").unwrap().as_str(),
            "https://example.com/path"
        );
    }

    #[test]
    fn browser_url_rejects_non_web_protocols_and_credentials() {
        assert!(normalize_browser_url("file:///C:/secret.txt").is_err());
        assert!(normalize_browser_url("javascript:alert(1)").is_err());
        assert!(normalize_browser_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn read_only_click_allows_expansion_and_comments() {
        assert!(validate_click_target("展开更多").is_ok());
        assert!(validate_click_target("查看全部评论").is_ok());
    }

    #[test]
    fn read_only_click_rejects_side_effect_actions() {
        for label in ["登录", "立即发布", "删除评论", "提交订单", "点赞"] {
            assert!(
                validate_click_target(label).is_err(),
                "{label} should be rejected"
            );
        }
    }

    #[test]
    fn source_connection_only_allows_fixed_source_hosts() {
        assert!(validate_source_connection_url("https://www.zhihu.com/").is_ok());
        assert!(validate_source_connection_url("https://127.0.0.1/").is_err());
        assert!(validate_source_connection_url("https://example.com/").is_err());
    }
}
