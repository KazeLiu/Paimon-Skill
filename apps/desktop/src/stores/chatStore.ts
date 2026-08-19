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
import type { ChatEvent, ChatSourceEvent, Conversation, Message, SourceRecord, ToolActivity, ToolRunRecord } from "@/types/chat";
import type { ProviderView } from "@/types/providers";

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
    async bindEvents() {
      if (this.unlisten) return;
      this.unlisten = await listen<ChatEvent>("chat-event", (event) => this.handleEvent(event.payload));
    },
    async selectConversation(id: string) {
      this.activeConversationId = id;
      this.error = "";
      const [messages, sources, toolRuns] = await Promise.all([
        listMessages(id),
        listSources(id),
        listToolRuns(id),
      ]);
      if (this.activeConversationId !== id) return;
      this.messages = messages;
      this.sources = sources;
      this.toolRuns = toolRuns;
      this.toolActivities = toolRuns.map(({ conversationId, runId, toolName, status }) => ({ conversationId, runId, toolName, status }));
      const conversation = this.conversations.find((item) => item.id === id);
      if (conversation) this.selectedProviderId = conversation.providerId;
    },
    newConversation() {
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
    async send(content: string) {
      if (!this.selectedProviderId) throw new Error("请先配置并选择模型");
      this.sending = true;
      this.error = "";
      try {
        const result = await sendMessage({
          conversationId: this.activeConversationId,
          providerId: this.selectedProviderId,
          content,
        });
        this.activeConversationId = result.conversation.id;
        this.activeRunId = result.runId;
        this.messages.push(result.userMessage, {
          id: `pending:${result.runId}`,
          conversationId: result.conversation.id,
          role: "assistant",
          content: "",
          createdAt: new Date().toISOString(),
        });
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
        this.error = error instanceof Error ? error.message : String(error);
        this.bufferedEvents = [];
        this.bufferedSourceEvents = [];
        this.bufferedToolEvents = [];
        this.sending = false;
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
        if (pending && event.messageId) pending.id = event.messageId;
        this.activeRunId = null;
        this.sending = false;
      }
      if (event.kind === "run_failed") {
        this.error = event.error || "生成失败";
        this.messages = this.messages.filter((message) => message.id !== `pending:${event.runId}`);
        this.activeRunId = null;
        this.sending = false;
      }
      if (event.kind === "run_cancelled") {
        this.messages = this.messages.filter((message) => message.id !== `pending:${event.runId}`);
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
      await deleteConversation(id);
      this.conversations = this.conversations.filter((item) => item.id !== id);
      if (this.activeConversationId === id) this.newConversation();
    },
  },
});
