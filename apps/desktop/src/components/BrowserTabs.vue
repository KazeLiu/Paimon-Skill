<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import {
  ArrowLeft,
  ArrowRight,
  ChevronsDown,
  Globe2,
  MessageSquareText,
  MousePointerClick,
  Plus,
  RefreshCw,
  ScanText,
  X,
} from "@lucide/vue";

import { useBrowserStore } from "@/stores/browserStore";
import type { BrowserBounds, PageSnapshot } from "@/types/browser";

const emit = defineEmits<{
  (event: "ask-page", snapshot: PageSnapshot): void;
}>();

const store = useBrowserStore();
const { tabs, activeTabId, activeTab, loadingAction, error, lastSnapshot } = storeToRefs(store);
const address = ref("https://wiki.biligame.com/ys/");
const clickTarget = ref("");
const viewport = ref<HTMLElement | null>(null);
let resizeObserver: ResizeObserver | null = null;

function currentBounds(): BrowserBounds {
  const rect = viewport.value?.getBoundingClientRect();
  return {
    x: Math.max(0, rect?.left ?? 0),
    y: Math.max(0, rect?.top ?? 0),
    width: Math.max(1, rect?.width || 800),
    height: Math.max(1, rect?.height || 600),
  };
}

async function syncBounds() {
  await nextTick();
  await store.setBounds(currentBounds()).catch(() => undefined);
}

async function openAddress() {
  const value = address.value.trim();
  if (!value) return;
  if (activeTabId.value) await store.navigate(value);
  else await store.open(value, currentBounds());
}

async function createTab() {
  address.value = "https://wiki.biligame.com/ys/";
  await store.open(address.value, currentBounds());
}

async function askCurrentPage() {
  const snapshot = await store.extract();
  if (snapshot) emit("ask-page", snapshot);
}

async function clickVisibleTarget() {
  const target = clickTarget.value.trim();
  if (!target) return;
  await store.click(target);
  clickTarget.value = "";
}

watch(
  () => activeTab.value?.url,
  (url) => {
    if (url) address.value = url;
  },
);

onMounted(async () => {
  await Promise.all([store.initialize(), store.bindEvents()]);
  if (typeof ResizeObserver !== "undefined" && viewport.value) {
    resizeObserver = new ResizeObserver(() => void syncBounds());
    resizeObserver.observe(viewport.value);
  }
  await syncBounds();
});

onUnmounted(() => resizeObserver?.disconnect());
</script>

<template>
  <section class="browser-workspace">
    <div class="browser-tab-strip">
      <div
        v-for="tab in tabs"
        :key="tab.id"
        class="browser-tab"
        :class="{ 'is-active': tab.id === activeTabId }"
      >
        <button type="button" class="browser-tab-select" @click="store.select(tab.id)">
          <Globe2 :size="14" />
          <span>{{ tab.title || tab.url }}</span>
          <i v-if="tab.loading" aria-label="正在加载"></i>
        </button>
        <button class="browser-tab-close" type="button" title="关闭标签" aria-label="关闭标签" @click="store.close(tab.id)">
          <X :size="13" />
        </button>
      </div>
      <button class="browser-new-tab" type="button" title="新建网页标签" aria-label="新建网页标签" @click="createTab">
        <Plus :size="16" />
      </button>
    </div>

    <div class="browser-toolbar">
      <div class="browser-navigation">
        <button type="button" title="后退" aria-label="后退" :disabled="!activeTabId" @click="store.history('back')">
          <ArrowLeft :size="17" />
        </button>
        <button type="button" title="前进" aria-label="前进" :disabled="!activeTabId" @click="store.history('forward')">
          <ArrowRight :size="17" />
        </button>
        <button type="button" title="刷新" aria-label="刷新" :disabled="!activeTabId" @click="store.reload">
          <RefreshCw :size="16" />
        </button>
      </div>

      <form class="browser-address" data-action="open-address" @submit.prevent="openAddress">
        <Globe2 :size="15" />
        <input v-model="address" data-field="browser-address" aria-label="网页地址" autocomplete="off" spellcheck="false" />
      </form>

      <div class="browser-actions">
        <button type="button" title="读取当前页" aria-label="读取当前页" :disabled="!activeTabId || loadingAction" @click="store.extract">
          <ScanText :size="17" />
        </button>
        <button type="button" title="向下滚动并读取" aria-label="向下滚动并读取" :disabled="!activeTabId || loadingAction" @click="store.scroll()">
          <ChevronsDown :size="17" />
        </button>
        <button
          type="button"
          data-action="ask-current-page"
          title="询问当前网页"
          aria-label="询问当前网页"
          :disabled="!activeTabId || loadingAction"
          @click="askCurrentPage"
        >
          <MessageSquareText :size="17" />
        </button>
      </div>
    </div>

    <div class="browser-readonly-bar">
      <MousePointerClick :size="15" />
      <input v-model="clickTarget" placeholder="展开更多、查看全部评论等可见文字" @keydown.enter.prevent="clickVisibleTarget" />
      <button type="button" :disabled="!clickTarget.trim() || loadingAction" @click="clickVisibleTarget">点击</button>
      <span v-if="error" class="is-error">{{ error }}</span>
      <span v-else-if="lastSnapshot?.loginChallenge" class="is-error">
        {{ lastSnapshot.challengeReason || "页面需要登录或验证" }}
      </span>
      <span v-else>只读操作，不会执行登录、发布、点赞或提交。</span>
    </div>

    <div ref="viewport" class="browser-viewport">
      <div v-if="tabs.length === 0" class="browser-empty">
        <Globe2 :size="30" />
        <strong>应用内浏览器</strong>
        <span>登录态保存在应用独立的 WebView2 Profile 中。</span>
        <button type="button" @click="createTab">打开 BWiki</button>
      </div>
    </div>
  </section>
</template>
