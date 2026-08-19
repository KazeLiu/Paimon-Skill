<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { Check, CircleAlert, FlaskConical, Plus, Save, Trash2 } from "@lucide/vue";

import { deleteProvider, listProviders, saveProvider, testProvider } from "@/api/providers";
import type { CapabilityReport, ProviderConfig, ProviderView } from "@/types/providers";

const providers = ref<ProviderView[]>([]);
const selectedId = ref<string | null>(null);
const apiKey = ref("");
const loading = ref(true);
const saving = ref(false);
const testing = ref(false);
const feedback = ref("");
const errorMessage = ref("");
const report = ref<CapabilityReport | null>(null);

const form = reactive<ProviderConfig>(blankProvider());
const selectedProvider = computed(() => providers.value.find((provider) => provider.config.id === selectedId.value));
const capabilityRows = computed(() => {
  if (!report.value) return [];
  return [
    { label: "基础请求", status: report.value.basicRequest },
    { label: "流式输出", status: report.value.streaming },
    { label: "工具调用", status: report.value.toolCalling },
  ];
});

function blankProvider(): ProviderConfig {
  return {
    id: `provider-${Date.now()}`,
    name: "",
    baseUrl: "https://api.deepseek.com",
    model: "deepseek-chat",
    secretRef: "",
    timeoutMs: 30000,
    temperature: 0.3,
    maxTokens: 2048,
    streaming: true,
    toolCalling: true,
    enabled: true,
  };
}

function selectProvider(provider: ProviderView) {
  Object.assign(form, provider.config);
  selectedId.value = provider.config.id;
  apiKey.value = "";
  feedback.value = "";
  errorMessage.value = "";
  report.value = null;
}

function newProvider() {
  Object.assign(form, blankProvider());
  selectedId.value = null;
  apiKey.value = "";
  feedback.value = "";
  errorMessage.value = "";
  report.value = null;
}

function formInput() {
  const generatedId = form.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-") || form.id;
  const providerId = selectedId.value ?? generatedId;
  return {
    config: {
      ...form,
      id: providerId,
      secretRef: form.secretRef || `provider:${providerId}`,
    },
    apiKey: apiKey.value || undefined,
  };
}

async function load() {
  loading.value = true;
  try {
    providers.value = await listProviders();
    if (providers.value[0]) selectProvider(providers.value[0]);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function save() {
  saving.value = true;
  feedback.value = "";
  errorMessage.value = "";
  try {
    const saved = await saveProvider(formInput());
    const index = providers.value.findIndex((provider) => provider.config.id === saved.config.id);
    if (index >= 0) providers.value[index] = saved;
    else providers.value.push(saved);
    selectProvider(saved);
    feedback.value = "已保存模型配置";
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!selectedId.value) return;
  saving.value = true;
  try {
    await deleteProvider(selectedId.value);
    providers.value = providers.value.filter((provider) => provider.config.id !== selectedId.value);
    newProvider();
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    saving.value = false;
  }
}

async function test() {
  testing.value = true;
  report.value = null;
  errorMessage.value = "";
  try {
    report.value = await testProvider(formInput());
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    testing.value = false;
  }
}

onMounted(load);
</script>

<template>
  <section class="model-settings-page">
    <header class="settings-page-header model-settings-header">
      <div>
        <span class="eyebrow">模型设置</span>
        <h2>连接你的模型</h2>
        <p>支持 DeepSeek、中转站、Ollama、LM Studio 和其他 OpenAI-Compatible 服务。</p>
      </div>
      <button class="secondary-button" type="button" data-action="new-provider" @click="newProvider">
        <Plus :size="16" /> 新增模型
      </button>
    </header>

    <div v-if="loading" class="settings-loading" role="status">正在读取模型配置...</div>
    <div v-else class="model-settings-body">
      <nav class="provider-list" aria-label="模型配置列表">
        <button
          v-for="provider in providers"
          :key="provider.config.id"
          class="provider-list-item"
          :class="{ 'is-selected': provider.config.id === selectedId }"
          type="button"
          @click="selectProvider(provider)"
        >
          <span class="provider-status" :class="{ 'is-ready': provider.hasApiKey }"></span>
          <span>
            <strong>{{ provider.config.name }}</strong>
            <small>{{ provider.config.model }}</small>
          </span>
        </button>
        <div v-if="providers.length === 0" class="provider-list-empty">
          <span>还没有模型配置</span>
          <button type="button" @click="newProvider">创建第一项</button>
        </div>
      </nav>

      <form class="provider-editor" @submit.prevent="save">
        <div class="editor-heading">
          <div>
            <span class="eyebrow">OpenAI-Compatible</span>
            <h3>{{ form.name || "未命名模型" }}</h3>
          </div>
          <button v-if="selectedProvider" class="icon-text-button danger-button" type="button" @click="remove">
            <Trash2 :size="15" /> 删除
          </button>
        </div>

        <div class="form-grid">
          <label class="form-field">
            <span>显示名称</span>
            <input data-field="provider-name" v-model="form.name" required placeholder="例如：DeepSeek 官方" />
          </label>
          <label class="form-field">
            <span>模型 ID</span>
            <input data-field="model-id" v-model="form.model" required placeholder="例如：deepseek-chat" />
          </label>
          <label class="form-field form-field-wide">
            <span>Base URL</span>
            <input data-field="base-url" v-model="form.baseUrl" required placeholder="https://api.example.com/v1" />
          </label>
          <label class="form-field form-field-wide">
            <span>API Key <small>只保存到系统凭据库</small></span>
            <input data-field="api-key" v-model="apiKey" type="password" autocomplete="new-password" placeholder="留空则使用已保存的 Key" />
            <em v-if="selectedProvider?.hasApiKey">已安全保存 API Key</em>
          </label>
          <label class="form-field">
            <span>温度</span>
            <input v-model.number="form.temperature" type="number" min="0" max="2" step="0.1" />
          </label>
          <label class="form-field">
            <span>最大输出 Token</span>
            <input v-model.number="form.maxTokens" type="number" min="1" step="128" />
          </label>
        </div>

        <div class="capability-toggles">
          <label><input v-model="form.streaming" type="checkbox" /> 启用流式输出</label>
          <label><input v-model="form.toolCalling" type="checkbox" /> 启用工具调用</label>
        </div>

        <div v-if="report" class="capability-report" aria-live="polite">
          <div v-for="entry in capabilityRows" :key="entry.label" class="capability-row">
            <span :class="entry.status.supported ? 'is-success' : 'is-error'">
              <Check v-if="entry.status.supported" :size="15" /><CircleAlert v-else :size="15" />
              {{ entry.label }}
            </span>
            <small>{{ entry.status.message }}</small>
          </div>
        </div>

        <div class="provider-editor-footer">
          <span v-if="feedback" class="is-success"><Check :size="16" /> {{ feedback }}</span>
          <span v-else-if="errorMessage" class="is-error"><CircleAlert :size="16" /> {{ errorMessage }}</span>
          <span v-else>API Key 不会显示在配置文件或日志中。</span>
          <div class="settings-actions">
            <button class="secondary-button" type="button" data-action="test-provider" :disabled="testing || saving" @click="test">
              <FlaskConical :size="16" /> {{ testing ? "测试中" : "测试连接" }}
            </button>
            <button class="primary-button" type="button" data-action="save-provider" :disabled="saving || testing" @click="save">
              <Save :size="16" /> {{ saving ? "保存中" : "保存配置" }}
            </button>
          </div>
        </div>
      </form>
    </div>
  </section>
</template>
