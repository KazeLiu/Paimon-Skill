import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  cancelRun,
  deleteConversation,
  listConversations,
  listMessages,
  listSources,
  listToolRuns,
  renameConversation,
  sendMessage,
} from "@/api/chat";
import { listProviders } from "@/api/providers";
import type { ChatAttachment, ChatEvent, ChatSourceEvent, Conversation, FailedRunState, Message, StoredSourceRecord, ToolActivity, ToolRunRecord } from "@/types/chat";
import type { ProviderView } from "@/types/providers";

const FAILED_RUNS_STORAGE_KEY = "paimon.failed-runs.v1";
type BufferedRunEvent =
  | { type: "chat"; payload: ChatEvent }
  | { type: "source"; payload: ChatSourceEvent }
  | { type: "tool"; payload: ToolActivity };

function loadFailedRuns(): Record<string, FailedRunState> {
  if (typeof localStorage === "undefined") return {};
  try {
    const stored = JSON.parse(localStorage.getItem(FAILED_RUNS_STORAGE_KEY) || "{}") as Record<string, FailedRunState>;
    return Object.fromEntries(Object.entries(stored).map(([id, failure]) => [id, { ...failure, attachments: [] }])) as Record<string, FailedRunState>;
  } catch {
    return {};
  }
}

function persistFailedRuns(failedRuns: Record<string, FailedRunState>) {
  if (typeof localStorage === "undefined") return;
  try {
    const compact = Object.fromEntries(Object.entries(failedRuns).map(([id, failure]) => [id, { ...failure, attachments: [] }]));
    localStorage.setItem(FAILED_RUNS_STORAGE_KEY, JSON.stringify(compact));
  } catch {
    // Failure state remains in memory; storage quota must not block cleanup.
  }
}

export const useChatStore = defineStore("chat", {
  state: () => ({
    providers: [] as ProviderView[],
    conversations: [] as Conversation[],
    messages: [] as Message[],
    selectedProviderId: null as string | null,
    activeConversationId: null as string | null,
    activeRunId: null as string | null,
    loading: false,
    sending: false,
    error: "",
    sources: [] as StoredSourceRecord[],
    toolRuns: [] as ToolRunRecord[],
    toolActivities: [] as ToolActivity[],
    bufferedEvents: [] as ChatEvent[],
    bufferedSourceEvents: [] as ChatSourceEvent[],
    bufferedToolEvents: [] as ToolActivity[],
    bufferedTimeline: [] as BufferedRunEvent[],
    failedRuns: loadFailedRuns(),
    cancellingRunIds: [] as string[],
    sendGeneration: 0,
    unlisten: null as UnlistenFn | null,
  }),
  getters: {
    selectedProvider(state) {
      return state.providers.find((provider) => provider.config.id === state.selectedProviderId) ?? null;
    },
    activeConversation(state) {
      return state.conversations.find((conversation) => conversation.id === state.activeConversationId) ?? null;
    },
  },
  actions: {
    async initialize() {
      this.loading = true;
      this.error = "";
      try {
        const [providers, conversations] = await Promise.all([listProviders(), listConversations()]);
        this.providers = providers.filter((provider) => provider.config.enabled);
        this.conversations = conversations;
        this.selectedProviderId = this.providers.find((provider) => provider.config.isDefault)?.config.id
          ?? this.selectedProviderId
          ?? this.providers[0]?.config.id
          ?? null;
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
      } finally {
        this.loading = false;
      }
    },
    syncProvider(provider: ProviderView) {
      if (!provider.config.enabled) {
        this.removeProvider(provider.config.id);
        return;
      }
      const index = this.providers.findIndex((item) => item.config.id === provider.config.id);
      if (index >= 0) this.providers.splice(index, 1, provider);
      else this.providers.push(provider);
      if (provider.config.isDefault) {
        this.providers.forEach((item) => {
          if (item.config.id !== provider.config.id) item.config.isDefault = false;
        });
      }
      this.selectedProviderId = provider.config.id;
    },
    removeProvider(id: string) {
      this.providers = this.providers.filter((provider) => provider.config.id !== id);
      if (this.selectedProviderId === id) {
        this.selectedProviderId = this.providers.find((provider) => provider.config.isDefault)?.config.id
          ?? this.providers[0]?.config.id
          ?? null;
      }
    },
    async bindEvents() {
      if (this.unlisten) return;
      try {
        this.unlisten = await listen<ChatEvent>("chat-event", (event) => this.handleEvent(event.payload));
      } catch {
        this.unlisten = null;
      }
    },
    async selectConversation(id: string) {
      if (this.sending) return;
      this.selectedProviderId = this.providers.find((provider) => provider.config.isDefault)?.config.id
        ?? this.providers[0]?.config.id
        ?? null;
      this.activeConversationId = id;
      this.error = "";
      const [messages, sources, toolRuns] = await Promise.all([
        listMessages(id),
        listSources(id),
        listToolRuns(id),
      ]);
      if (this.activeConversationId !== id) return;
      this.messages = messages;
      const failed = this.failedRuns[id];
      if (failed && !this.messages.some((message) => message.id === `failed:${id}`)) {
        this.messages.push({
          id: `failed:${id}`,
          conversationId: id,
          role: "assistant",
          content: failed.partialContent,
          attachments: [],
          status: "error",
          error: failed.error,
          retryable: true,
          retryMessageId: failed.userMessageId,
          createdAt: new Date().toISOString(),
        });
      }
      this.sources = sources;
      this.toolRuns = toolRuns;
      this.toolActivities = toolRuns.map(({ conversationId, runId, toolName, status }) => ({ conversationId, runId, toolName, status }));
      const conversation = this.conversations.find((item) => item.id === id);
      if (conversation) this.selectedProviderId = conversation.providerId;
    },
    newConversation() {
      if (this.sending) return;
      this.activeConversationId = null;
      this.activeRunId = null;
      this.messages = [];
      this.error = "";
      this.sources = [];
      this.toolRuns = [];
      this.toolActivities = [];
      this.bufferedEvents = [];
      this.bufferedSourceEvents = [];
      this.bufferedToolEvents = [];
      this.bufferedTimeline = [];
      this.cancellingRunIds = [];
    },
    async send(content: string, attachments: ChatAttachment[] = []) {
      return this.startSend(content, attachments);
    },
    async retry(messageId: string) {
      if (this.sending) return;
      const failedMessage = this.messages.find((message) => message.id === messageId);
      if (!failedMessage?.retryable || (!failedMessage.retryMessageId && !failedMessage.retryContent)) return;
      const storedFailure = this.activeConversationId ? this.failedRuns[this.activeConversationId] : undefined;
      const userMessage = this.messages.find((message) => message.id === failedMessage.retryMessageId)
        ?? (storedFailure ? {
          id: storedFailure.userMessageId,
          conversationId: storedFailure.conversationId,
          role: "user",
          content: storedFailure.content,
          attachments: storedFailure.attachments,
          createdAt: new Date().toISOString(),
        } : undefined);
      if (!userMessage && !failedMessage.retryContent) {
        failedMessage.error = "无法找到原始消息，请重新发送";
        return;
      }
      await this.startSend(
        userMessage?.content ?? failedMessage.retryContent ?? "",
        userMessage?.attachments ?? failedMessage.retryAttachments ?? [],
        userMessage ? failedMessage.retryMessageId : undefined,
        failedMessage.id,
      );
    },
    async startSend(
      content: string,
      attachments: ChatAttachment[] = [],
      retryMessageId?: string,
      failedMessageId?: string,
    ) {
      if (!this.selectedProviderId) throw new Error("请先配置并选择模型");
      if (!content.trim() && attachments.length === 0) throw new Error("消息或附件不能为空");
      this.sending = true;
      const generation = ++this.sendGeneration;
      this.error = "";
      if (!retryMessageId) {
        this.messages.forEach((message) => {
          if (message.status === "error") message.retryable = false;
        });
      }
      try {
        const result = await sendMessage({
          conversationId: this.activeConversationId,
          providerId: this.selectedProviderId,
          content,
          attachments,
          retryMessageId,
        });
        if (generation !== this.sendGeneration) return;
        this.activeConversationId = result.conversation.id;
        this.activeRunId = result.runId;
        const pending: Message = {
          id: `pending:${result.runId}`,
          conversationId: result.conversation.id,
          role: "assistant",
          content: "",
          attachments: [],
          status: "streaming",
          retryMessageId: result.userMessage.id,
          runId: result.runId,
          createdAt: new Date().toISOString(),
        };
        if (failedMessageId) {
          const failedIndex = this.messages.findIndex((message) => message.id === failedMessageId);
          const replacement = retryMessageId ? [pending] : [result.userMessage, pending];
          if (failedIndex >= 0) this.messages.splice(failedIndex, 1, ...replacement);
          else this.messages.push(...replacement);
        } else {
          this.messages.push(result.userMessage, pending);
        }
        delete this.failedRuns[result.conversation.id];
        persistFailedRuns(this.failedRuns);
        const buffered = this.bufferedEvents.filter((event) => event.runId === result.runId);
        this.bufferedEvents = [];
        this.bufferedSourceEvents = [];
        this.bufferedToolEvents = [];
        const timeline = this.bufferedTimeline.filter((event) => event.payload.runId === result.runId);
        this.bufferedTimeline = [];
        timeline.forEach((event) => {
          if (event.type === "chat") this.handleEvent(event.payload);
          else if (event.type === "source") this.handleSource(event.payload);
          else this.handleTool(event.payload);
        });
        if (!timeline.length) buffered.forEach((event) => this.handleEvent(event));
        const existing = this.conversations.findIndex((item) => item.id === result.conversation.id);
        if (existing >= 0) this.conversations.splice(existing, 1);
        this.conversations.unshift(result.conversation);
      } catch (error) {
        if (generation !== this.sendGeneration) return;
        this.error = error instanceof Error ? error.message : String(error);
        this.bufferedEvents = [];
        this.bufferedSourceEvents = [];
        this.bufferedToolEvents = [];
        this.bufferedTimeline = [];
        this.sending = false;
        if (!retryMessageId) {
          const failureId = `failed:local-${generation}`;
          this.messages.push({
            id: failureId,
            conversationId: this.activeConversationId ?? failureId,
            role: "assistant",
            content: "",
            attachments: [],
            status: "error",
            error: this.error,
            retryable: true,
            retryContent: content,
            retryAttachments: attachments,
            createdAt: new Date().toISOString(),
          });
        }
        throw error;
      }
    },
    handleEvent(event: ChatEvent) {
      if (!this.activeRunId && this.sending && (!this.activeConversationId || event.conversationId === this.activeConversationId)) {
        this.bufferedEvents.push(event);
        this.bufferedTimeline.push({ type: "chat", payload: event });
        return;
      }
      if (event.runId !== this.activeRunId) return;
      if (event.conversationId !== this.activeConversationId) {
        if (event.kind === "assistant_finished" || event.kind === "run_failed" || event.kind === "run_cancelled") {
          this.activeRunId = null;
          this.sending = false;
          if (event.kind === "run_failed") this.error = event.error || "生成失败";
        }
        return;
      }
      const pending = this.messages.find((message) => message.id === `pending:${event.runId}`);
      if (event.kind === "message_delta" && pending && event.delta) {
        pending.content += event.delta;
        const segments = pending.segments ?? (pending.segments = []);
        const last = segments[segments.length - 1];
        if (last?.kind === "text") last.content += event.delta;
        else segments.push({ id: `text-live-${segments.length + 1}`, kind: "text", content: event.delta });
      }
      if (event.kind === "assistant_finished") {
        if (pending && event.finalContent != null) pending.content = event.finalContent;
        if (pending && event.segments) pending.segments = event.segments;
        if (pending && event.messageId) {
          pending.id = event.messageId;
          pending.status = undefined;
          pending.error = undefined;
          pending.retryable = false;
          pending.retryMessageId = undefined;
        }
        this.error = "";
        this.activeRunId = null;
        this.sending = false;
      }
      if (event.kind === "run_retrying") {
        if (this.cancellingRunIds.includes(event.runId)) return;
        if (pending) {
          const lastSegment = pending.segments?.[pending.segments.length - 1];
          if (lastSegment?.kind === "text") pending.segments?.pop();
          pending.content = pending.segments?.filter((segment) => segment.kind === "text").map((segment) => segment.content).join("\n\n") || "";
          pending.status = "streaming";
          pending.error = undefined;
          pending.retryable = false;
        }
        this.error = event.error ? `请求失败，正在进行第 ${event.delta ?? "下一次"} 次重试…` : "正在重试…";
        return;
      }
      if (event.kind === "run_failed") {
        this.error = event.error || "生成失败";
        if (pending) {
          pending.status = "error";
          pending.error = this.error;
          pending.retryable = true;
          const userMessage = pending.retryMessageId
            ? this.messages.find((message) => message.id === pending.retryMessageId)
            : undefined;
          if (userMessage && this.selectedProviderId) {
            this.failedRuns[event.conversationId] = {
              conversationId: event.conversationId,
              providerId: this.selectedProviderId,
              userMessageId: userMessage.id,
              content: userMessage.content,
              attachments: userMessage.attachments ?? [],
              partialContent: pending.content,
              error: this.error,
              runId: event.runId,
            };
            persistFailedRuns(this.failedRuns);
          }
        }
        this.activeRunId = null;
        this.sending = false;
      }
      if (event.kind === "run_cancelled") {
        if (pending) {
          pending.status = "cancelled";
          pending.error = "已停止生成，可以重试这条回复";
          pending.retryable = true;
        }
        delete this.failedRuns[event.conversationId];
        persistFailedRuns(this.failedRuns);
        this.cancellingRunIds = this.cancellingRunIds.filter((runId) => runId !== event.runId);
        this.error = "";
        this.activeRunId = null;
        this.sending = false;
      }
    },
    handleSource(event: ChatSourceEvent | StoredSourceRecord) {
      if ("source" in event) {
        if (!this.activeRunId && this.sending && (!this.activeConversationId || event.conversationId === this.activeConversationId)) {
          this.bufferedSourceEvents.push(event);
          this.bufferedTimeline.push({ type: "source", payload: event });
          return;
        }
        if (!this.activeConversationId || event.conversationId !== this.activeConversationId) return;
        if (!this.activeRunId || event.runId !== this.activeRunId) return;
        event = {
          ...event.source,
          id: `live:${event.runId}:${event.source.url}`,
          conversationId: event.conversationId,
          runId: event.runId,
          callId: "",
          createdAt: event.source.capturedAt,
        };
      }
      if (!this.sources.some((item) => item.runId === event.runId && item.url === event.url)) this.sources.push(event);
    },
    handleTool(activity: ToolActivity) {
      if (!this.activeRunId && this.sending && (!this.activeConversationId || activity.conversationId === this.activeConversationId)) {
        this.bufferedToolEvents.push(activity);
        this.bufferedTimeline.push({ type: "tool", payload: activity });
        return;
      }
      if (!this.activeConversationId || activity.conversationId !== this.activeConversationId) return;
      if (!this.activeRunId || activity.runId !== this.activeRunId) return;
      this.toolActivities.push(activity);
      const pending = this.messages.find((message) => message.id === `pending:${activity.runId}`);
      if (!pending) return;
      const segments = pending.segments ?? (pending.segments = []);
      const existing = activity.callId ? segments.find((segment) => segment.id === `tool-${activity.callId}`) : undefined;
      if (existing) {
        existing.status = activity.status;
        existing.details = activity.details;
        existing.url = activity.url;
        existing.platform = activity.platform;
      } else if (activity.status !== "running" || activity.callId) {
        segments.push({
          id: activity.callId ? `tool-${activity.callId}` : `tool-live-${segments.length + 1}`,
          kind: "tool",
          content: "",
          toolName: activity.toolName,
          status: activity.status,
          details: activity.details,
          url: activity.url,
          platform: activity.platform,
        });
      }
    },
    async cancel() {
      const runId = this.activeRunId;
      if (!runId) return;
      if (!this.cancellingRunIds.includes(runId)) this.cancellingRunIds.push(runId);
      this.error = "";
      try {
        const cancelled = await cancelRun(runId);
        if (!cancelled) this.cancellingRunIds = this.cancellingRunIds.filter((id) => id !== runId);
      } catch (error) {
        this.cancellingRunIds = this.cancellingRunIds.filter((id) => id !== runId);
        this.error = error instanceof Error ? error.message : String(error);
        throw error;
      }
    },
    async rename(id: string, title: string) {
      await renameConversation(id, title);
      const conversation = this.conversations.find((item) => item.id === id);
      if (conversation) conversation.title = title;
    },
    async remove(id: string) {
      try {
        if (this.activeConversationId === id && this.sending && !this.activeRunId) {
          this.error = "请求正在启动，请稍候再删除当前对话";
          return;
        }
        if (this.activeConversationId === id && this.activeRunId) {
          await this.cancel();
          this.sendGeneration += 1;
          this.activeRunId = null;
          this.sending = false;
          this.bufferedEvents = [];
          this.bufferedSourceEvents = [];
          this.bufferedToolEvents = [];
        }
        await deleteConversation(id);
        this.conversations = this.conversations.filter((item) => item.id !== id);
        delete this.failedRuns[id];
        persistFailedRuns(this.failedRuns);
        if (this.activeConversationId === id) this.newConversation();
      } catch (error) {
        this.error = error instanceof Error ? error.message : String(error);
        throw error;
      }
    },
  },
});
