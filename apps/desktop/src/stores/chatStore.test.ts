import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { useChatStore } from "./chatStore";
import { listConversations, listMessages, sendMessage } from "@/api/chat";
import { listProviders } from "@/api/providers";

vi.mock("@/api/chat", () => ({
  listConversations: vi.fn(),
  listMessages: vi.fn(),
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
    vi.mocked(listProviders).mockResolvedValue([{ config: {
      id: "deepseek", name: "DeepSeek", baseUrl: "https://api.deepseek.com", model: "deepseek-chat",
      secretRef: "provider:deepseek", timeoutMs: 30000, temperature: 0.3, maxTokens: 2048,
      streaming: true, toolCalling: true, enabled: true,
    }, hasApiKey: true }]);
    vi.mocked(listConversations).mockResolvedValue([]);
    vi.mocked(listMessages).mockResolvedValue([]);
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

  it.each(["run_failed", "run_cancelled"] as const)("removes unpersisted assistant content on %s", async (kind) => {
    vi.mocked(sendMessage).mockResolvedValue({
      runId: "run-1",
      conversation: { id: "c1", title: "测试", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
      userMessage: { id: "m1", conversationId: "c1", role: "user", content: "你好", createdAt: "1" },
    });
    const store = useChatStore();
    await store.initialize();
    await store.send("你好");
    store.handleEvent({ kind: "message_delta", runId: "run-1", conversationId: "c1", delta: "半截", messageId: null, error: null });
    store.handleEvent({ kind, runId: "run-1", conversationId: "c1", delta: null, messageId: null, error: kind === "run_failed" ? "timeout" : null });

    expect(store.messages.map((message) => message.content)).toEqual(["你好"]);
    expect(store.sending).toBe(false);
  });

  it("finishes the active run after switching to another conversation", async () => {
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
    expect(store.messages).toEqual([]);
  });
});
