<script setup lang="ts">
import { computed } from "vue";

import { renderMarkdown } from "@/lib/markdown";
import { openBrowserTab } from "@/api/browser";

const props = defineProps<{ content: string }>();
const html = computed(() => renderMarkdown(props.content));

async function handleClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  const anchor = target?.closest("a[href]") as HTMLAnchorElement | null;
  if (!anchor) return;
  const href = anchor.href;
  if (!/^https?:/i.test(href)) return;
  event.preventDefault();
  try {
    await openBrowserTab(href);
  } catch {
    // The browser command reports the actionable error in the application log.
  }
}
</script>

<template>
  <div class="markdown-content" v-html="html" @click="handleClick"></div>
</template>
