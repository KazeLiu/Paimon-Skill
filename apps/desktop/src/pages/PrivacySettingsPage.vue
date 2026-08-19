<script setup lang="ts">
import { ref } from "vue";
import { CircleAlert, Database, ShieldCheck, Trash2 } from "@lucide/vue";

import { clearBrowserProfile } from "@/api/browser";

const confirming = ref(false);
const clearing = ref(false);
const feedback = ref("");
const errorMessage = ref("");

function requestClear() {
  feedback.value = "";
  errorMessage.value = "";
  confirming.value = true;
}

function cancelClear() {
  confirming.value = false;
}

async function confirmClear() {
  clearing.value = true;
  feedback.value = "";
  errorMessage.value = "";
  try {
    await clearBrowserProfile();
    confirming.value = false;
    feedback.value = "浏览器登录态已清理";
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    clearing.value = false;
  }
}
</script>

<template>
  <section class="privacy-settings-page">
    <header class="settings-page-header">
      <div>
        <span class="eyebrow">隐私与数据</span>
        <h2>控制本地数据</h2>
        <p>模型配置保存在应用配置目录；API Key 只写入系统凭据库，不会进入 SQLite 对话数据库。</p>
      </div>
      <ShieldCheck :size="28" aria-hidden="true" />
    </header>

    <div class="privacy-settings-list">
      <article class="privacy-setting-row">
        <div class="privacy-setting-icon" aria-hidden="true"><Database :size="18" /></div>
        <div>
          <h3>应用内网页登录态</h3>
          <p>网页标签使用独立、持久的 WebView2 Profile。清理后，应用内网页需要重新登录；不会影响系统浏览器。</p>
          <small>数据目录：应用本地数据目录/browser-profiles/default</small>
        </div>
        <button
          v-if="!confirming"
          class="secondary-button danger-button"
          type="button"
          data-action="request-clear-browser-profile"
          @click="requestClear"
        >
          <Trash2 :size="16" /> 清理登录态
        </button>
        <div v-else class="privacy-confirmation" role="alertdialog" aria-label="确认清理登录态">
          <span>将退出应用内网页的登录状态，确定继续？</span>
          <div class="settings-actions">
            <button class="secondary-button" type="button" data-action="cancel-clear-browser-profile" @click="cancelClear">
              取消
            </button>
            <button class="danger-button" type="button" data-action="confirm-clear-browser-profile" :disabled="clearing" @click="confirmClear">
              {{ clearing ? "清理中" : "确认清理" }}
            </button>
          </div>
        </div>
      </article>
    </div>

    <p v-if="feedback" class="is-success" aria-live="polite"><ShieldCheck :size="16" /> {{ feedback }}</p>
    <p v-if="errorMessage" class="is-error" aria-live="assertive"><CircleAlert :size="16" /> {{ errorMessage }}</p>
  </section>
</template>
