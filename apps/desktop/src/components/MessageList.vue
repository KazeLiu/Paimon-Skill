<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { AlertCircle, ExternalLink, FileText, RotateCw } from "@lucide/vue";

import { openBrowserTab } from "@/api/browser";
import MarkdownMessage from "@/components/MarkdownMessage.vue";
import type { AssistantSegment, ConversationStep, Message } from "@/types/chat";
import travelerDialogueAvatarUrl from "@/assets/traveler-dialogue-avatar.png";
import paimonDialogueAvatarUrl from "@/assets/paimon-dialogue-avatar.png";

interface RetryProvider {
  id: string;
  label: string;
}

const props = withDefaults(defineProps<{
  messages: Message[];
  streaming: boolean;
  retrying?: boolean;
  retryNotice?: string;
  providers?: RetryProvider[];
  selectedProviderId?: string | null;
  stepsByRun?: Record<string, ConversationStep[]>;
}>(), { providers: () => [], selectedProviderId: null, stepsByRun: () => ({}) });
const emit = defineEmits<{
  retry: [messageId: string];
  selectProvider: [providerId: string];
}>();
const listRef = ref<HTMLElement | null>(null);
const followsLatest = ref(true);
const activeMessageId = ref("");

const outlineItems = computed(() => {
  const items: Array<{ id: string; questionId: string; question: string; answer: string }> = [];
  props.messages.forEach((message, index) => {
    if (message.role !== "user") return;
    const next = props.messages[index + 1];
    items.push({
      id: `turn:${message.id}`,
      questionId: message.id,
      question: outlineText(message),
      answer: next?.role === "assistant" ? outlineText(next) : "等待 AI 回复…",
    });
  });
  return items;
});

function lastQuestionId() {
  return [...props.messages].reverse().find((message) => message.role === "user")?.id ?? "";
}

function questionIdForMessage(messageId: string) {
  const index = props.messages.findIndex((message) => message.id === messageId);
  if (index < 0) return "";
  for (let current = index; current >= 0; current -= 1) {
    if (props.messages[current].role === "user") return props.messages[current].id;
  }
  return "";
}

async function scrollToBottom() {
  await nextTick();
  const list = listRef.value;
  if (!list) return;
  list.scrollTop = list.scrollHeight;
  activeMessageId.value = lastQuestionId();
}

function handleScroll() {
  const list = listRef.value;
  if (!list) return;
  followsLatest.value = list.scrollHeight - list.scrollTop - list.clientHeight <= 48;
  const marker = list.scrollTop + list.clientHeight * 0.32;
  let current = props.messages[0]?.id ?? "";
  for (const element of list.querySelectorAll<HTMLElement>("[data-message-id]")) {
    if (element.offsetTop <= marker) current = element.dataset.messageId ?? current;
    else break;
  }
  activeMessageId.value = questionIdForMessage(current);
}

function jumpToMessage(id: string) {
  const list = listRef.value;
  const target = Array.from(list?.querySelectorAll<HTMLElement>("[data-message-id]") ?? [])
    .find((element) => element.dataset.messageId === id);
  if (!target) return;
  followsLatest.value = id === lastQuestionId();
  activeMessageId.value = id;
  target.scrollIntoView({ behavior: "smooth", block: "center" });
}

function outlineText(message: Message) {
  const text = message.content
    .replace(/```[\s\S]*?```/g, "代码片段")
    .replace(/[#>*_`\[\]()~-]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  const fallback = message.attachments?.length ? `附件：${message.attachments.map((item) => item.name).join("、")}` : "空消息";
  const summary = text || fallback;
  return summary.length > 90 ? `${summary.slice(0, 90)}…` : summary;
}

watch(
  () => [props.messages.length, props.messages[props.messages.length - 1]?.content],
  () => { if (followsLatest.value) void scrollToBottom(); },
);
onMounted(() => {
  activeMessageId.value = lastQuestionId();
  void scrollToBottom();
});

function formatFileSize(size: number) {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

function errorSummary(error?: string) {
  const summary = error?.split(/\r?\n/, 1)[0]?.trim() || "模型请求未完成";
  return summary.length > 240 ? `${summary.slice(0, 240)}…` : summary;
}

function selectRetryProvider(event: Event) {
  emit("selectProvider", (event.target as HTMLSelectElement).value);
}

function stepStatus(status?: string | null) {
  if (status === "running") return "进行中";
  if (status === "failed") return "失败";
  return "已完成";
}

function assistantSegments(message: Message): AssistantSegment[] {
  if (message.role !== "assistant") return [];
  if (message.segments?.length) return message.segments;
  return message.content ? [{ id: `legacy-text-${message.id}`, kind: "text", content: message.content }] : [];
}

type AssistantBlock =
  | { id: string; kind: "text"; segment: AssistantSegment }
  | { id: string; kind: "tools"; segments: AssistantSegment[] };

function assistantBlocks(message: Message): AssistantBlock[] {
  const blocks: AssistantBlock[] = [];
  for (const segment of assistantSegments(message)) {
    if (segment.kind !== "tool") {
      blocks.push({ id: segment.id, kind: "text", segment });
      continue;
    }
    const previous = blocks[blocks.length - 1];
    if (previous?.kind === "tools") previous.segments.push(segment);
    else blocks.push({ id: `tool-batch-${segment.id}`, kind: "tools", segments: [segment] });
  }
  return blocks;
}

function latestTool(segments: AssistantSegment[]) {
  return segments[segments.length - 1];
}

function hasRunningTool(segments: AssistantSegment[]) {
  return segments.some((segment) => segment.status === "running");
}

function legacyStepsFor(message: Message) {
  return message.role === "assistant" && message.runId ? props.stepsByRun[message.runId] ?? [] : [];
}

async function openSource(url?: string) {
  if (url) await openBrowserTab(url);
}
</script>

<template>
  <div class="message-list-shell">
    <nav v-if="outlineItems.length > 1" class="message-outline" aria-label="当前对话目录">
      <div class="message-outline-list">
        <button
          v-for="item in outlineItems"
          :key="item.id"
          type="button"
          class="message-outline-item"
          :class="{ 'is-active': item.questionId === activeMessageId }"
          data-role="message-outline-item"
          :aria-label="`跳转到问题：${item.question}`"
          @click="jumpToMessage(item.questionId)"
        >
          <i aria-hidden="true"></i>
          <span class="message-outline-card">
            <strong>{{ item.question }}</strong>
            <small>{{ item.answer }}</small>
          </span>
        </button>
      </div>
    </nav>
    <div ref="listRef" class="message-list" aria-live="polite" @scroll="handleScroll">
    <template v-for="message in messages" :key="message.id">
    <article class="chat-message" :class="`is-${message.role}`" :data-message-id="message.id">
      <div class="message-avatar" :class="{ 'is-traveler': message.role === 'user', 'is-paimon': message.role !== 'user' }" aria-hidden="true">
        <img :src="message.role === 'user' ? travelerDialogueAvatarUrl : paimonDialogueAvatarUrl" alt="" />
      </div>
      <div class="message-body">
        <span class="message-author">{{ message.role === "user" ? "旅行者" : "派蒙" }}</span>
        <div v-if="message.attachments?.length" class="message-attachments">
          <template v-for="attachment in message.attachments" :key="`${message.id}:${attachment.name}`">
            <figure v-if="attachment.kind === 'image'" class="message-image">
              <img :src="attachment.dataUrl" :alt="attachment.name" />
              <figcaption>{{ attachment.name }}</figcaption>
            </figure>
            <div v-else class="message-file">
              <FileText :size="18" aria-hidden="true" />
              <span><strong>{{ attachment.name }}</strong><small>{{ formatFileSize(attachment.size) }}</small></span>
            </div>
          </template>
        </div>
        <template v-if="message.role === 'assistant' && assistantSegments(message).length">
          <template v-for="block in assistantBlocks(message)" :key="block.id">
            <MarkdownMessage v-if="block.kind === 'text' && block.segment.content" :content="block.segment.content" />
            <details
              v-else-if="block.kind === 'tools'"
              class="conversation-tool-batch conversation-steps"
              :class="{ 'is-running': hasRunningTool(block.segments) }"
              data-role="conversation-tool-batch"
            >
              <summary>
                <span class="conversation-step-dot" :class="`is-${hasRunningTool(block.segments) ? 'running' : 'completed'}`" aria-hidden="true"></span>
                <span class="tool-batch-label">{{ hasRunningTool(block.segments) ? '正在执行' : '已执行' }}</span>
                <strong class="tool-batch-current">{{ latestTool(block.segments)?.toolName || '查询步骤' }}</strong>
                <span class="tool-batch-count">{{ block.segments.length }} 个工具</span>
              </summary>
              <div class="conversation-tool-list">
                <details v-for="segment in block.segments" :key="segment.id" class="conversation-tool" data-role="conversation-tool">
                  <summary>
                    <span class="conversation-step-dot" :class="`is-${segment.status || 'completed'}`" aria-hidden="true"></span>
                    <strong>{{ segment.toolName || '查询步骤' }}</strong>
                    <span>{{ stepStatus(segment.status) }}</span>
                  </summary>
                  <div class="conversation-step-list">
                    <p v-if="segment.details">{{ segment.details }}</p>
                    <button v-if="segment.url" type="button" @click="openSource(segment.url)"><ExternalLink :size="14" aria-hidden="true" />在新窗口打开</button>
                  </div>
                </details>
              </div>
            </details>
          </template>
        </template>
        <MarkdownMessage v-else-if="message.content" :content="message.content" />
        <template v-if="!message.segments?.length && legacyStepsFor(message).length">
          <details v-for="step in legacyStepsFor(message)" :key="step.id" class="conversation-steps" data-role="conversation-steps">
            <summary>
              <span class="conversation-step-dot" :class="`is-${step.status}`" aria-hidden="true"></span>
              <strong>查询步骤</strong>
              <span>{{ stepStatus(step.status) }}</span>
            </summary>
            <div class="conversation-step-list">
              <strong>{{ step.title }}</strong>
              <p v-if="step.details">{{ step.details }}</p>
              <button v-if="step.url" type="button" @click="openSource(step.url)"><ExternalLink :size="14" aria-hidden="true" />在新窗口打开</button>
            </div>
          </details>
        </template>
        <div
          v-if="retryNotice && message.status === 'streaming' && message.id.startsWith('pending:')"
          class="retry-notice"
          role="status"
          aria-live="polite"
        >
          <RotateCw :size="14" aria-hidden="true" />
          <span>{{ retryNotice }}</span>
        </div>
        <div v-if="message.status === 'error' || message.status === 'cancelled'" class="run-error" :class="{ 'is-cancelled': message.status === 'cancelled' }" role="alert">
          <div class="run-error-heading">
            <AlertCircle :size="18" aria-hidden="true" />
            <strong>{{ message.status === "cancelled" ? "已停止生成" : "请求失败" }}</strong>
          </div>
          <p>{{ errorSummary(message.error) }}</p>
          <details v-if="message.error">
            <summary>查看详细信息</summary>
            <pre>{{ message.error }}</pre>
          </details>
          <div v-if="message.retryable" class="retry-controls">
            <label v-if="providers.length" class="retry-provider-field">
              <span>重试模型</span>
              <select data-role="retry-provider" :value="selectedProviderId || ''" :disabled="retrying" @change="selectRetryProvider">
                <option v-for="provider in providers" :key="provider.id" :value="provider.id">{{ provider.label }}</option>
              </select>
            </label>
            <button
              class="retry-button"
              type="button"
              :aria-label="providers.length ? '使用所选模型重试这条消息' : '重试这条消息'"
              :disabled="retrying || (providers.length > 0 && !selectedProviderId)"
              @click="$emit('retry', message.id)"
            >
              <RotateCw :size="15" aria-hidden="true" />
              使用所选模型重试
            </button>
          </div>
        </div>
        <span v-else-if="message.status === 'streaming' || (streaming && message.id.startsWith('pending:'))" class="typing-indicator" aria-label="正在生成">
          <i></i><i></i><i></i>
        </span>
      </div>
    </article>
    </template>
    <span data-role="message-list-end" aria-hidden="true"></span>
    </div>
  </div>
</template>
