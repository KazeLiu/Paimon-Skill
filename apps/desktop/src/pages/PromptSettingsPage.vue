<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { AlertCircle, Check, RotateCcw, Save } from "@lucide/vue";

import PromptResourceEditor from "@/components/PromptResourceEditor.vue";
import {
  getPromptSettings,
  listPromptResources,
  resetPromptSettings,
  savePromptSettings,
} from "@/api/prompts";
import type { PromptFragment, PromptSettings } from "@/types/prompts";

const resources = ref<PromptFragment[]>([]);
const settings = reactive<PromptSettings>({
  overrides: {},
  disabledResourceIds: [],
  customSuffix: "",
  updatedAt: null,
});
const draftContents = reactive<Record<string, string>>({});
const enabledResources = reactive<Record<string, boolean>>({});
const loading = ref(true);
const saving = ref(false);
const status = ref<"idle" | "saved" | "error">("idle");
const errorMessage = ref("");

const enabledCount = computed(
  () => resources.value.filter((resource) => enabledResources[resource.id]).length,
);

function messageFrom(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function applySettings(next: PromptSettings) {
  settings.overrides = { ...next.overrides };
  settings.disabledResourceIds = [...next.disabledResourceIds];
  settings.customSuffix = next.customSuffix;
  settings.updatedAt = next.updatedAt;

  for (const resource of resources.value) {
    draftContents[resource.id] = next.overrides[resource.id] ?? resource.content;
    enabledResources[resource.id] = !next.disabledResourceIds.includes(resource.id);
  }
}

async function load() {
  loading.value = true;
  errorMessage.value = "";
  try {
    const [loadedResources, loadedSettings] = await Promise.all([
      listPromptResources(),
      getPromptSettings(),
    ]);
    resources.value = [...loadedResources].sort((left, right) => left.order - right.order);
    applySettings(loadedSettings);
  } catch (error) {
    errorMessage.value = messageFrom(error);
    status.value = "error";
  } finally {
    loading.value = false;
  }
}

async function save() {
  saving.value = true;
  status.value = "idle";
  errorMessage.value = "";

  const overrides = Object.fromEntries(
    resources.value
      .filter((resource) => draftContents[resource.id] !== resource.content)
      .map((resource) => [resource.id, draftContents[resource.id]]),
  );
  const next: PromptSettings = {
    overrides,
    disabledResourceIds: resources.value
      .filter((resource) => !enabledResources[resource.id])
      .map((resource) => resource.id),
    customSuffix: settings.customSuffix,
    updatedAt: new Date().toISOString(),
  };

  try {
    await savePromptSettings(next);
    Object.assign(settings, next);
    status.value = "saved";
  } catch (error) {
    errorMessage.value = messageFrom(error);
    status.value = "error";
  } finally {
    saving.value = false;
  }
}

async function resetAll() {
  saving.value = true;
  status.value = "idle";
  errorMessage.value = "";
  try {
    const defaults = await resetPromptSettings();
    applySettings(defaults);
    status.value = "saved";
  } catch (error) {
    errorMessage.value = messageFrom(error);
    status.value = "error";
  } finally {
    saving.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="prompt-settings-page">
    <header class="settings-page-header">
      <div>
        <span class="eyebrow">应用设置</span>
        <h2>系统提示词与规则</h2>
        <p>这些内容会在每次模型请求前组合。所有文本都可以修改，原神规则也可以单独停用。</p>
      </div>
      <div class="settings-summary" aria-label="启用资源数量">
        <strong>{{ enabledCount }}</strong>
        <span>项规则启用</span>
      </div>
    </header>

    <div v-if="loading" class="settings-loading" role="status">正在读取提示词配置...</div>

    <div v-else class="settings-scroll-area">
      <PromptResourceEditor
        v-for="resource in resources"
        :key="resource.id"
        :resource="resource"
        :enabled="enabledResources[resource.id]"
        :model-value="draftContents[resource.id]"
        @update:enabled="enabledResources[resource.id] = $event"
        @update:model-value="draftContents[resource.id] = $event"
      />

      <article class="prompt-resource custom-suffix-resource">
        <header class="prompt-resource__header">
          <div>
            <strong>自定义补充规则</strong>
            <small>追加在所有已启用规则之后，适合存放个人回答偏好。</small>
          </div>
        </header>
        <textarea v-model="settings.customSuffix" rows="5" aria-label="自定义补充规则"></textarea>
      </article>
    </div>

    <footer class="settings-action-bar">
      <div class="save-feedback" aria-live="polite">
        <span v-if="status === 'saved'" class="is-success"><Check :size="16" /> 已保存</span>
        <span v-else-if="status === 'error'" class="is-error">
          <AlertCircle :size="16" /> {{ errorMessage }}
        </span>
        <span v-else>修改只会保存在本机。</span>
      </div>
      <div class="settings-actions">
        <button
          class="secondary-button"
          type="button"
          data-action="reset-prompts"
          :disabled="saving"
          @click="resetAll"
        >
          <RotateCcw :size="16" />
          恢复默认
        </button>
        <button
          class="primary-button"
          type="button"
          data-action="save-prompts"
          :disabled="saving"
          @click="save"
        >
          <Save :size="16" />
          {{ saving ? "保存中" : "保存设置" }}
        </button>
      </div>
    </footer>
  </section>
</template>
