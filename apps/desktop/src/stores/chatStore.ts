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
import type { ChatAttachment, ChatEvent, ChatSourceEvent, Conversation, FailedRunState, Message, SourceRecord, ToolActivity, ToolRunRecord } from "@/types/chat";
import type { ProviderView } from "@/types/providers";

const FAILED_RUNS_STORAGE_KEY = "paimon.failed-runs.v1";

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
    sources: [] as SourceRecord[],
    toolRuns: [] as ToolRunRecord[],
    toolActivities: [] as ToolActivity[],
    bufferedEvents: [] as ChatEvent[],
    bufferedSourceEvents: [] as ChatSourceEvent[],
    bufferedToolEvents: [] as ToolActivity[],
    failedRuns: loadFailedRuns(),
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
        this.selectedProviderId = this.selectedProviderId ?? this.providers[0]?.config.id ?? null;
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
      this.selectedProviderId = provider.config.id;
    },
    removeProvider(id: string) {
      this.providers = this.providers.filter((provider) => provider.config.id !== id);
      if (this.selectedProviderId === id) {
        this.selectedProviderId = this.providers[0]?.config.id ?? null;
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
        const bufferedSources = this.bufferedSourceEvents.filter((event) => event.runId === result.runId);
        const bufferedTools = this.bufferedToolEvents.filter((event) => event.runId === result.runId);
        this.bufferedEvents = [];
        this.bufferedSourceEvents = [];
        this.bufferedToolEvents = [];
        bufferedSources.forEach((event) => this.handleSource(event));
        bufferedTools.forEach((event) => this.handleTool(event));
        buffered
          .filter((event) => event.kind === "message_delta")
          .forEach((event) => this.handleEvent(event));
        buffered
          .filter((event) => event.kind !== "message_delta")
          .forEach((event) => this.handleEvent(event));
        const existing = this.conversations.findIndex((item) => item.id === result.conversation.id);
        if (existing >= 0) this.conversations.splice(existing, 1);
        this.conversations.unshift(result.conversation);
      } catch (error) {
        if (generation !== this.sendGeneration) return;
        this.error = error instanceof Error ? error.message : String(error);
        this.bufferedEvents = [];
        this.bufferedSourceEvents = [];
        this.bufferedToolEvents = [];
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
      if (event.kind === "message_delta" && pending && event.delta) pending.content += event.delta;
      if (event.kind === "assistant_finished") {
        if (pending && event.finalContent != null) pending.content = event.finalContent;
        if (pending && event.messageId) {
          pending.id = event.messageId;
          pending.status = undefined;
          pending.error = undefined;
          pending.retryable = false;
          pending.retryMessageId = undefined;
        }
        this.activeRunId = null;
        this.sending = false;
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
            };
            persistFailedRuns(this.failedRuns);
          }
        }
        this.activeRunId = null;
        this.sending = false;
      }
      if (event.kind === "run_cancelled") {
        this.messages = this.messages.filter((message) => message.id !== `pending:${event.runId}`);
        delete this.failedRuns[event.conversationId];
        persistFailedRuns(this.failedRuns);
        this.activeRunId = null;
        this.sending = false;
      }
    },
    handleSource(event: ChatSourceEvent | SourceRecord) {
      if ("source" in event) {
        if (!this.activeRunId && this.sending && (!this.activeConversationId || event.conversationId === this.activeConversationId)) {
          this.bufferedSourceEvents.push(event);
          return;
        }
        if (!this.activeConversationId || event.conversationId !== this.activeConversationId) return;
        if (!this.activeRunId || event.runId !== this.activeRunId) return;
        event = event.source;
      }
      if (!this.sources.some((item) => item.url === event.url)) this.sources.push(event);
    },
    handleTool(activity: ToolActivity) {
      if (!this.activeRunId && this.sending && (!this.activeConversationId || activity.conversationId === this.activeConversationId)) {
        this.bufferedToolEvents.push(activity);
        return;
      }
      if (!this.activeConversationId || activity.conversationId !== this.activeConversationId) return;
      if (!this.activeRunId || activity.runId !== this.activeRunId) return;
      this.toolActivities.push(activity);
    },
    async cancel() {
      if (this.activeRunId) await cancelRun(this.activeRunId);
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
