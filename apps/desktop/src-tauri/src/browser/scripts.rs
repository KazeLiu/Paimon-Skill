pub const SEMANTIC_CONTEXT_BOOTSTRAP: &str = r#"
(() => {
  if (window.__paimonSemanticContext) return;
  const pageSession = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  const state = { generation: 0, nextId: 0 };
  const normalize = (value, limit = 240) => String(value || '').replace(/\s+/g, ' ').trim().slice(0, limit);
  const roots = (root) => {
    const result = [root];
    for (let i = 0; i < result.length; i += 1) {
      for (const element of result[i].querySelectorAll?.('*') || []) if (element.shadowRoot) result.push(element.shadowRoot);
    }
    return result;
  };
  const all = (root, selector = '*') => roots(root).flatMap((item) => [...(item.querySelectorAll?.(selector) || [])]);
  const visible = (element) => {
    if (!element || element.getAttribute?.('aria-hidden') === 'true') return false;
    const style = getComputedStyle(element); const rect = element.getBoundingClientRect();
    return style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0 && rect.width > 0 && rect.height > 0;
  };
  const text = (element) => normalize([...element.childNodes || []].filter((node) => node.nodeType === Node.TEXT_NODE).map((node) => node.textContent).join(' '));
  const title = (element) => normalize(element.getAttribute?.('title'));
  const label = (element) => {
    const labelled = String(element.getAttribute?.('aria-labelledby') || '').split(/\s+/).filter(Boolean).map((id) => document.getElementById(id)?.innerText).filter(Boolean).join(' ');
    const field = element.matches?.('input,textarea,select,[contenteditable="true"]') ? element : element.querySelector?.('input,textarea,select,[contenteditable="true"]');
    const fieldLabel = field?.labels?.[0]?.innerText || field?.closest?.('label')?.innerText || '';
    return normalize(element.getAttribute?.('aria-label') || labelled || fieldLabel || text(element) || element.innerText || field?.getAttribute?.('placeholder') || title(element));
  };
  const interactive = (element) => element.matches?.('a[href],button,input,textarea,select,summary,[role="button"],[role="link"],[role="checkbox"],[role="radio"],[role="combobox"],[role="option"],[role="menuitem"],[role="tab"],[role="switch"],[contenteditable="true"],[tabindex]:not([tabindex="-1"])') || (['div','span','li','label'].includes(String(element.tagName || '').toLowerCase()) && (getComputedStyle(element).cursor === 'pointer' || Boolean(element.getAttribute('aria-label'))));
  const kind = (element) => {
    if (element.matches('input,textarea,select,[contenteditable="true"]')) return element.isContentEditable ? 'contenteditable' : (element.tagName.toLowerCase() === 'textarea' ? 'textarea' : element.tagName.toLowerCase() === 'select' ? 'select' : 'input');
    if (element.matches('a[href],[role="link"]')) return 'link';
    if (element.matches('button,[role="button"]')) return 'button';
    if (element.matches('[role="checkbox"]')) return 'checkbox';
    if (element.matches('[role="radio"]')) return 'radio';
    if (element.matches('[role="switch"]')) return 'switch';
    if (element.matches('[role="tab"]')) return 'tab';
    if (element.matches('[role="menuitem"]')) return 'menuitem';
    return 'action';
  };
  const id = (element) => {
    const current = element.getAttribute('data-paimon-id');
    if (current && current.startsWith(`paimon-${pageSession}-${state.generation}-`)) return current;
    const next = `paimon-${pageSession}-${state.generation}-${++state.nextId}`; element.setAttribute('data-paimon-id', next); return next;
  };
  const stateOf = (element) => {
    const result = {};
    if (element.disabled || element.getAttribute('aria-disabled') === 'true') result.disabled = true;
    if (element.checked || element.getAttribute('aria-checked') === 'true') result.checked = true;
    if (element.getAttribute('aria-selected') === 'true') result.selected = true;
    if (element.getAttribute('aria-expanded') === 'true') result.expanded = true;
    return Object.keys(result).length ? result : undefined;
  };
  const node = (element, depth, budget) => {
    if (!visible(element) || budget.count >= budget.maxNodes || depth > budget.maxDepth) { budget.truncated = true; return null; }
    const tag = element.tagName.toLowerCase(); const isAction = interactive(element);
    let type = isAction ? kind(element) : '';
    if (!type && element.matches('form,[role="form"]')) type = 'form';
    else if (!type && element.matches('table,[role="grid"]')) type = 'table';
    else if (!type && element.matches('tr,[role="row"]')) type = 'table_row';
    else if (!type && element.matches('td,th,[role="gridcell"],[role="columnheader"]')) type = 'table_cell';
    else if (!type && element.matches('ul,ol,[role="list"]')) type = 'list';
    else if (!type && element.matches('li,[role="listitem"]')) type = 'list_item';
    else if (!type && element.matches('h1,h2,h3,h4,h5,h6,[role="heading"]')) type = 'heading';
    else if (!type && element.matches('p,blockquote,pre,[role="status"]')) type = 'text_block';
    else if (!type && element.matches('main,article,section,nav,aside,[role="main"],[role="region"],[role="dialog"]')) type = 'region';
    else if (!type && (text(element) || title(element) || element.children.length > 1)) type = 'container';
    if (!type) return null;
    budget.count += 1;
    const result = { type };
    if (isAction) result.elementId = id(element);
    const name = label(element); if (name && !['input','textarea','select','contenteditable'].includes(type)) result.name = name;
    const direct = text(element); if (direct && direct !== name) result.text = direct;
    const field = element.matches('input,textarea,select,[contenteditable="true"]') ? element : element.querySelector?.('input,textarea,select,[contenteditable="true"]');
    if (field?.getAttribute('placeholder')) result.placeholder = normalize(field.getAttribute('placeholder'));
    if (field?.getAttribute('type') && field.getAttribute('type') !== 'text') result.inputType = field.getAttribute('type');
    const currentState = stateOf(field || element); if (currentState) result.state = currentState;
    if (element.getAttribute('role')) result.role = element.getAttribute('role');
    const children = [];
    if (direct && !isAction) children.push({ type: 'text', value: direct });
    for (const child of [...element.children, ...(element.shadowRoot ? [...element.shadowRoot.children] : [])]) {
      const item = node(child, depth + 1, budget); if (item) children.push(item);
      if (budget.count >= budget.maxNodes) break;
    }
    if (children.length) result.children = children;
    return result;
  };
  const hash = (value) => { let result = 2166136261; for (const char of String(value || '')) result = Math.imul(result ^ char.charCodeAt(0), 16777619); return (result >>> 0).toString(16); };
  const fingerprint = () => ({ url: location.href, title: document.title, textHash: hash(normalize(document.body?.innerText, 6000)), interactiveCount: all(document, '*').filter((item) => visible(item) && interactive(item)).length, scrollY: Number(scrollY || 0) });
  const challenge = () => { const body = normalize(document.body?.innerText, 12000).toLowerCase(); return ['验证码','安全验证','请先登录','扫码登录','captcha','verify you are human','sign in to continue'].find((term) => body.includes(term)) || null; };
  const scan = () => {
    state.generation += 1; state.nextId = 0;
    const main = document.querySelector('main,article,[role="main"]') || document.body; const budget = { maxNodes: 640, maxDepth: 24, count: 0, truncated: false };
    const tree = { type: 'document', name: normalize(document.title || location.hostname) }; const children = [];
    for (const child of [...main.children]) { const item = node(child, 0, budget); if (item) children.push(item); if (budget.count >= budget.maxNodes) break; }
    if (children.length) tree.children = children; if (budget.truncated) tree.truncated = true;
    const links = all(main, 'a[href]').filter(visible).slice(0, 150).map((anchor) => ({ text: normalize(anchor.innerText || anchor.getAttribute('aria-label') || anchor.getAttribute('title'), 200), url: anchor.href })).filter((link) => link.text && /^https?:/i.test(link.url));
    const textNodes = all(main, 'h1,h2,h3,h4,h5,h6,p,li,blockquote,pre,td,th').filter(visible).map((item) => normalize(item.innerText)).filter(Boolean);
    return { untrustedPageContent: true, title: document.title || location.hostname, url: location.href, text: [...new Set(textNodes)].join('\n') || main.innerText || '', links, loginChallenge: Boolean(challenge()), challengeReason: challenge(), structure: { tree }, snapshotGeneration: state.generation, pageFingerprint: fingerprint(), interactiveTotal: all(document, '*').filter((item) => visible(item) && interactive(item)).length, shadowRootCount: roots(document).length - 1, truncated: budget.truncated };
  };
  const searchable = (element) => ([
    ['text', text(element) || element.innerText],
    ['label', label(element)],
    ['title', title(element)],
    ['placeholder', element.getAttribute('placeholder')],
  ]).map(([field, value]) => ({ field, value: normalize(value, 320) })).filter((item) => item.value);
  const context = (element) => {
    const ancestors = []; let current = element.parentElement;
    while (current && ancestors.length < 4) {
      if (visible(current) && current.matches('article,section,li,tr,form,[role="dialog"],[role="region"],[role="listitem"],[role="row"],main')) {
        ancestors.push({ type: current.matches('article,section,main,[role="dialog"],[role="region"]') ? 'region' : current.matches('li,[role="listitem"]') ? 'list_item' : current.matches('tr,[role="row"]') ? 'table_row' : 'form', name: label(current), text: normalize(text(current), 320) });
      }
      current = current.parentElement;
    }
    return { type: 'context', text: normalize(element.parentElement?.innerText, 480), ancestors };
  };
  const find = (query) => { const q = normalize(query, 300).toLowerCase(); if (!q) throw new Error('查找内容不能为空'); const matches = all(document, '*').filter((item) => visible(item)).map((item) => { const values = searchable(item); const exact = values.filter(({ value }) => value.toLowerCase() === q); const partial = values.filter(({ value }) => value.toLowerCase().includes(q)); const selected = exact.length ? exact : partial; if (!selected.length) return null; const target = interactive(item) ? item : item.closest?.('button,a,[role="button"],[role="link"]'); return { score: exact.length ? 1000 : 600, element: target || item, text: selected[0].value, matchedIn: [...new Set(selected.map(({ field }) => field))] }; }).filter(Boolean).sort((a,b) => b.score - a.score).slice(0, 24); return { url: location.href, title: document.title, query: normalize(query), matchCount: matches.length, returnedMatches: matches.length, matches: matches.map((item,index) => ({ index: index + 1, score: item.score, matchedIn: item.matchedIn, match: { type: kind(item.element), elementId: interactive(item.element) ? id(item.element) : undefined, name: label(item.element), text: item.text }, relation: context(item.element) })), snapshotGeneration: state.generation, pageFingerprint: fingerprint() }; };
  window.__paimonSemanticContext = { scan, find, resolve: (value) => {
    const prefix = `paimon-${pageSession}-${state.generation}-`;
    if (!String(value || '').startsWith(prefix)) return null;
    return all(document, '[data-paimon-id]').find((item) => item.getAttribute('data-paimon-id') === value);
  } };
})()
"#;

pub const EXTRACT_VISIBLE_PAGE_SCRIPT: &str = SEMANTIC_CONTEXT_BOOTSTRAP;

pub fn semantic_scan_script() -> String {
    format!(
        "{}\nwindow.__paimonSemanticContext.scan()",
        SEMANTIC_CONTEXT_BOOTSTRAP
    )
}

pub fn find_page_context_script(query: &str) -> Result<String, String> {
    let json =
        serde_json::to_string(query).map_err(|error| format!("无法编码查找内容：{error}"))?;
    Ok(format!(
        "{}\nwindow.__paimonSemanticContext.find({json})",
        SEMANTIC_CONTEXT_BOOTSTRAP
    ))
}

pub fn click_element_script(element_id: &str) -> Result<String, String> {
    let json = serde_json::to_string(element_id)
        .map_err(|error| format!("无法编码页面元素 ID：{error}"))?;
    Ok(format!("{}\n(() => {{ const id = {json}; const element = window.__paimonSemanticContext.resolve(id); if (!element) return {{ clicked: false, error: '页面元素 ID 已过期或不存在' }}; const style = getComputedStyle(element); const rect = element.getBoundingClientRect(); if (style.display === 'none' || style.visibility === 'hidden' || !rect.width || !rect.height) return {{ clicked: false, error: '页面元素当前不可见' }}; if (element.matches('input,textarea,select,[contenteditable=\"true\"],[disabled],[aria-disabled=\"true\"],button[type=\"submit\"]')) return {{ clicked: false, error: '为了保护账户和页面状态，不能操作表单、禁用项或提交控件' }}; const label = (element.innerText || element.getAttribute('aria-label') || element.getAttribute('title') || '').replace(/\\s+/g, ' ').trim(); if (/(登录|注册|提交|发布|发送|保存|删除|购买|支付|下单|充值|点赞|收藏|关注|转发|login|sign[ -]?in|register|submit|publish|send|save|delete|buy|pay|order|like|follow|share)/i.test(label)) return {{ clicked: false, error: '为了保护账户和页面状态，不能点击可能产生副作用的控件' }}; const isButton = element.matches('button,[role=\"button\"]'); const isReadOnlyButton = /(展开|显示|详情|评论|回复|更多|下一页|上一页|加载|查看|打开|expand|show|detail|comment|reply|more|next|previous|load|view|open)/i.test(label) || element.hasAttribute('aria-expanded') || element.hasAttribute('aria-controls'); if (isButton && !isReadOnlyButton) return {{ clicked: false, error: '未能确认这是只读展开或导航控件，因此拒绝点击' }}; element.scrollIntoView({{ block: 'center' }}); element.click(); return {{ clicked: true, elementId: id, label }}; }})()", SEMANTIC_CONTEXT_BOOTSTRAP))
}

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

pub const EXTRACT_COMMENTS_HTML_SCRIPT: &str = r#"
(() => {
  const bodyText = (document.body?.innerText || '').slice(0, 12000).toLowerCase();
  const terms = ['验证码', '安全验证', '请先登录', '扫码登录', 'captcha', 'verify you are human', 'sign in to continue'];
  const challengeReason = terms.find((term) => bodyText.includes(term)) || null;
  const selectors = [
    '[data-comment]', '[data-comment-id]', '.comment', 'article.comment', '[class*=comment]',
    '.reply-item', '.reply-wrap', '.note-comment', '.comment-item',
    '.CommentItem', '.CommentItemV2', '[class*=CommentItem]'
  ];
  const isVisible = (element) => {
    const style = window.getComputedStyle(element);
    const rect = element.getBoundingClientRect();
    return style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity) !== 0
      && rect.width > 0 && rect.height > 0;
  };
  const seen = new Set();
  const candidates = selectors.flatMap((selector) => Array.from(document.querySelectorAll(selector)))
    .filter((element) => {
      if (seen.has(element) || !isVisible(element)) return false;
      seen.add(element);
      return true;
    });
  const maxHtmlChars = 1000000;
  let html = '';
  let htmlTruncated = false;
  for (const element of candidates) {
    const fragment = element.outerHTML || '';
    if (html.length + fragment.length + 1 > maxHtmlChars) {
      htmlTruncated = true;
      break;
    }
    html += (html ? '\n' : '') + fragment;
  }
  return {
    title: document.title || location.hostname,
    url: location.href,
    html,
    htmlTruncated,
    loginChallenge: Boolean(challengeReason),
    challengeReason,
  };
})()
"#;

pub fn extract_bilibili_subtitles_script(all_parts: bool) -> String {
    format!(
        r#"(async () => {{
  const allParts = {all_parts};
  const video = globalThis.__INITIAL_STATE__?.videoData;
  const currentUrl = new URL(location.href);
  const bvid = String(video?.bvid || currentUrl.pathname.match(/\/video\/(BV[0-9A-Za-z]+)/i)?.[1] || '');
  const pages = Array.isArray(video?.pages) ? video.pages.slice(0, 24).map((page, index) => ({{
    cid: Number(page?.cid), page: Number(page?.page || index + 1), part: String(page?.part || '')
  }})).filter((page) => Number.isSafeInteger(page.cid) && page.cid > 0) : [];
  const requestedPage = Number(currentUrl.searchParams.get('p') || 0);
  const current = (requestedPage > 0 ? pages.find((page) => page.page === requestedPage) : null)
    || pages.find((page) => page.cid === Number(video?.cid)) || pages[0];
  const owner = video?.owner || {{}}; const stat = video?.stat || {{}};
  const videoInfo = {{
    aid: Number(video?.aid) || null, bvid, title: String(video?.title || document.title || ''),
    author: String(owner?.name || ''), description: String(video?.desc || ''),
    publishedAt: Number(video?.pubdate) || null, durationSeconds: Number(video?.duration) || null,
    views: Number(stat?.view) || null, replies: Number(stat?.reply) || null,
    currentPart: null, pages
  }};
  if (!bvid || !current) return {{ version: 1, status: 'invalid_video_state', bvid, videoInfo, pages: [], tracks: [] }};
  videoInfo.currentPart = current.page;
  const selected = allParts ? pages.slice(0, 12) : [current];
  const tracks = []; const warnings = [];
  let loginRequired = false;
  for (const part of selected) {{
    try {{
      const endpoint = new URL('https://api.bilibili.com/x/player/v2');
      endpoint.searchParams.set('bvid', bvid); endpoint.searchParams.set('cid', String(part.cid));
      const response = await fetch(endpoint, {{ credentials: 'include' }});
      if (!response.ok) {{ warnings.push(`P${{part.page}} player HTTP ${{response.status}}`); continue; }}
      const payload = await response.json();
      if (payload?.code !== 0) {{ warnings.push(`P${{part.page}} player code ${{payload?.code ?? 'unknown'}}`); continue; }}
      const candidates = payload?.data?.subtitle?.subtitles;
      if (!Array.isArray(candidates)) {{ warnings.push(`P${{part.page}} subtitle list malformed`); continue; }}
      if (!candidates.length && payload?.data?.need_login_subtitle === true) loginRequired = true;
      for (const candidate of candidates.slice(0, 6)) {{
        const rawUrl = typeof candidate?.subtitle_url === 'string' ? candidate.subtitle_url.trim() : '';
        if (!rawUrl) {{ if (payload?.data?.need_login_subtitle === true) loginRequired = true; continue; }}
        const subtitleUrl = new URL(rawUrl.startsWith('//') ? `https:${{rawUrl}}` : rawUrl);
        const host = subtitleUrl.hostname.toLowerCase();
        if (subtitleUrl.protocol !== 'https:' || !(host === 'hdslb.com' || host.endsWith('.hdslb.com') || host === 'bilibili.com' || host.endsWith('.bilibili.com'))) {{
          warnings.push(`P${{part.page}} subtitle host rejected`); continue;
        }}
        const subtitleResponse = await fetch(subtitleUrl, {{ credentials: 'omit', referrerPolicy: 'no-referrer' }});
        if (!subtitleResponse.ok) {{ warnings.push(`P${{part.page}} subtitle HTTP ${{subtitleResponse.status}}`); continue; }}
        const text = await subtitleResponse.text();
        if (text.length > 2000000) {{ warnings.push(`P${{part.page}} subtitle too large`); continue; }}
        let parsed; try {{ parsed = JSON.parse(text); }} catch {{ warnings.push(`P${{part.page}} subtitle JSON invalid`); continue; }}
        const body = Array.isArray(parsed?.body) ? parsed.body : Array.isArray(parsed) ? parsed : null;
        if (!body) {{ warnings.push(`P${{part.page}} subtitle body malformed`); continue; }}
        const lan = String(candidate?.lan || '');
        const subtitleType = lan.toLowerCase().startsWith('ai-') || Number(candidate?.type) === 1 ? 'ai' : 'manual';
        tracks.push({{
          subtitleType, language: lan, languageLabel: String(candidate?.lan_doc || ''),
          partIndex: part.page, partId: String(part.cid), partTitle: part.part,
          segments: body.slice(0, 4000).map((segment) => ({{
            from: Number(segment?.from), to: Number(segment?.to), content: String(segment?.content ?? '')
          }}))
        }});
      }}
    }} catch {{ warnings.push(`P${{part.page}} subtitle unavailable`); }}
  }}
  return {{ version: 1, status: tracks.length ? 'ok' : loginRequired ? 'login_required' : 'no_subtitles', bvid, videoInfo,
    currentPart: current.page, requestedAllParts: allParts, pages: selected, tracks, warnings: warnings.slice(0, 24) }};
}})()"#,
        all_parts = if all_parts { "true" } else { "false" }
    )
}

/// Extract a small number of visual frames from the page's HTML video element.
///
/// The frame stays in memory as a bounded data URL.  This intentionally does not
/// inspect or download any media resource and does not expose player URLs.
pub fn extract_bilibili_keyframes_script(timestamps_ms: &[u64]) -> Result<String, String> {
    const MAX_FRAMES: usize = 6;
    let timestamps = timestamps_ms
        .iter()
        .copied()
        .take(MAX_FRAMES)
        .collect::<Vec<_>>();
    let serialized = serde_json::to_string(&timestamps)
        .map_err(|error| format!("无法编码关键帧时间点：{error}"))?;
    Ok(format!(
        r#"(async () => {{
  const requested = {timestamps};
  const video = document.querySelector('video');
  const title = document.title || location.hostname;
  const result = {{ version: 1, status: 'no_video', title, frames: [], warnings: [] }};
  if (!video) return result;
  const waitForSeek = () => new Promise((resolve) => {{
    let settled = false;
    const finish = (value) => {{ if (settled) return; settled = true; clearTimeout(timer); video.removeEventListener('seeked', onSeeked); resolve(value); }};
    const onSeeked = () => finish(true);
    const timer = setTimeout(() => finish(false), 1500);
    video.addEventListener('seeked', onSeeked, {{ once: true }});
  }});
  const frames = [];
  const warnings = [];
  const maxTotalChars = 16_000_000;
  let totalChars = 0;
  const originalTime = Number(video.currentTime || 0);
  const wasPaused = Boolean(video.paused);
  const width = Math.min(960, Math.max(1, Number(video.videoWidth || video.clientWidth || 960)));
  const height = Math.min(540, Math.max(1, Number(video.videoHeight || video.clientHeight || 540)));
  const canvas = document.createElement('canvas'); canvas.width = width; canvas.height = height;
  const context = canvas.getContext('2d', {{ alpha: false }});
  if (!context) return {{ ...result, status: 'canvas_unavailable', warnings: ['无法创建画布上下文'] }};
  try {{
    video.pause();
    for (const raw of requested) {{
      const timestampMs = Number(raw);
      if (!Number.isFinite(timestampMs) || timestampMs < 0) {{ warnings.push('忽略无效关键帧时间点'); continue; }}
      const durationMs = Number.isFinite(video.duration) ? Math.max(0, video.duration * 1000) : null;
      const boundedMs = durationMs === null ? timestampMs : Math.min(timestampMs, durationMs);
      const targetSeconds = boundedMs / 1000;
      const alreadyThere = Math.abs(Number(video.currentTime || 0) - targetSeconds) < 0.05;
      const seek = alreadyThere ? Promise.resolve(true) : waitForSeek();
      try {{ video.currentTime = targetSeconds; }} catch {{ warnings.push(`时间点 ${{Math.round(boundedMs)}}ms 无法定位`); continue; }}
      const sought = await seek;
      if (!sought) {{ warnings.push(`时间点 ${{Math.round(boundedMs)}}ms 定位超时`); continue; }}
      try {{
        context.drawImage(video, 0, 0, width, height);
        const dataUrl = canvas.toDataURL('image/jpeg', 0.72);
        if (!dataUrl.startsWith('data:image/jpeg;base64,')) {{ warnings.push('关键帧格式不可用'); continue; }}
        if (dataUrl.length > 11_000_000) {{ warnings.push('关键帧超过大小限制'); continue; }}
        if (totalChars + dataUrl.length > maxTotalChars) {{ warnings.push('关键帧总大小超过限制'); break; }}
        totalChars += dataUrl.length;
        frames.push({{ timestampMs: Math.round(boundedMs), mimeType: 'image/jpeg', width, height, dataUrl }});
      }} catch {{ warnings.push(`时间点 ${{Math.round(boundedMs)}}ms 无法读取画面`); }}
    }}
  }} finally {{
    try {{ video.currentTime = originalTime; }} catch {{}}
    if (!wasPaused) {{ try {{ await video.play(); }} catch {{}} }}
  }}
  result.status = frames.length ? 'ok' : 'no_readable_frames';
  result.frames = frames;
  result.warnings = warnings.slice(0, 12);
  return result;
}})()"#,
        timestamps = serialized
    ))
}

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
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("__paimonSemanticContext"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("data-paimon-id"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("pageSession"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("startsWith(prefix)"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("aria-label"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("placeholder"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("matchedIn"));
        assert!(EXTRACT_VISIBLE_PAGE_SCRIPT.contains("ancestors"));
        assert!(!EXTRACT_VISIBLE_PAGE_SCRIPT.contains("platformTerms"));
    }

    #[test]
    fn semantic_click_script_rejects_forms_and_side_effect_labels() {
        let script = click_element_script("paimon-test-1-1").unwrap();

        assert!(script.contains("button[type=\"submit\"]"));
        assert!(script.contains("不能点击可能产生副作用的控件"));
        assert!(script.contains("未能确认这是只读展开或导航控件"));
    }

    #[test]
    fn child_navigation_interception_marks_all_links_for_new_windows() {
        assert!(
            super::super::commands::CHILD_NAVIGATION_SCRIPT.contains("anchor.target = '_blank'")
        );
        assert!(super::super::commands::CHILD_NAVIGATION_SCRIPT.contains("noopener"));
    }

    #[test]
    fn comment_html_script_reuses_login_and_captcha_terms() {
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("请先登录"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("captcha"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("outerHTML"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("getBoundingClientRect"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("display !== 'none'"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("visibility !== 'hidden'"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("1000000"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains("htmlTruncated"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains(".reply-item"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains(".comment-item"));
        assert!(EXTRACT_COMMENTS_HTML_SCRIPT.contains(".CommentItem"));
    }

    #[test]
    fn bilibili_subtitle_script_reads_only_public_subtitle_data() {
        let script = extract_bilibili_subtitles_script(true);
        assert!(script.contains("/x/player/v2"));
        assert!(script.contains("subtitle_url"));
        assert!(script.contains("subtitleType"));
        assert!(script.contains("pages.slice(0, 12)"));
        assert!(!script.to_ascii_lowercase().contains("audio"));
    }

    #[test]
    fn bilibili_keyframe_script_is_bounded_and_does_not_download_audio() {
        let script = extract_bilibili_keyframes_script(&[0, 1, 2, 3, 4, 5, 6]).unwrap();
        assert!(script.contains("querySelector('video')"));
        assert!(script.contains("toDataURL('image/jpeg', 0.72)"));
        assert!(script.contains("requested = [0,1,2,3,4,5]"));
        assert!(script.contains("maxTotalChars = 16_000_000"));
        assert!(script.contains("frames.length ? 'ok' : 'no_readable_frames'"));
        assert!(!script.to_ascii_lowercase().contains("audio"));
    }

    #[test]
    fn bilibili_keyframe_script_serializes_untrusted_time_points() {
        let script = extract_bilibili_keyframes_script(&[u64::MAX]).unwrap();
        assert!(script.contains("18446744073709551615"));
        assert!(!script.contains("window.evil"));
    }
}
