<script setup lang="ts">
import { onMounted, ref } from "vue";
import { Clipboard, Copy, RefreshCw, Trash2 } from "@lucide/vue";

import { clearLogs, exportLogs, listLogs } from "@/api/diagnostics";
import type { LogEntry } from "@/types/diagnostics";

const entries = ref<LogEntry[]>([]);
const loading = ref(true);
const busy = ref(false);
const feedback = ref("");
const errorMessage = ref("");

async function refresh() {
  loading.value = true;
  errorMessage.value = "";
  try {
    entries.value = await listLogs();
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function copyLogs() {
  busy.value = true;
  feedback.value = "";
  try {
    await navigator.clipboard.writeText(await exportLogs());
    feedback.value = "已复制，可直接粘贴给 AI 阅读";
  } catch (error) {
    errorMessage.value = `复制失败：${String(error)}`;
  } finally {
    busy.value = false;
  }
}

async function clearAll() {
  if (!window.confirm("确定清空全部诊断日志吗？")) return;
  await clearLogs();
  feedback.value = "日志已清空";
  await refresh();
}

function detailText(details: Record<string, unknown> | null) {
  return details ? JSON.stringify(details, null, 2) : "";
}

onMounted(refresh);
</script>

<template>
  <section class="diagnostics-settings-page">
    <header class="settings-page-header">
      <div>
        <span class="eyebrow">故障排查</span>
        <h2>诊断日志</h2>
        <p>记录模型连接和应用错误，内容已自动脱敏。</p>
      </div>
      <div class="settings-actions">
        <button class="secondary-button" type="button" data-action="refresh-logs" @click="refresh">
          <RefreshCw :size="16" /> 刷新
        </button>
        <button class="primary-button" type="button" data-action="copy-logs" :disabled="busy" @click="copyLogs">
          <Copy :size="16" /> {{ busy ? "处理中" : "一键复制给 AI" }}
        </button>
      </div>
    </header>
    <div class="diagnostics-toolbar">
      <span>{{ entries.length }} 条日志，最多保留 200 条</span>
      <button class="text-button danger-button" type="button" data-action="clear-logs" @click="clearAll">
        <Trash2 :size="15" /> 清空日志
      </button>
    </div>
    <div v-if="feedback" class="diagnostics-feedback is-success" role="status"><Clipboard :size="15" /> {{ feedback }}</div>
    <div v-if="errorMessage" class="diagnostics-feedback is-error" role="alert">{{ errorMessage }}</div>
    <div v-if="loading" class="settings-loading">正在读取诊断日志...</div>
    <div v-else-if="entries.length === 0" class="diagnostics-empty">暂无诊断日志</div>
    <div v-else class="diagnostics-list">
      <article v-for="entry in [...entries].reverse()" :key="entry.id" class="diagnostics-entry">
        <div class="diagnostics-entry-heading">
          <span :class="entry.level === 'error' ? 'is-error' : 'is-success'">{{ entry.level }}</span>
          <strong>{{ entry.category }}</strong>
          <time>{{ new Date(entry.createdAt).toLocaleString() }}</time>
        </div>
        <p>{{ entry.message }}</p>
        <details v-if="entry.details">
          <summary>查看脱敏详情</summary>
          <pre>{{ detailText(entry.details) }}</pre>
        </details>
      </article>
    </div>
  </section>
</template>
