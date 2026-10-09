<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { Check, CircleAlert, FlaskConical, Plus, RefreshCw, Save, Star, Trash2 } from "@lucide/vue";

import { deleteProvider, listProviderModels, listProviders, saveProvider, testProvider } from "@/api/providers";
import type { CapabilityReport, ProviderConfig, ProviderView } from "@/types/providers";

const DEFAULT_TIMEOUT_MS = 180_000;

const providers = ref<ProviderView[]>([]);
const selectedId = ref<string | null>(null);
const apiKey = ref("");
const loading = ref(true);
const saving = ref(false);
const testing = ref(false);
const loadingModels = ref(false);
const models = ref<string[]>([]);
const feedback = ref("");
const errorMessage = ref("");
const report = ref<CapabilityReport | null>(null);

const form = reactive<ProviderConfig>(blankProvider());
const emit = defineEmits<{
  "provider-updated": [provider: ProviderView];
  "provider-deleted": [id: string];
}>();
const selectedProvider = computed(() => providers.value.find((provider) => provider.config.id === selectedId.value));
const capabilityRows = computed(() => report.value
  ? [{ label: "Responses 请求", status: report.value.basicRequest }]
  : []);

function blankProvider(): ProviderConfig {
  return {
    id: `provider-${Date.now()}`,
    name: "",
    baseUrl: "",
    model: "",
    secretRef: "",
    timeoutMs: DEFAULT_TIMEOUT_MS,
    streaming: true,
    toolCalling: true,
    enabled: true,
    isDefault: false,
  };
}

function selectProvider(provider: ProviderView) {
  Object.assign(form, provider.config);
  selectedId.value = provider.config.id;
  apiKey.value = "";
  models.value = provider.config.model ? [provider.config.model] : [];
  feedback.value = "";
  errorMessage.value = "";
  report.value = null;
}

function newProvider() {
  Object.assign(form, blankProvider());
  selectedId.value = null;
  apiKey.value = "";
  models.value = [];
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
    emit("provider-updated", saved);
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
    emit("provider-deleted", selectedId.value);
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

async function fetchModels() {
  loadingModels.value = true;
  feedback.value = "";
  errorMessage.value = "";
  report.value = null;
  try {
    models.value = await listProviderModels(formInput());
    if (!models.value.includes(form.model)) form.model = models.value[0] ?? "";
    feedback.value = `已获取 ${models.value.length} 个模型`;
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : String(error);
  } finally {
    loadingModels.value = false;
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
        <p>统一使用 Responses API；填写地址和 Key 后获取模型，再选择并测试连接。</p>
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
            <span class="eyebrow">Responses API</span>
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
          <label class="form-field form-field-wide">
            <span>Base URL</span>
            <input data-field="base-url" v-model="form.baseUrl" required placeholder="https://api.example.com/v1" />
          </label>
          <label class="form-field">
            <span>请求超时</span>
            <span class="input-with-suffix">
              <input data-field="timeout-ms" v-model.number="form.timeoutMs" type="number" min="1" max="600" step="1" required />
              <em>秒</em>
            </span>
            <small>中转站排队较久时可提高，最多 600 秒</small>
          </label>
          <label class="form-field form-field-wide">
            <span>API Key <small>只保存到系统凭据库</small></span>
            <input data-field="api-key" v-model="apiKey" type="password" autocomplete="new-password" placeholder="留空则使用已保存的 Key" />
            <em v-if="selectedProvider?.hasApiKey">已安全保存 API Key</em>
          </label>
          <label class="form-field form-field-wide model-picker-field">
            <span>模型</span>
            <span class="model-picker-row">
              <select data-field="model-id" v-model="form.model" required :disabled="models.length === 0">
                <option value="" disabled>{{ loadingModels ? "正在获取模型..." : "请先获取模型" }}</option>
                <option v-for="model in models" :key="model" :value="model">{{ model }}</option>
              </select>
              <button class="secondary-button" type="button" data-action="fetch-models" :disabled="loadingModels || testing || saving" @click="fetchModels">
                <RefreshCw :size="16" :class="{ 'is-spinning': loadingModels }" />
                {{ loadingModels ? "获取中" : "获取模型" }}
              </button>
            </span>
          </label>
          <label class="default-provider-toggle" data-role="default-provider-toggle">
            <input data-field="default-provider" v-model="form.isDefault" type="checkbox" />
            <Star :size="16" aria-hidden="true" />
            <span><strong>设为默认模型</strong><small>新建对话会自动使用此模型</small></span>
          </label>
        </div>

        <div v-if="report" class="capability-report" aria-live="polite">
          <div v-for="entry in capabilityRows" :key="entry.label" class="capability-row">
            <span :class="entry.status.supported ? 'is-success' : 'is-error'">
              <Check v-if="entry.status.supported" :size="15" /><CircleAlert v-else :size="15" />
              {{ entry.label }}
            </span>
            <small>{{ entry.status.message }}</small>
            <details v-if="entry.status.diagnostic" class="capability-diagnostic" data-role="capability-diagnostic">
              <summary>查看详细日志</summary>
              <dl>
                <dt>阶段</dt><dd>{{ entry.status.diagnostic.stage }}</dd>
                <dt>接口</dt><dd>{{ entry.status.diagnostic.endpoint }}</dd>
                <dt>HTTP</dt><dd>{{ entry.status.diagnostic.httpStatus == null ? "未收到响应" : `HTTP ${entry.status.diagnostic.httpStatus}` }}</dd>
                <dt>Content-Type</dt><dd>{{ entry.status.diagnostic.contentType || "未提供" }}</dd>
              </dl>
              <div v-if="entry.status.diagnostic.responseHeaders.length" class="diagnostic-block">
                <strong>响应头（安全字段）</strong>
                <pre>{{ entry.status.diagnostic.responseHeaders.join("\n") }}</pre>
              </div>
              <div v-if="entry.status.diagnostic.parseError" class="diagnostic-block">
                <strong>解析错误</strong>
                <pre>{{ entry.status.diagnostic.parseError }}</pre>
              </div>
              <div v-if="entry.status.diagnostic.responseBody" class="diagnostic-block">
                <strong>响应正文{{ entry.status.diagnostic.truncated ? "（已截断）" : "" }}</strong>
                <pre>{{ entry.status.diagnostic.responseBody }}</pre>
              </div>
              <div v-if="entry.status.diagnostic.requestBody" class="diagnostic-block">
                <strong>请求正文（已脱敏）</strong>
                <pre>{{ entry.status.diagnostic.requestBody }}</pre>
              </div>
            </details>
          </div>
        </div>

        <div class="provider-editor-footer">
          <span v-if="feedback" class="is-success"><Check :size="16" /> {{ feedback }}</span>
          <span v-else-if="errorMessage" class="is-error"><CircleAlert :size="16" /> {{ errorMessage }}</span>
          <span v-else>API Key 不会显示在配置文件或日志中。</span>
          <div class="settings-actions">
            <button class="secondary-button" type="button" data-action="test-provider" :disabled="testing || saving || loadingModels || !form.model" @click="test">
              <FlaskConical :size="16" /> {{ testing ? "测试中" : "测试连接" }}
            </button>
            <button class="primary-button" type="button" data-action="save-provider" :disabled="saving || testing || loadingModels || !form.model" @click="save">
              <Save :size="16" /> {{ saving ? "保存中" : "保存配置" }}
            </button>
          </div>
        </div>
      </form>
    </div>
  </section>
</template>
