use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

pub fn validate_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw.trim()).map_err(|error| format!("网页地址无效：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("网页工具只允许 HTTP 或 HTTPS 地址".into());
    }
    if url.username() != "" || url.password().is_some() {
        return Err("网页地址不能包含用户名或密码".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "网页地址缺少主机名".to_string())?
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    if host == "localhost"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host == "metadata.google.internal"
        || host == "169.254.169.254"
    {
        return Err("网页工具拒绝访问本机或云元数据地址".into());
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_or_reserved(ip) {
            return Err("网页工具拒绝访问内网或保留地址".into());
        }
    }
    Ok(url)
}

fn is_private_or_reserved(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.octets() == [100, 64, 0, 1]
        }
        IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unspecified() || is_unique_local(ip) || is_link_local_v6(ip)
        }
    }
}

fn is_unique_local(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xfe00) == 0xfc00
}

fn is_link_local_v6(ip: Ipv6Addr) -> bool {
    (ip.segments()[0] & 0xffc0) == 0xfe80
}

#[allow(dead_code)]
fn _keep_ip_types(_: Ipv4Addr) {}

#[cfg(test)]
mod tests {
    use super::validate_url;

    #[test]
    fn allows_public_http_urls() {
        assert!(validate_url("https://example.com/article").is_ok());
    }

    #[test]
    fn blocks_protocol_credentials_and_private_targets() {
        for url in [
            "file:///etc/passwd",
            "http://user:pass@example.com/",
            "http://localhost:8080/",
            "http://127.0.0.1:11434/",
            "http://10.0.0.2/",
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]/",
            "http://[fd00::1]/",
        ] {
            assert!(validate_url(url).is_err(), "should reject {url}");
        }
    }
}
