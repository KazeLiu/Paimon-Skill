# Tauri 2 Desktop App Implementation Roadmap

> This document defines subsystem boundaries and delivery order. Each phase receives a separate executable plan under `docs/superpowers/plans/` before implementation begins.

**Goal:** Build a Windows Tauri 2 desktop client that opens directly into a configurable Paimon chat experience with model providers, web research tools, persistent web login, Markdown rendering, and local conversation history.

**Architecture:** Vue 3 renders the desktop UI. Rust owns the Agent loop, provider adapters, tool execution, URL safety, SQLite persistence, and WebView task coordination. Prompt text is loaded from packaged resource files and user settings; Rust code contains tool contracts and validation, not a hard-coded domain prompt. Existing `skills/genshin-build-wiki/SKILL.md` is packaged as a default rule resource and can be enabled, disabled, or reset from settings.

**Tech Stack:** Tauri 2, Rust, Vue 3, TypeScript, Pinia, SQLite, `reqwest`, `serde`, WebView2, `markdown-it`, DOMPurify, Windows Credential Manager/system secret storage, NSIS.

---

## File Map

- Create: `apps/desktop/src-tauri/` for Rust commands, Agent runtime, providers, tools, storage, WebView coordination, and Tauri configuration.
- Create: `apps/desktop/src/` for Vue routes, Pinia stores, settings, chat, browser tabs, and Markdown rendering.
- Create: `apps/desktop/resources/prompts/` for editable-default prompt fragments and prompt metadata.
- Create: `apps/desktop/resources/skills/genshin-build-wiki/` as the packaged copy of the existing skill at build time; do not hand-edit this copy.
- Create: `apps/desktop/tests/` for provider, prompt composition, URL safety, extraction, and UI-level tests.
- Modify: root `package.json` to add workspace scripts for desktop development and packaging.
- Modify: root `README.md` with desktop build prerequisites and the application scope.
- Modify: `skills/genshin-build-wiki/SKILL.md` only when the shared domain rules change; desktop-specific behavior belongs under `apps/desktop`.

## Task 1: Scaffold the Tauri 2 workspace

**Files:**
- Create: `apps/desktop/package.json`
- Create: `apps/desktop/src-tauri/Cargo.toml`
- Create: `apps/desktop/src-tauri/tauri.conf.json`
- Create: `apps/desktop/src/main.ts`
- Create: `apps/desktop/src/App.vue`
- Modify: `package.json`

- [ ] **Step 1: Add the desktop workspace scripts and dependency boundaries.**

  Add root scripts `desktop:dev`, `desktop:build`, and `desktop:test`; keep the existing skill tests unchanged. The desktop package must pin Tauri 2 and Vue dependencies instead of relying on globally installed tools.

- [ ] **Step 2: Create the smallest Tauri window and Vue shell.**

  The initial shell must expose a stable layout with a sidebar, chat viewport, and settings route placeholder. It must start without any model configured and show an actionable configuration state.

- [ ] **Step 3: Verify the skeleton.**

  Run `npm install`, `npm run desktop:dev`, and `npm run desktop:build` from the repository root. Expected result: the Tauri window opens and the Windows bundle is produced without requiring the existing skill to be installed globally.

## Task 2: Implement configurable prompt resources

**Files:**
- Create: `apps/desktop/resources/prompts/base-system.md`
- Create: `apps/desktop/resources/prompts/paimon-persona.md`
- Create: `apps/desktop/resources/prompts/tool-policy.md`
- Create: `apps/desktop/src-tauri/src/prompt/mod.rs`
- Create: `apps/desktop/src-tauri/src/prompt/composer.rs`
- Create: `apps/desktop/src-tauri/src/prompt/models.rs`
- Create: `apps/desktop/src/pages/SettingsPrompts.vue`
- Create: `apps/desktop/src/stores/promptStore.ts`
- Create: `apps/desktop/tests/prompt-composer.test.ts`

- [ ] **Step 1: Define the persisted prompt model.**

  Store `base_system`, `persona`, `enabled_rule_resources`, `tool_policy`, `custom_suffix`, and `updated_at`. The model must distinguish packaged defaults from user overrides so “恢复默认” can replace user text without deleting the resource files.

- [ ] **Step 2: Implement prompt composition.**

  Compose enabled fragments in this exact order: base system, persona, enabled skill resources, tool policy, custom suffix. Return both the final text and a list of contributing resource IDs for diagnostics. Reject empty final prompts only when every fragment is disabled and no custom text exists.

- [ ] **Step 3: Add settings commands and UI.**

  Expose `get_prompt_settings`, `save_prompt_settings`, `reset_prompt_settings`, and `list_rule_resources` through Tauri commands. The settings page must provide editable text areas, resource enable toggles, save status, and a reset action. It must never require editing source files.

- [ ] **Step 4: Test prompt behavior.**

  Test default composition, custom text persistence, disabled skill resources, reset behavior, ordering, and the absence of hard-coded prompt fallback in the Agent command path. Run `npm run desktop:test -- prompt-composer` and expect all cases to pass.

## Task 3: Add provider adapters and connection tests

**Files:**
- Create: `apps/desktop/src-tauri/src/providers/mod.rs`
- Create: `apps/desktop/src-tauri/src/providers/openai_compatible.rs`
- Create: `apps/desktop/src-tauri/src/providers/models.rs`
- Create: `apps/desktop/src-tauri/src/commands/providers.rs`
- Create: `apps/desktop/src/pages/SettingsModels.vue`
- Create: `apps/desktop/src/stores/providerStore.ts`
- Create: `apps/desktop/tests/provider-adapter.test.ts`

- [ ] **Step 1: Define the provider contract.**

  Use `ProviderConfig { id, name, base_url, model, api_key_ref, timeout_ms, temperature, max_tokens, streaming, tool_calling }`. Normalize Base URLs and reject non-HTTP(S) endpoints except explicitly enabled local model hosts.

- [ ] **Step 2: Implement OpenAI-Compatible chat and SSE streaming.**

  Support `/v1/chat/completions`, role/content messages, tool definitions, tool calls, cancellation, timeouts, and malformed SSE errors. Keep API keys outside SQLite values by storing only a secret reference.

- [ ] **Step 3: Implement connection, streaming, and tool capability tests.**

  Return separate results for basic request, streaming request, and tool-call probe. A provider can be usable for ordinary chat while being marked unavailable for tool calls.

- [ ] **Step 4: Build the model settings UI.**

  Allow add/edit/delete/select provider, reveal only masked key state, test each capability, and display actionable error text. Include presets for DeepSeek, generic relay, Ollama `/v1`, and LM Studio `/v1`, while keeping all URLs editable.

- [ ] **Step 5: Test provider parsing and failures.**

  Mock successful JSON, SSE chunks, timeout, HTTP error, invalid JSON, and unsupported tool-call responses. Run `npm run desktop:test -- provider-adapter` and expect deterministic results without network access.

## Task 4: Implement local storage and Agent runtime

**Status (2026-08-19): In progress.** Conversations, messages, complete tool runs, and deduplicated sources now persist in SQLite with conversation/run/call identifiers and cascade deletion. Historical conversations restore their messages, sources, and tool activity. The Agent supports bounded multi-round tool calling, structured tool failures, streaming, and cancellation. Remaining work: provider/prompt database consolidation, browser profile metadata, older-context summarization/trimming, and the finer-grained progress/result event contract.

**Files:**
- Create: `apps/desktop/src-tauri/src/storage/migrations/001_initial.sql`
- Create: `apps/desktop/src-tauri/src/storage/mod.rs`
- Create: `apps/desktop/src-tauri/src/storage/repository.rs`
- Create: `apps/desktop/src-tauri/src/agent/mod.rs`
- Create: `apps/desktop/src-tauri/src/agent/context.rs`
- Create: `apps/desktop/src-tauri/src/agent/events.rs`
- Create: `apps/desktop/src-tauri/src/commands/chat.rs`
- Create: `apps/desktop/src/stores/chatStore.ts`
- Create: `apps/desktop/tests/agent-runtime.test.ts`

- [ ] **Step 1: Create SQLite tables.**

  Add `providers`, `prompt_settings`, `conversations`, `messages`, `tool_runs`, `sources`, and `browser_profiles`. Store complete tool results locally but send only bounded, relevant excerpts to the model.

- [ ] **Step 2: Implement the Agent event contract.**

  Emit `message_delta`, `tool_started`, `tool_progress`, `tool_result`, `source_added`, `assistant_finished`, `run_failed`, and `run_cancelled`. Every event includes a conversation ID and run ID so the UI can safely ignore stale runs.

- [ ] **Step 3: Implement the tool-calling loop.**

  Load the composed prompt from storage, append conversation context, call the selected provider, validate tool-call arguments, execute the tool registry, append structured tool results, and continue until the model returns a final answer or the configured maximum tool rounds is reached.

- [ ] **Step 4: Implement context trimming.**

  Keep the latest user/assistant turns, summarize older messages, cap each tool result by characters/tokens, and preserve source metadata even when body text is trimmed.

- [ ] **Step 5: Test the runtime.**

  Use a fake provider and fake tools to test ordinary chat, one tool round, multiple tool rounds, cancellation, malformed arguments, maximum-round failure, and context trimming.

## Task 5: Build chat UI and safe Markdown rendering

**Files:**
- Create: `apps/desktop/src/pages/ChatPage.vue`
- Create: `apps/desktop/src/components/ConversationSidebar.vue`
- Create: `apps/desktop/src/components/MessageList.vue`
- Create: `apps/desktop/src/components/MarkdownMessage.vue`
- Create: `apps/desktop/src/components/ToolRunPanel.vue`
- Create: `apps/desktop/src/components/SourceList.vue`
- Create: `apps/desktop/src/lib/markdown.ts`
- Create: `apps/desktop/tests/markdown-rendering.test.ts`

- [ ] **Step 1: Implement conversation actions.**

  Support new conversation, rename, delete, select, send, stop, retry, copy, and model selection. Disable send while no provider is configured and show the settings link in the empty state.

- [ ] **Step 2: Implement streaming message updates.**

  Subscribe to Tauri events by conversation/run ID, append deltas incrementally, and keep tool progress visible in a collapsible panel.

- [ ] **Step 3: Implement sanitized Markdown.**

  Render GFM headings, lists, tables, code blocks, links, and blockquotes. Strip raw HTML, reject `javascript:`, `file:`, and other unsafe protocols, and sanitize with DOMPurify before injecting HTML.

- [ ] **Step 4: Render sources separately from model text.**

  Show actual tool-recorded sources with title, URL, platform/source level, captured time, and a link action. Do not trust model-written URLs as evidence.

- [ ] **Step 5: Test Markdown and interaction states.**

  Verify XSS payloads, unsafe links, tables, code blocks, partial streaming Markdown, empty provider state, cancellation, and stale event handling.

## Task 6: Add safe web and domain tools

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/mod.rs`
- Create: `apps/desktop/src-tauri/src/tools/registry.rs`
- Create: `apps/desktop/src-tauri/src/tools/url_policy.rs`
- Create: `apps/desktop/src-tauri/src/tools/bwiki.rs`
- Create: `apps/desktop/src-tauri/src/tools/obc.rs`
- Create: `apps/desktop/src-tauri/src/tools/static_page.rs`
- Create: `apps/desktop/src-tauri/src/tools/models.rs`
- Create: `apps/desktop/tests/url-policy.test.ts`

- [ ] **Step 1: Define tool schemas and permissions.**

  Register `search_bwiki`, `read_bwiki_page`, `read_obc_page`, and `read_url`. Each schema specifies required arguments, maximum result size, whether login is needed, and whether the tool has side effects; all current tools must be read-only.

- [ ] **Step 2: Implement URL validation.**

  Allow only `http` and `https`, reject loopback, link-local, private IPv4/IPv6, metadata endpoints, embedded credentials, excessive redirects, oversized responses, and requests exceeding the configured timeout. Permit local model hosts only through provider requests, not generic URL tools.

- [ ] **Step 3: Implement BWiki/OBC and static extraction.**

  Use structured BWiki API requests where available. For HTML pages, extract title, canonical URL, visible text, headings, links, and publication candidates; remove scripts, styles, navigation, and hidden content.

- [ ] **Step 4: Record sources and failures.**

  Every successful or failed tool run stores request metadata, actual URL, capture time, and bounded result/error text. The Agent receives a typed result that distinguishes unavailable source from empty source.

- [ ] **Step 5: Test tools without external network.**

  Mock HTTP responses and assert URL policy, parser output, source metadata, redirect limits, response-size limits, and failure messages.

## Task 7: Add persistent WebView2 browser tasks

**Status (2026-08-19): In progress.** The current branch implements application-owned persistent WebView2 profile data, embedded child-WebView tabs, address navigation, back/forward/reload, bounded page extraction, bounded scrolling, generic read-only visible-text clicks, login/challenge detection, “询问当前网页”, and Agent tools for opening, extracting, scrolling, and clicking the active page. Agent browser results are source/tool-run records scoped to the conversation and run. Profile clearing is now available from the privacy settings page and is guarded by a canonical-path check; remaining work includes explicit wait/cancellation controls, saved dynamic-page fixtures, and broader external-window/login-flow handling.

**Files:**
- Create: `apps/desktop/src-tauri/src/browser/mod.rs`
- Create: `apps/desktop/src-tauri/src/browser/profile.rs`
- Create: `apps/desktop/src-tauri/src/browser/task.rs`
- Create: `apps/desktop/src-tauri/src/browser/scripts.rs`
- Create: `apps/desktop/src-tauri/src/commands/browser.rs`
- Create: `apps/desktop/src/components/BrowserTabs.vue`
- Create: `apps/desktop/src/components/BrowserTab.vue`
- Create: `apps/desktop/src/stores/browserStore.ts`
- Create: `apps/desktop/tests/browser-extraction.test.ts`

- [x] **Step 1: Create persistent application profiles.**

  Use an application-owned WebView2 data directory per browser profile. Persist it across restarts, expose profile clearing in privacy settings with an explicit confirmation and canonical-path guard, and never export cookies or local storage to SQLite or model context.

- [ ] **Step 2: Implement visible browser tabs.**

  Support open/close/select, address navigation, back, forward, reload, page title, load state, and external-link policy. Keep the Vue shell isolated from loaded pages and expose only narrow Tauri commands.

- [ ] **Step 3: Implement read-only dynamic loading.**

  Provide bounded `wait_for_stable`, `scroll_until_stable`, and `extract_visible_page` operations. Stop after configured rounds/timeouts, detect login/challenge pages, and allow user cancellation.

- [ ] **Step 4: Implement generic click/read operations.**

  Accept a model-generated target only as a visible-text/accessible-label request. Require a read-only action class, reject submit/login/publish/delete actions, execute in the page, and return a fresh snapshot after the click.

- [ ] **Step 5: Add “询问当前网页”.**

  Let the user send the active page title, URL, and extracted bounded content into the current conversation. Mark it as user-selected browser content, not as a trusted system instruction.

- [ ] **Step 6: Test extraction and safety.**

  Use local fixture pages to test dynamic content, stable-scroll termination, expandable sections, comments, login challenge detection, cancellation, and side-effect rejection.

Current automated coverage includes URL protocol/credential rejection, side-effect click rejection, bounded content and links, stable-scroll termination, WebView callback decoding, tab visibility state, browser UI address opening, current-page handoff, and visible error state. Dynamic fixture and cancellation coverage remains pending.

## Task 8: Add website adapters for comments and high-frequency pages

**Status (2026-08-19): In progress.** A bounded `browser_extract_comments` tool now extracts visible comments from generic pages and Bilibili/Xiaohongshu/Zhihu selector families, including author, content, visible likes, timestamps, source URL, and login/CAPTCHA stop reasons. Candidate DOM is visibility-filtered and capped at 1 MB; results are persisted through the existing tool/source records. Remaining work: richer per-site pagination/expand workflows, saved fixtures from representative dynamic pages, and more robust selector fallback when a site changes its markup.

**Files:**
- Create: `apps/desktop/src-tauri/src/adapters/mod.rs`
- Create: `apps/desktop/src-tauri/src/adapters/bilibili.rs`
- Create: `apps/desktop/src-tauri/src/adapters/xiaohongshu.rs`
- Create: `apps/desktop/src-tauri/src/adapters/zhihu.rs`
- Create: `apps/desktop/src-tauri/src/tools/community.rs`
- Create: `apps/desktop/tests/community-adapters.test.ts`

- [ ] **Step 1: Define a shared extracted-page model.**

  Use `WebPageResult { platform, title, url, author, published_at, content, comments, metrics, links, captured_at }`; comments contain author, content, likes when visible, and capture context.

- [ ] **Step 2: Implement bounded platform workflows.**

  For each platform, search by query, open a bounded number of results, expand visible comments or “view all” controls, scroll until stable, deduplicate entries, and return structured content. All workflows stop on login/challenge detection.

- [ ] **Step 3: Register community tools and source levels.**

  Add `search_bilibili`, `read_bilibili_page`, `search_xiaohongshu`, `read_xiaohongshu_page`, and corresponding Zhihu tools only after the profile is logged in. Mark results as community experience and preserve actual post/video URLs.

- [ ] **Step 4: Test with saved HTML fixtures.**

  Assert extraction of titles, authors, timestamps, comments, metrics, pagination, duplicate removal, and fallback to generic browser extraction after selector failure.

## Task 9: Package, privacy controls, and verification

**Status (2026-08-19): In progress.** Added a privacy settings page with confirmation-gated clearing of the application-owned WebView2 default Profile. The cleanup command closes embedded tabs, clears in-memory browser state, validates the fixed profile path against its canonical parent, and removes only that profile directory. A user guide documents storage locations, secret handling, login-state isolation, and the read-only web boundary. Remaining work: final NSIS artifact verification and a manual smoke test against a representative logged-in dynamic page.

**Files:**
- Modify: `apps/desktop/src/pages/SettingsPage.vue`
- Create: `apps/desktop/src/pages/SettingsPrivacy.vue`
- Create: `apps/desktop/src-tauri/src/commands/diagnostics.rs`
- Modify: `apps/desktop/src-tauri/tauri.conf.json`
- Modify: `README.md`
- Create: `docs/desktop-user-guide.md`

- [ ] **Step 1: Add privacy and diagnostics controls.**

  Provide clear prompt settings, clear conversations, clear browser profile data, delete provider secrets, export/import non-secret settings, and a redacted diagnostics view. Never include API keys or cookies in logs.

- [ ] **Step 2: Configure Windows packaging.**

  Produce NSIS installer and portable artifact configuration. Keep application data outside the installation directory and ensure upgrades do not remove SQLite or WebView2 profile data.

- [ ] **Step 3: Document prerequisites and behavior.**

  Document Rust, Node, WebView2, Windows build prerequisites, model configuration, first-time community login, supported source types, known site-adapter limits, and data deletion behavior.

- [ ] **Step 4: Run verification.**

  Run `npm test`, `npm run desktop:test`, `npm run desktop:build`, and a manual smoke test covering: configure provider, test connection, edit prompt, new conversation, tool lookup, Markdown rendering, open/login browser tab, dynamic scroll, comment extraction, cancel run, restart app, and confirm conversation/login persistence.

- [ ] **Step 5: Review the release boundary.**

  Confirm the release does not claim arbitrary-site automation, CAPTCHA bypass, or external side effects. Confirm that all source URLs shown in the UI originate from tool records.
