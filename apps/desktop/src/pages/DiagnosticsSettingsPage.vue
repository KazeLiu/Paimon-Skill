<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { Clipboard, Copy, RefreshCw, Trash2 } from "@lucide/vue";

import { clearLogs, exportLogs, getDiagnosticsSettings, listLogs, setDiagnosticsSettings } from "@/api/diagnostics";
import type { LogEntry } from "@/types/diagnostics";

const entries = ref<LogEntry[]>([]);
const loading = ref(true);
const busy = ref(false);
const feedback = ref("");
const errorMessage = ref("");
const selectedIds = ref<string[]>([]);
const verbose = ref(false);
const savingSettings = ref(false);
const allSelected = computed(() => entries.value.length > 0 && selectedIds.value.length === entries.value.length);

function selectAll() {
  selectedIds.value = entries.value.map((entry) => entry.id);
}

function clearSelection() {
  selectedIds.value = [];
}

async function refresh() {
  loading.value = true;
  errorMessage.value = "";
  try {
    const [logs, settings] = await Promise.all([listLogs(), getDiagnosticsSettings()]);
    entries.value = logs;
    verbose.value = settings.verbose;
    selectedIds.value = entries.value.map((entry) => entry.id);
  } catch (error) {
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
  }
}

async function toggleVerbose() {
  savingSettings.value = true;
  errorMessage.value = "";
  try {
    const settings = await setDiagnosticsSettings({ verbose: verbose.value });
    verbose.value = settings.verbose;
    feedback.value = settings.verbose ? "已开启详细日志模式" : "已关闭详细日志模式";
  } catch (error) {
    verbose.value = !verbose.value;
    errorMessage.value = `保存日志设置失败：${String(error)}`;
  } finally {
    savingSettings.value = false;
  }
}

async function copyLogs() {
  busy.value = true;
  feedback.value = "";
  try {
    if (!selectedIds.value.length) {
      feedback.value = "请至少选择一条日志";
      return;
    }
    await navigator.clipboard.writeText(await exportLogs(selectedIds.value));
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
        <button class="primary-button" type="button" data-action="copy-logs" :disabled="busy || !selectedIds.length" @click="copyLogs">
          <Copy :size="16" /> {{ busy ? "处理中" : `复制所选日志（${selectedIds.length}）` }}
        </button>
      </div>
    </header>
    <div class="diagnostics-toolbar">
      <span>{{ entries.length }} 条日志，最多保留 200 条</span>
      <label class="diagnostics-verbose-toggle">
        <input v-model="verbose" type="checkbox" :disabled="savingSettings" data-action="toggle-verbose" @change="toggleVerbose" />
        <span>详细日志模式</span>
        <small>记录工具参数、结果、研究步骤和模型请求摘要（自动脱敏）</small>
      </label>
      <div class="diagnostics-selection-actions">
        <button class="text-button" type="button" data-action="select-all" :disabled="!entries.length || allSelected" @click="selectAll">全选</button>
        <button class="text-button" type="button" data-action="clear-selection" :disabled="!selectedIds.length" @click="clearSelection">取消全选</button>
        <button class="text-button danger-button" type="button" data-action="clear-logs" @click="clearAll">
          <Trash2 :size="15" /> 清空日志
        </button>
      </div>
    </div>
    <div v-if="feedback" class="diagnostics-feedback is-success" role="status"><Clipboard :size="15" /> {{ feedback }}</div>
    <div v-if="errorMessage" class="diagnostics-feedback is-error" role="alert">{{ errorMessage }}</div>
    <div v-if="loading" class="settings-loading">正在读取诊断日志...</div>
    <div v-else-if="entries.length === 0" class="diagnostics-empty">暂无诊断日志</div>
    <div v-else class="diagnostics-list">
      <article v-for="entry in [...entries].reverse()" :key="entry.id" class="diagnostics-entry" :class="{ 'is-selected': selectedIds.includes(entry.id) }">
        <div class="diagnostics-entry-heading">
          <label class="diagnostics-select">
            <input :data-log-id="entry.id" v-model="selectedIds" type="checkbox" :value="entry.id" />
            <span class="sr-only">选择日志 {{ entry.id }}</span>
          </label>
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
