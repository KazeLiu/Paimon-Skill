import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "./chatStore";
import { deleteConversation, listConversations, listMessages, listSources, listToolRuns, sendMessage } from "@/api/chat";
import { listProviders } from "@/api/providers";

const eventApi = vi.hoisted(() => ({
  listen: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => eventApi);

vi.mock("@/api/chat", () => ({
  listConversations: vi.fn(),
  listMessages: vi.fn(),
  listSources: vi.fn(),
  listToolRuns: vi.fn(),
  sendMessage: vi.fn(),
  cancelRun: vi.fn(),
  deleteConversation: vi.fn(),
  renameConversation: vi.fn(),
}));

vi.mock("@/api/providers", () => ({
  listProviders: vi.fn(),
}));

describe("chatStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    eventApi.listen.mockResolvedValue(vi.fn());
    vi.mocked(listProviders).mockResolvedValue([{ config: {
      id: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com", model: "deepseek-chat",
      secretRef: "provider:deepseek", timeoutMs: 30000,
      streaming: true, toolCalling: true, enabled: true,
    }, hasApiKey: true }]);
    vi.mocked(listConversations).mockResolvedValue([]);
    vi.mocked(listMessages).mockResolvedValue([]);
    vi.mocked(listSources).mockResolvedValue([]);
    vi.mocked(listToolRuns).mockResolvedValue([]);
  });

  it("does not block the chat store when event subscription is denied", async () => {
    eventApi.listen.mockRejectedValue(new Error("Command plugin:event|listen not allowed by ACL"));
    const store = useChatStore();

    await expect(store.bindEvents()).resolves.toBeUndefined();
    expect(store.unlisten).toBeNull();
  });

  it("adds the user message and merges streaming deltas into one assistant message", async () => {
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    const store = useChatStore();
    await store.initialize();

    await store.send("你好");
    store.handleEvent({ kind: "message_delta", runId: "run-1", conversationId: "c1", delta: "旅", messageId: null, error: null });
    store.handleEvent({ kind: "message_delta", runId: "run-1", conversationId: "c1", delta: "行者", messageId: null, error: null });

    expect(store.messages[0].content).toBe("你好");
    expect(store.messages[1].content).toBe("旅行者");

    store.handleEvent({ kind: "assistant_finished", runId: "run-1", conversationId: "c1", delta: null, finalContent: "旅行者，最终内容", messageId: "m2", error: null });
    expect(store.messages[1]).toMatchObject({ id: "m2", content: "旅行者，最终内容" });
  });

  it("updates the active provider model after settings are saved", async () => {
    const store = useChatStore();
    await store.initialize();

    store.syncProvider({
      config: {
        id: "deepseek",
        name: "DeepSeek",
        baseUrl: "https://api.deepseek.com",
        model: "deepseek-chat-terra",
        secretRef: "provider:deepseek",
        timeoutMs: 30000,
        streaming: true,
        toolCalling: true,
        enabled: true,
      },
      hasApiKey: true,
    });

    expect(store.selectedProvider?.config.model).toBe("deepseek-chat-terra");
    expect(store.providers[0].config.model).toBe("deepseek-chat-terra");
  });

  it("selects the provider that was just saved in settings", async () => {
    const store = useChatStore();
    await store.initialize();

    store.syncProvider({
      config: {
        id: "relay",
        name: "Relay",
        baseUrl: "https://relay.example/v1",
        model: "gpt-5.2",
        secretRef: "provider:relay",
        timeoutMs: 30000,
        streaming: true,
        toolCalling: true,
        enabled: true,
      },
      hasApiKey: true,
    });

    expect(store.selectedProviderId).toBe("relay");
  });

  it("replays stream events that arrive before sendMessage resolves", async () => {
    let resolveSend!: (value: Awaited<ReturnType<typeof sendMessage>>) => void;
    vi.mocked(sendMessage).mockReturnValue(new Promise((resolve) => { resolveSend = resolve; }));
    const store = useChatStore();
    await store.initialize();

    const sending = store.send("你好");
    store.handleEvent({ kind: "message_delta", runId: "run-early", conversationId: "c1", delta: "完整", messageId: null, error: null });
    store.handleEvent({ kind: "assistant_finished", runId: "run-early", conversationId: "c1", delta: null, finalContent: "完整", messageId: "m2", error: null });
    resolveSend({
      runId: "run-early",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    await sending;

    expect(store.messages[1]).toMatchObject({ id: "m2", content: "完整" });
    expect(store.sending).toBe(false);
  });

  it("replays source and tool events that arrive before sendMessage resolves", async () => {
    let resolveSend!: (value: Awaited<ReturnType<typeof sendMessage>>) => void;
    vi.mocked(sendMessage).mockReturnValue(new Promise((resolve) => { resolveSend = resolve; }));
    const store = useChatStore();
    await store.initialize();

    const sending = store.send("你好");
    store.handleTool({ conversationId: "c1", runId: "run-early", toolName: "browser_extract_page", status: "completed" });
    store.handleSource({
      conversationId: "c1",
      runId: "run-early",
      source: { title: "动态页面", url: "https://example.com", sourceLevel: "browser_webview", platform: "example.com", capturedAt: "1" },
    });
    store.handleEvent({ kind: "assistant_finished", runId: "run-early", conversationId: "c1", delta: null, finalContent: "完成", messageId: "m2", error: null });
    resolveSend({
      runId: "run-early",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    await sending;

    expect(store.sources.map((source) => source.url)).toEqual(["https://example.com"]);
    expect(store.toolActivities).toEqual([
      { conversationId: "c1", runId: "run-early", toolName: "browser_extract_page", status: "completed" },
    ]);
  });

  it("keeps a visible retryable error message when a run fails", async () => {
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.handleEvent({ kind: "message_delta", runId: "run-1", conversationId: "c1", delta: "半截", messageId: null, error: null });
    store.handleEvent({ kind: "run_failed", runId: "run-1", conversationId: "c1", delta: null, messageId: null, error: "HTTP 503: Service temporarily unavailable" });

    expect(store.messages[1]).toMatchObject({
      id: "pending:run-1",
      content: "半截",
      status: "error",
      error: "HTTP 503: Service temporarily unavailable",
      retryable: true,
    });
    expect(store.sending).toBe(false);
  });

  it("keeps an immediate send failure visible and retries it without duplicating the user message", async () => {
    vi.mocked(sendMessage)
      .mockRejectedValueOnce(new Error("所选模型缺少 API Key，请先在设置中保存"))
      .mockResolvedValueOnce({
        runId: "run-retry",
        conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "2" },
        userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", attachments: [], createdAt: "1" },
      });
    const store = useChatStore();
    await store.initialize();

    await expect(store.send("你好")).rejects.toThrow("所选模型缺少 API Key");
    const failed = store.messages.find((message) => message.status === "error");
    expect(failed).toMatchObject({ retryable: true, retryContent: "你好" });

    await store.retry(failed!.id);

    expect(store.messages.filter((message) => message.role === "user")).toHaveLength(1);
    expect(store.messages[store.messages.length - 1]).toMatchObject({ id: "pending:run-retry", status: "streaming" });
  });

  it("retries a failed run without inserting the user message twice", async () => {
    vi.mocked(sendMessage)
      .mockResolvedValueOnce({
        runId: "run-1",
        conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
        userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", attachments: [], createdAt: "1" },
      })
      .mockResolvedValueOnce({
        runId: "run-2",
        conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "2" },
        userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", attachments: [], createdAt: "1" },
      });
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.handleEvent({ kind: "run_failed", runId: "run-1", conversationId: "c1", delta: null, messageId: null, error: "timeout" });

    await store.retry("pending:run-1");

    const lastSendCall = vi.mocked(sendMessage).mock.calls[vi.mocked(sendMessage).mock.calls.length - 1];
    expect(lastSendCall?.[0]).toMatchObject({
      conversationId: "c1",
      providerId: "deepseek",
      content: "你好",
      retryMessageId: "m1",
    });
    expect(store.messages.filter((message) => message.role === "user")).toHaveLength(1);
    expect(store.messages[store.messages.length - 1]).toMatchObject({ id: "pending:run-2", status: "streaming" });
  });

  it("retries a failed run with the newly selected provider", async () => {
    vi.mocked(listProviders).mockResolvedValue([
      { config: { id: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com", model: "deepseek-chat", secretRef: "provider:deepseek", timeoutMs: 30000, streaming: true, toolCalling: true, enabled: true }, hasApiKey: true },
      { config: { id: "relay", name: "Relay", baseUrl: "https://relay.example/v1", model: "gpt-5.6-luna", secretRef: "provider:relay", timeoutMs: 30000, streaming: true, toolCalling: true, enabled: true }, hasApiKey: true },
    ]);
    vi.mocked(sendMessage)
      .mockResolvedValueOnce({
        runId: "run-1",
        conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
        userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", attachments: [], createdAt: "1" },
      })
      .mockResolvedValueOnce({
        runId: "run-2",
        conversation: { id: "c1", title: "测试", providerId: "relay", createdAt: "1", updatedAt: "2" },
        userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", attachments: [], createdAt: "1" },
      });
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.handleEvent({ kind: "run_failed", runId: "run-1", conversationId: "c1", delta: null, messageId: null, error: "timeout" });
    store.selectedProviderId = "relay";

    await store.retry("pending:run-1");

    expect(sendMessage).toHaveBeenLastCalledWith(expect.objectContaining({
      conversationId: "c1",
      providerId: "relay",
      retryMessageId: "m1",
    }));
    expect(store.conversations[0].providerId).toBe("relay");
  });

  it("sends image and file attachments with the user message", async () => {
    const attachments = [{
      name: "build.png",
      mimeType: "image/png",
      dataUrl: "data:image/png;base64,YQ==",
      size: 1,
      kind: "image" as const,
    }];
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "附件", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "看看这个", attachments, createdAt: "1" },
    });
    const store = useChatStore();
    await store.initialize();

    await store.send("看看这个", attachments);

    expect(sendMessage).toHaveBeenCalledWith(expect.objectContaining({ attachments }));
    expect(store.messages[0].attachments).toEqual(attachments);
  });

  it("cancels the active run before deleting its conversation", async () => {
    const store = useChatStore();
    store.conversations = [{ id: "c1", title: "当前", providerId: "deepseek", createdAt: "1", updatedAt: "1" }];
    store.activeConversationId = "c1";
    store.activeRunId = "run-1";

    await store.remove("c1");

    expect(deleteConversation).toHaveBeenCalledWith("c1");
    expect(store.activeConversationId).toBeNull();
  });

  it("does not switch conversations while a request is in flight", async () => {
    const store = useChatStore();
    store.conversations = [{ id: "c1", title: "当前", providerId: "deepseek", createdAt: "1", updatedAt: "1" }];
    store.activeConversationId = "c1";
    store.sending = true;

    await store.selectConversation("c2");
    store.newConversation();

    expect(store.activeConversationId).toBe("c1");
    expect(store.sending).toBe(true);
  });

  it("still removes unpersisted assistant content when a run is cancelled", async () => {
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.handleEvent({ kind: "run_cancelled", runId: "run-1", conversationId: "c1", delta: null, messageId: null, error: null });

    expect(store.messages.map((message) => message.content)).toEqual(["你好"]);
    expect(store.sending).toBe(false);
  });

  it("keeps the active conversation while a request is in flight", async () => {
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    vi.mocked(listMessages).mockResolvedValue([]);
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.conversations.push({ id: "c2", title: "另一个对话", providerId: "deepseek", createdAt: "2", updatedAt: "2" });
    await store.selectConversation("c2");

    store.handleEvent({ kind: "assistant_finished", runId: "run-1", conversationId: "c1", delta: null, finalContent: "完整", messageId: "m2", error: null });

    expect(store.activeRunId).toBeNull();
    expect(store.sending).toBe(false);
    expect(store.activeConversationId).toBe("c1");
    expect(store.messages).toMatchObject([
      { id: "m1", conversationId: "c1" },
      { id: "m2", content: "完整" },
    ]);
  });

  it("does not buffer an old conversation event while a new send is unresolved", () => {
    const store = useChatStore();
    store.activeConversationId = "c2";
    store.sending = true;

    store.handleEvent({ kind: "message_delta", runId: "run-old", conversationId: "c1", delta: "旧内容", messageId: null, error: null });

    expect(store.bufferedEvents).toEqual([]);
  });

  it("restores messages, sources, and tool runs when selecting a conversation", async () => {
    vi.mocked(listMessages).mockResolvedValue([
      { id: "m1", conversationId: "c1", role: "assistant", content: "结论", createdAt: "1" },
    ]);
    vi.mocked(listSources).mockResolvedValue([
      { id: "s1", conversationId: "c1", runId: "r1", callId: "call1", title: "来源", url: "https://example.com", sourceLevel: "web", platform: "网页", capturedAt: "1", createdAt: "1" },
    ]);
    vi.mocked(listToolRuns).mockResolvedValue([
      {
        id: "t1", conversationId: "c1", runId: "r1", callId: "call1", toolName: "read_url",
        arguments: "{}", status: "completed", result: null, error: null, createdAt: "1",
      },
    ]);
    const store = useChatStore();
    store.conversations = [{ id: "c1", title: "历史", providerId: "deepseek", createdAt: "1", updatedAt: "1" }];

    await store.selectConversation("c1");

    expect(store.messages).toHaveLength(1);
    expect(store.sources[0].url).toBe("https://example.com");
    expect(store.toolActivities).toEqual([{ conversationId: "c1", runId: "r1", toolName: "read_url", status: "completed" }]);
  });

  it("ignores stale source and tool events from another conversation or run", async () => {
    const store = useChatStore();
    store.conversations = [
      { id: "c1", title: "当前", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      { id: "c2", title: "旧", providerId: "deepseek", createdAt: "2", updatedAt: "2" },
    ];
    await store.selectConversation("c1");
    store.activeRunId = "run-current";

    const source = { title: "旧来源", url: "https://old.example", sourceLevel: "web", platform: "网页", capturedAt: "1" };
    store.handleSource({ conversationId: "c2", runId: "run-old", source });
    store.handleSource({ conversationId: "c1", runId: "run-old", source: { ...source, url: "https://wrong-run.example" } });
    store.handleTool({ conversationId: "c2", runId: "run-old", toolName: "read_url", status: "completed" });
    store.handleTool({ conversationId: "c1", runId: "run-old", toolName: "read_url", status: "completed" });

    expect(store.sources).toEqual([]);
    expect(store.toolActivities).toEqual([]);
  });

  it("does not overwrite the latest conversation with stale activity results", async () => {
    let resolveFirst!: (value: Awaited<ReturnType<typeof listSources>>) => void;
    vi.mocked(listMessages).mockResolvedValue([]);
    vi.mocked(listToolRuns).mockResolvedValue([]);
    vi.mocked(listSources)
      .mockReturnValueOnce(new Promise((resolve) => { resolveFirst = resolve; }))
      .mockResolvedValueOnce([
        { id: "s2", conversationId: "c2", runId: "r2", callId: "call2", title: "新来源", url: "https://new.example", sourceLevel: "web", platform: "网页", capturedAt: "2", createdAt: "2" },
      ]);
    const store = useChatStore();
    store.conversations = [
      { id: "c1", title: "旧", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      { id: "c2", title: "新", providerId: "deepseek", createdAt: "2", updatedAt: "2" },
    ];

    const first = store.selectConversation("c1");
    await store.selectConversation("c2");
    resolveFirst([{ id: "s1", conversationId: "c1", runId: "r1", callId: "call1", title: "旧来源", url: "https://old.example", sourceLevel: "web", platform: "网页", capturedAt: "1", createdAt: "1" }]);
    await first;

    expect(store.activeConversationId).toBe("c2");
    expect(store.sources.map((source) => source.url)).toEqual(["https://new.example"]);
  });
});
