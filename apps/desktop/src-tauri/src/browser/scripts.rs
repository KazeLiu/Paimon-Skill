pub const EXTRACT_VISIBLE_PAGE_SCRIPT: &str = r#"
(() => {
  const isVisible = (element) => {
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0
      && rect.width > 0 && rect.height > 0;
  };
  const main = document.querySelector('main, article, [role="main"]') || document.body;
  const text = Array.from(main.querySelectorAll('h1,h2,h3,h4,p,li,blockquote,pre,td,th'))
    .filter(isVisible)
    .map((element) => element.innerText?.trim())
    .filter(Boolean)
    .join('\n');
  const links = Array.from(main.querySelectorAll('a[href]'))
    .filter(isVisible)
    .slice(0, 150)
    .map((anchor) => ({
      text: (anchor.innerText || anchor.getAttribute('aria-label') || '').trim().slice(0, 200),
      url: anchor.href,
    }))
    .filter((link) => link.text && /^https?:/i.test(link.url));
  const bodyText = (document.body?.innerText || '').slice(0, 12000).toLowerCase();
  const challengeTerms = ['验证码', '安全验证', '请先登录', '扫码登录', 'verify you are human', 'captcha', 'sign in to continue'];
  const challengeReason = challengeTerms.find((term) => bodyText.includes(term)) || null;
  return {
    untrustedPageContent: true,
    title: document.title || location.hostname,
    url: location.href,
    text: text || main.innerText || '',
    links,
    loginChallenge: Boolean(challengeReason),
    challengeReason,
  };
})()
"#;

pub const SCROLL_SAMPLE_SCRIPT: &str = r#"
(() => {
  const root = document.scrollingElement || document.documentElement;
  window.scrollBy({ top: Math.max(window.innerHeight * 0.82, 420), behavior: 'auto' });
  return {
    scrollY: root.scrollTop,
    scrollHeight: root.scrollHeight,
    viewportHeight: root.clientHeight,
  };
})()
"#;

pub fn click_visible_target_script(target: &str) -> Result<String, String> {
    let json = serde_json::to_vec(target).map_err(|error| format!("无法编码点击目标：{error}"))?;
    let hex = json
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(
        r#"(() => {{
  const bytes = Uint8Array.from('{hex}'.match(/../g), (value) => parseInt(value, 16));
  const target = JSON.parse(new TextDecoder().decode(bytes));
  const normalize = (value) => (value || '').replace(/\s+/g, ' ').trim();
  const isVisible = (element) => {{
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.display !== 'none' && style.visibility !== 'hidden' && rect.width > 0 && rect.height > 0;
  }};
  const candidates = Array.from(document.querySelectorAll('button,a,[role="button"],summary'))
    .filter(isVisible)
    .map((element) => ({{
      element,
      label: normalize(element.innerText || element.getAttribute('aria-label') || element.getAttribute('title')),
    }}));
  const normalizedTarget = normalize(target).toLowerCase();
  const match = candidates.find((item) => item.label.toLowerCase() === normalizedTarget)
    || candidates.find((item) => item.label.toLowerCase().includes(normalizedTarget));
  if (!match) return {{ clicked: false, error: '未找到可见的只读点击目标' }};
  const element = match.element;
  if (element.matches('[disabled],[aria-disabled="true"],input[type="submit"],button[type="submit"]')) {{
    return {{ clicked: false, error: '目标是禁用项或提交控件' }};
  }}
  element.scrollIntoView({{ block: 'center', behavior: 'auto' }});
  element.click();
  return {{ clicked: true, label: match.label }};
}})()"#
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_script_uses_serialized_target_instead_of_string_interpolation() {
        let script = click_visible_target_script("a\"; window.evil = true; //").unwrap();

        assert!(script.contains("JSON.parse"));
        assert!(!script.contains("window.evil = true"));
    }

    #[test]
    fn extraction_script_marks_page_content_as_untrusted() {
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("untrustedPageContent"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("innerText"));
    }
}
