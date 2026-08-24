<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  Bot,
  Check,
  ChevronDown,
  FileText,
  Globe2,
  MessageSquarePlus,
  Paperclip,
  PanelRightOpen,
  PanelRightClose,
  Search,
  Send,
  Settings,
  Sparkles,
  Square,
  Trash2,
  X,
} from "@lucide/vue";
import PromptSettingsPage from "@/pages/PromptSettingsPage.vue";
import PrivacySettingsPage from "@/pages/PrivacySettingsPage.vue";
import ModelSettingsPage from "@/pages/ModelSettingsPage.vue";
import DiagnosticsSettingsPage from "@/pages/DiagnosticsSettingsPage.vue";
import MessageList from "@/components/MessageList.vue";
import SourceDirectoryPage from "@/pages/SourceDirectoryPage.vue";
import { openBrowserTab } from "@/api/browser";
import { useChatStore } from "@/stores/chatStore";
import type { ChatAttachment, ChatSourceEvent, Conversation, ToolActivity, ToolRunRecord } from "@/types/chat";

type ViewId = "chat" | "sources" | "settings";
type SettingsSection = "models" | "prompts" | "privacy" | "diagnostics";

const activeView = ref<ViewId>("chat");
const settingsSection = ref<SettingsSection>("models");
const draft = ref("");
const attachments = ref<ChatAttachment[]>([]);
const attachmentInput = ref<HTMLInputElement | null>(null);
const composerError = ref("");
const conversationError = ref("");
const conversationSearch = ref("");
const activityPaneVisible = ref(true);
const modelMenuOpen = ref(false);
const modelSelectorRef = ref<HTMLElement | null>(null);
const chatStore = useChatStore();
const {
  providers,
  conversations,
  messages,
  selectedProviderId,
  activeConversationId,
  activeRunId,
  sending,
  error,
  sources,
  toolActivities,
  toolRuns,
} = storeToRefs(chatStore);
let unlistenSource: UnlistenFn | null = null;
let unlistenTool: UnlistenFn | null = null;
const activeTitle = computed(() => {
  if (activeView.value === "chat") return chatStore.activeConversation?.title ?? "新对话";
  if (activeView.value === "sources") return "信息源";
  return "设置";
});

const filteredConversations = computed(() => {
  const query = conversationSearch.value.trim().toLocaleLowerCase();
  if (!query) return conversations.value;
  return conversations.value.filter((conversation) => conversation.title.toLocaleLowerCase().includes(query));
});

type ActivityItem = {
  id: string;
  kind: "source" | "tool";
  timestamp: string;
  title: string;
  platform?: string;
  url?: string;
  status?: string;
  details?: string;
  sourceLevel?: string;
  runId?: string;
};

type ActivityGroup =
  | (ActivityItem & { kind: "source"; steps?: undefined })
  | {
      id: string;
      kind: "tool";
      timestamp: string;
      title: string;
      status: string;
      steps: ActivityItem[];
    };

const activityItems = computed<ActivityItem[]>(() => {
  const items: ActivityItem[] = sources.value.map((source) => ({
    id: `source:${source.url}:${source.capturedAt}`,
    kind: "source",
    timestamp: source.capturedAt,
    title: source.title,
    platform: source.platform,
    url: source.url,
    sourceLevel: source.sourceLevel,
  }));
  const persisted = (toolRuns?.value ?? []).map((run: ToolRunRecord) => ({
    id: `tool:${run.id}`,
    kind: "tool" as const,
    timestamp: run.createdAt,
    title: run.toolName,
    runId: run.runId,
    status: run.status,
    details: run.result?.content || run.error || run.arguments,
  }));
  const live = toolActivities.value
    .filter((activity) => !persisted.some((run) => run.runId === activity.runId && run.title === activity.toolName))
    .map((activity) => ({
      id: `tool:${activity.runId}:${activity.toolName}`,
      kind: "tool" as const,
      timestamp: activity.createdAt || new Date().toISOString(),
      title: activity.toolName,
      status: activity.status,
      details: activity.details,
    }));
  return [...items, ...persisted, ...live].sort((a, b) => Date.parse(b.timestamp) - Date.parse(a.timestamp));
});

const activityGroups = computed<ActivityGroup[]>(() => {
  const groups: ActivityGroup[] = [];
  for (const item of activityItems.value) {
    if (item.kind === "tool") {
      const previous = groups[groups.length - 1];
      if (previous?.kind === "tool") {
        previous.steps.push(item);
        previous.timestamp = item.timestamp > previous.timestamp ? item.timestamp : previous.timestamp;
        previous.status = previous.steps.some((step) => step.status === "running")
          ? "running"
          : previous.steps.some((step) => step.status === "failed") ? "failed" : "completed";
        if (previous.title !== item.title) previous.title = "工具流程";
        continue;
      }
      groups.push({
        id: `workflow:${item.id}`,
        kind: "tool",
        timestamp: item.timestamp,
        title: item.title,
        status: item.status || "completed",
        steps: [item],
      });
      continue;
    }
    groups.push(item as ActivityGroup);
  }
  return groups;
});

async function openActivityUrl(url?: string) {
  if (!url) return;
  await openBrowserTab(url);
}

async function send() {
  const content = draft.value.trim();
  if ((!content && attachments.value.length === 0) || sending.value) return;
  const outgoingAttachments = attachments.value.slice();
  const sent = await Promise.resolve(chatStore.send(content, outgoingAttachments)).then(() => true).catch(() => false);
  if (sent) {
    draft.value = "";
    attachments.value = [];
    composerError.value = "";
    if (attachmentInput.value) attachmentInput.value.value = "";
  }
  await nextTick();
}

const MAX_ATTACHMENT_BYTES = 10 * 1024 * 1024;
const MAX_TOTAL_ATTACHMENT_BYTES = 20 * 1024 * 1024;
const MAX_ATTACHMENTS = 6;
const SUPPORTED_FILE_EXTENSIONS = new Set([
  "pdf", "txt", "md", "csv", "json", "xml", "yaml", "yml",
  "doc", "docx", "ppt", "pptx", "xls", "xlsx",
]);
const SUPPORTED_IMAGE_MIME_TYPES = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);
const FILE_MIME_TYPES: Record<string, string> = {
  pdf: "application/pdf",
  txt: "text/plain",
  md: "text/markdown",
  csv: "text/csv",
  json: "application/json",
  xml: "application/xml",
  yaml: "application/yaml",
  yml: "application/yaml",
  doc: "application/msword",
  docx: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
  ppt: "application/vnd.ms-powerpoint",
  pptx: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  xls: "application/vnd.ms-excel",
  xlsx: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
};

function attachmentKind(file: File): ChatAttachment["kind"] | null {
  if (SUPPORTED_IMAGE_MIME_TYPES.has(file.type.toLowerCase())) return "image";
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (file.type.startsWith("text/") || SUPPORTED_FILE_EXTENSIONS.has(extension)) return "file";
  return null;
}

function readFile(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error(`无法读取文件：${file.name}`));
    reader.readAsDataURL(file);
  });
}

async function handleAttachments(event: Event) {
  const input = event.target as HTMLInputElement;
  const files = Array.from(input.files ?? []);
  composerError.value = "";
  if (attachments.value.length + files.length > MAX_ATTACHMENTS) {
    composerError.value = `最多添加 ${MAX_ATTACHMENTS} 个附件`;
    input.value = "";
    return;
  }
  for (const file of files) {
    const kind = attachmentKind(file);
    if (!kind) {
      composerError.value = `暂不支持该文件类型：${file.name}`;
      continue;
    }
    if (file.size > MAX_ATTACHMENT_BYTES) {
      composerError.value = `单个附件不能超过 10 MB：${file.name}`;
      continue;
    }
    const currentTotal = attachments.value.reduce((total, item) => total + item.size, 0);
    if (currentTotal + file.size > MAX_TOTAL_ATTACHMENT_BYTES) {
      composerError.value = "附件总大小不能超过 20 MB";
      break;
    }
    try {
      const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
      const mimeType = file.type || FILE_MIME_TYPES[extension] || "application/octet-stream";
      attachments.value.push({
        name: file.name,
        mimeType,
        dataUrl: await readFile(file),
        size: file.size,
        kind,
      });
    } catch (fileError) {
      composerError.value = fileError instanceof Error ? fileError.message : String(fileError);
    }
  }
  input.value = "";
}

function removeAttachment(index: number) {
  attachments.value.splice(index, 1);
}

async function confirmDeleteConversation(conversation: Conversation) {
  if (!window.confirm(`确定删除对话“${conversation.title}”吗？删除后无法恢复。`)) return;
  conversationError.value = "";
  await Promise.resolve(chatStore.remove(conversation.id)).catch((deleteError) => {
    conversationError.value = deleteError instanceof Error ? deleteError.message : String(deleteError);
  });
}

async function retryMessage(messageId: string) {
  await Promise.resolve(chatStore.retry(messageId)).catch(() => undefined);
}

function handleComposerKeydown(event: KeyboardEvent) {
  if (event.key === "Enter" && !event.shiftKey) {
    event.preventDefault();
    void send();
  }
}

function openModels() {
  activeView.value = "settings";
  settingsSection.value = "models";
}

function selectModel(providerId: string) {
  selectedProviderId.value = providerId;
  modelMenuOpen.value = false;
}

function handleModelMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    modelMenuOpen.value = false;
    return;
  }
  if (!modelMenuOpen.value || !providers.value.length) return;
  const currentIndex = providers.value.findIndex((provider) => provider.config.id === selectedProviderId.value);
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const direction = event.key === "ArrowDown" ? 1 : -1;
    const nextIndex = (currentIndex + direction + providers.value.length) % providers.value.length;
    selectModel(providers.value[nextIndex].config.id);
  }
}

function handleDocumentPointerdown(event: PointerEvent) {
  if (modelSelectorRef.value && !modelSelectorRef.value.contains(event.target as Node)) {
    modelMenuOpen.value = false;
  }
}

onMounted(async () => {
  document.addEventListener("pointerdown", handleDocumentPointerdown);
  await Promise.all([chatStore.initialize(), chatStore.bindEvents()]);
  unlistenSource = await listen<ChatSourceEvent>("chat-source", (event) => chatStore.handleSource(event.payload)).catch(() => null);
  unlistenTool = await listen<ToolActivity>("chat-tool", (event) => chatStore.handleTool(event.payload)).catch(() => null);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerdown);
  unlistenSource?.();
  unlistenTool?.();
});
</script>

<template>
  <div class="app-shell" :class="{ 'activity-pane-collapsed': !activityPaneVisible || activeView !== 'chat' }">
    <aside class="conversation-rail" aria-label="对话导航">
      <div class="brand-block">
        <div class="brand-mark" aria-hidden="true">
          <Sparkles :size="18" />
        </div>
        <div>
          <strong>派蒙助手</strong>
          <span>桌面研究工作台</span>
        </div>
      </div>

      <button class="primary-command" type="button" @click="activeView = 'chat'; chatStore.newConversation()">
        <MessageSquarePlus :size="17" />
        新建对话
      </button>

      <button class="rail-browser" type="button" data-action="open-sources" :class="{ 'is-active': activeView === 'sources' }" @click="activeView = 'sources'">
        <Globe2 :size="17" />
        信息源
      </button>

      <label class="search-field">
        <Search :size="16" aria-hidden="true" />
        <span class="sr-only">搜索对话</span>
        <input v-model="conversationSearch" type="search" placeholder="搜索对话" />
      </label>

      <nav class="conversation-list" aria-label="最近对话">
        <span class="section-label">最近</span>
        <p v-if="conversationError" class="conversation-error" role="alert">{{ conversationError }}</p>
        <div
          v-for="conversation in filteredConversations"
          :key="conversation.id"
          class="conversation-row"
          :class="{ 'is-active': conversation.id === activeConversationId }"
        >
          <button type="button" class="conversation-item" @click="activeView = 'chat'; chatStore.selectConversation(conversation.id)">
            <span>{{ conversation.title }}</span>
            <small>{{ new Date(conversation.updatedAt).toLocaleDateString() }}</small>
          </button>
          <button
            class="conversation-delete"
            type="button"
            title="删除对话"
            :aria-label="`删除对话：${conversation.title}`"
            @click.stop="confirmDeleteConversation(conversation)"
          >
            <Trash2 :size="14" />
          </button>
        </div>
        <span v-if="conversations.length === 0" class="conversation-empty">暂无历史对话</span>
        <span v-else-if="filteredConversations.length === 0" class="conversation-empty">没有匹配的对话</span>
      </nav>

      <button class="rail-settings" type="button" @click="activeView = 'settings'">
        <Settings :size="17" />
        设置
      </button>
    </aside>

    <main class="workspace">
      <header v-if="activeView !== 'sources'" class="workspace-header">
        <div>
          <span class="eyebrow">当前会话</span>
          <h1>{{ activeTitle }}</h1>
        </div>
        <div v-if="activeView === 'chat'" class="header-actions">
          <div ref="modelSelectorRef" class="model-selector" :class="{ 'is-open': modelMenuOpen }">
            <span class="status-dot" :class="{ 'is-ready': selectedProviderId }" aria-hidden="true"></span>
            <button
              class="model-selector-trigger"
              type="button"
              data-action="model-selector"
              aria-label="选择模型"
              aria-haspopup="listbox"
              :aria-expanded="modelMenuOpen"
              :disabled="providers.length === 0 || sending"
              @click="modelMenuOpen = !modelMenuOpen"
              @keydown="handleModelMenuKeydown"
            >
              <span class="model-selector-value">
                <template v-if="chatStore.selectedProvider">
                  {{ chatStore.selectedProvider.config.name }} · {{ chatStore.selectedProvider.config.model }}
                </template>
                <template v-else>未配置模型</template>
              </span>
              <ChevronDown :size="15" aria-hidden="true" />
            </button>
            <div v-if="modelMenuOpen" class="model-menu" role="listbox" aria-label="可用模型">
              <button
                v-for="provider in providers"
                :key="provider.config.id"
                class="model-menu-option"
                :class="{ 'is-selected': provider.config.id === selectedProviderId }"
                type="button"
                role="option"
                :data-provider-id="provider.config.id"
                :aria-selected="provider.config.id === selectedProviderId"
                @click="selectModel(provider.config.id)"
              >
                <span>
                  <strong>{{ provider.config.name }}</strong>
                  <small>{{ provider.config.model }}</small>
                </span>
                <Check v-if="provider.config.id === selectedProviderId" :size="15" aria-hidden="true" />
              </button>
            </div>
          </div>
          <button
            class="icon-button"
            type="button"
            :title="activityPaneVisible ? '收起查询面板' : '打开查询面板'"
            :aria-label="activityPaneVisible ? '收起查询面板' : '打开查询面板'"
            @click="activityPaneVisible = !activityPaneVisible"
          >
            <PanelRightOpen v-if="!activityPaneVisible" :size="18" />
            <PanelRightClose v-else :size="18" />
          </button>
        </div>
      </header>

      <section v-if="activeView === 'chat'" class="chat-surface">
        <div v-if="messages.length === 0" class="empty-conversation">
          <div class="assistant-avatar" aria-hidden="true">
            <Bot :size="26" />
          </div>
          <h2>你好旅行者，有什么问题吗？</h2>
          <p>我可以帮你查配队，看圣遗物，聊一聊旅途中的故事。</p>
        </div>
        <MessageList
          v-else
          :messages="messages"
          :streaming="Boolean(activeRunId)"
          :retrying="sending"
          :providers="providers.map((provider) => ({ id: provider.config.id, label: `${provider.config.name} · ${provider.config.model}` }))"
          :selected-provider-id="selectedProviderId"
          @select-provider="selectModel"
          @retry="retryMessage"
        />

        <form class="composer" @submit.prevent="send">
          <input
            ref="attachmentInput"
            class="sr-only"
            type="file"
            multiple
            accept="image/*,.pdf,.txt,.md,.csv,.json,.xml,.yaml,.yml,.doc,.docx,.ppt,.pptx,.xls,.xlsx"
            :disabled="sending"
            @change="handleAttachments"
          />
          <div v-if="attachments.length" class="composer-attachments" aria-label="待发送附件">
            <div v-for="(attachment, index) in attachments" :key="`${attachment.name}:${index}`" class="composer-attachment">
              <img v-if="attachment.kind === 'image'" :src="attachment.dataUrl" alt="" />
              <FileText v-else :size="18" aria-hidden="true" />
              <span>{{ attachment.name }}</span>
              <button
                type="button"
                :aria-label="`移除附件：${attachment.name}`"
                :disabled="sending"
                @click="removeAttachment(index)"
              >
                <X :size="14" aria-hidden="true" />
              </button>
            </div>
          </div>
          <textarea
            v-model="draft"
            rows="3"
            placeholder="输入问题，Enter 发送，Shift + Enter 换行"
            @keydown="handleComposerKeydown"
          ></textarea>
          <div class="composer-footer">
            <div class="composer-status">
              <button
                class="attachment-button"
                type="button"
                aria-label="添加图片或文件"
                :title="selectedProviderId ? '添加图片或文件（是否可识别由当前模型决定）' : '请先选择模型'"
                :disabled="sending || !selectedProviderId"
                @click="attachmentInput?.click()"
              >
                <Paperclip :size="18" aria-hidden="true" />
              </button>
              <span :class="{ 'is-error': error || composerError }">{{ composerError || error || (selectedProviderId ? "已准备 · 可添加图片或文件" : "请先选择模型") }}</span>
            </div>
            <button v-if="sending" class="send-button" type="button" aria-label="停止生成" @click="chatStore.cancel">
              <Square :size="16" />
            </button>
            <button v-else class="send-button" type="submit" :disabled="(!draft.trim() && attachments.length === 0) || !selectedProviderId" aria-label="发送消息">
              <Send :size="18" />
            </button>
          </div>
        </form>
      </section>

      <SourceDirectoryPage v-else-if="activeView === 'sources'" />

      <section v-else class="settings-workspace">
        <nav class="settings-tabs" aria-label="设置分类">
          <button
            type="button"
            :class="{ 'is-active': settingsSection === 'models' }"
            @click="settingsSection = 'models'"
          >
            模型
          </button>
          <button
            type="button"
            :class="{ 'is-active': settingsSection === 'prompts' }"
            @click="settingsSection = 'prompts'"
          >
            系统提示词
          </button>
          <button
            type="button"
            :class="{ 'is-active': settingsSection === 'privacy' }"
            @click="settingsSection = 'privacy'"
          >
            隐私与数据
          </button>
          <button
            type="button"
            data-settings-section="diagnostics"
            :class="{ 'is-active': settingsSection === 'diagnostics' }"
            @click="settingsSection = 'diagnostics'"
          >
            日志
          </button>
        </nav>
        <ModelSettingsPage
          v-if="settingsSection === 'models'"
          @provider-updated="chatStore.syncProvider"
          @provider-deleted="chatStore.removeProvider"
        />
        <PromptSettingsPage v-else-if="settingsSection === 'prompts'" />
        <PrivacySettingsPage v-else-if="settingsSection === 'privacy'" />
        <DiagnosticsSettingsPage v-else />
      </section>
    </main>

    <aside v-if="activityPaneVisible && activeView === 'chat'" class="activity-pane" aria-label="查询活动">
      <div class="activity-heading">
        <div>
          <span class="eyebrow">研究过程</span>
          <h2>查询活动</h2>
        </div>
        <span class="activity-count">{{ activityGroups.length }}</span>
      </div>
      <div v-if="activityGroups.length === 0" class="activity-empty">
        <Sparkles :size="22" />
        <strong>暂无查询</strong>
        <span>网页、工具调用和来源会显示在这里。</span>
      </div>
      <div v-else class="activity-list">
        <details v-for="item in activityGroups" :key="item.id" class="activity-item" data-role="activity-item" :data-url="item.kind === 'source' ? item.url || undefined : undefined">
          <summary>
            <span class="activity-kind">{{ item.kind === "source" ? item.platform : "工具" }}</span>
            <strong>{{ item.title }}<template v-if="item.kind === 'tool' && item.steps.length > 1"> · {{ item.steps.length }} 次</template></strong>
            <small>{{ item.kind === "source" ? item.sourceLevel : (item.status === "completed" ? "已完成" : item.status === "running" ? "进行中" : "有失败") }} · {{ new Date(item.timestamp).toLocaleTimeString() }}</small>
          </summary>
          <div class="activity-details">
            <template v-if="item.kind === 'source'">
              <p v-if="item.url"><strong>URL：</strong>{{ item.url }}</p>
              <p v-if="item.details">{{ item.details }}</p>
              <button v-if="item.url" type="button" class="activity-open-link" @click="openActivityUrl(item.url)">在新窗口打开网页</button>
            </template>
            <template v-else>
              <div v-for="(step, index) in item.steps" :key="`${step.id}:${index}`" class="activity-step" data-role="activity-step">
                <strong>{{ step.title }}</strong>
                <small>{{ step.status === "completed" ? "已完成" : step.status === "running" ? "进行中" : "失败" }} · {{ new Date(step.timestamp).toLocaleTimeString() }}</small>
                <p v-if="step.details">{{ step.details }}</p>
              </div>
            </template>
          </div>
        </details>
      </div>
    </aside>
  </div>
</template>
