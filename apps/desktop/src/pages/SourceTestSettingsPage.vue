<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Copy, ExternalLink, FlaskConical, LoaderCircle, Play } from "@lucide/vue";
import { openBrowserTab } from "@/api/browser";
import { useSourceTestStore } from "@/stores/sourceTestStore";
import type { SourceTestPlatform } from "@/types/sourceTests";

const store = useSourceTestStore();
const platforms: { id: SourceTestPlatform; label: string; placeholder: string; hint: string }[] = [
  { id: "bilibili", label: "B站", placeholder: "https://www.bilibili.com/video/BV…", hint: "输入具体视频链接，检查当前分 P 的字幕、评论和播放量。" },
  { id: "xiaohongshu", label: "小红书", placeholder: "https://www.xiaohongshu.com/explore/…", hint: "输入完整笔记详情链接，保留分享链接中的参数。检查正文、评论和图片地址。" },
  { id: "zhihu", label: "知乎", placeholder: "https://www.zhihu.com/question/…/answer/…", hint: "输入具体回答或专栏文章链接，检查正文、评论和图片地址。" },
];
const platform = computed(() => platforms.find((item) => item.id === store.selectedPlatform)!);
const current = computed(() => store.cases[store.selectedPlatform]);
const result = computed(() => current.value.result);
const busy = computed(() => Boolean(store.runningPlatform));
const feedback = ref("");
const feedbackError = ref(false);
const failedImages = ref(new Set<string>());
const actionLabels: Record<string, string> = {
  open_page: "打开详情页", scroll_page: "滚动加载", extract_page: "读取正文",
  find_context: "定位页面内容", click_element: "展开内容", extract_comments: "读取评论",
  extract_subtitles: "读取字幕", extract_keyframes: "读取画面", close_page: "关闭测试网页",
};
const statusLabels: Record<string, string> = {
  running: "进行中", returned: "已返回", missing: "未取得", failed: "执行失败",
  complete: "字段齐全", partial: "结果不完整",
};
const artifacts = computed(() => result.value?.report ? [
  { label: "正文", artifact: result.value.report.body },
  { label: "评论", artifact: result.value.report.comments },
  { label: "字幕", artifact: result.value.report.transcript },
].filter((item) => item.artifact) : []);

function json(value: unknown) { return JSON.stringify(value, null, 2); }
function seconds(milliseconds: number) { return `${(milliseconds / 1000).toFixed(1)} 秒`; }

async function start() {
  feedback.value = "";
  await store.start(store.selectedPlatform);
}

async function openPage() {
  try { await openBrowserTab(current.value.url.trim()); }
  catch (error) { current.value.error = `打开网页失败：${String(error)}`; }
}

async function copyResult() {
  if (!result.value) return;
    feedbackError.value = false;
  try {
    await navigator.clipboard.writeText(json(result.value));
    feedback.value = "已复制脱敏测试结果";
  } catch (error) { feedbackError.value = true; feedback.value = `复制失败：${String(error)}`; }
}

watch(result, () => { failedImages.value = new Set(); feedback.value = ""; feedbackError.value = false; });
</script>

<template>
  <section class="source-tests-page" aria-label="采集测试">
    <header class="settings-page-header test-heading">
      <div>
        <span class="eyebrow">链路诊断</span>
        <h2><FlaskConical :size="19" aria-hidden="true" /> 采集测试</h2>
        <p>输入详情链接，检查正式查询链路实际返回了哪些内容。</p>
      </div>
      <button class="secondary-button" type="button" data-action="copy-source-test" :disabled="!result || busy" @click="copyResult">
        <Copy :size="16" aria-hidden="true" /> 复制测试结果
      </button>
    </header>

    <form class="test-input-panel" @submit.prevent="start">
      <fieldset class="test-platforms" :disabled="busy">
        <legend class="sr-only">测试平台</legend>
        <label v-for="item in platforms" :key="item.id" :class="{ 'is-active': store.selectedPlatform === item.id }">
          <input v-model="store.selectedPlatform" type="radio" name="test-platform" :value="item.id" :data-platform="item.id" />
          {{ item.label }}
        </label>
      </fieldset>
      <label class="form-field" for="source-test-url"><span>{{ platform.label }}详情链接</span></label>
      <div class="test-url-row">
        <input id="source-test-url" v-model="current.url" type="url" required :placeholder="platform.placeholder" :disabled="busy" aria-describedby="source-test-url-hint" />
        <button class="primary-button" type="submit" data-action="run-source-test" :disabled="busy || !current.url.trim()">
          <LoaderCircle v-if="busy" class="test-spinner" :size="16" aria-hidden="true" /><Play v-else :size="16" aria-hidden="true" />
          {{ busy ? "测试中" : "开始测试" }}
        </button>
      </div>
      <p id="source-test-url-hint" class="test-help">{{ platform.hint }}</p>
      <div class="test-context">
        <span>复用应用登录态 · 不调用模型 · 不写入聊天历史</span>
        <button class="secondary-button" type="button" :disabled="busy || !current.url.trim()" data-action="open-test-page" @click="openPage">
          <ExternalLink :size="14" aria-hidden="true" /> 打开网页 / 登录
        </button>
      </div>
      <p v-if="current.error" class="test-error" role="alert">{{ current.error }}</p>
    </form>

    <p v-if="feedback" :role="feedbackError ? 'alert' : 'status'" class="test-feedback" :class="{ 'is-error': feedbackError }">{{ feedback }}</p>
    <p v-if="busy" class="test-running" role="status">正在运行正式采集步骤，结束后会关闭测试网页。请等待当前测试完成后再进行其他查询。</p>

    <template v-if="result">
      <div class="test-result-heading">
        <div><strong>{{ result.report?.title || `${platform.label}测试结果` }}</strong><p>{{ new Date(result.startedAt).toLocaleString() }} · {{ seconds(result.elapsedMs) }}</p><p class="test-result-url">{{ result.url }}</p></div>
        <span class="test-badge" :class="`status-${result.status}`">{{ statusLabels[result.status] }}</span>
      </div>
      <div class="test-checks">
        <article v-for="check in result.checks" :key="check.key" class="test-check" :data-check="check.key">
          <div><strong>{{ check.label }}</strong><span class="test-badge" :class="`status-${check.status}`">{{ statusLabels[check.status] }}</span></div>
          <p>{{ check.detail }}</p>
        </article>
      </div>
      <div v-if="result.error || result.warnings.length" class="test-notices" role="status">
        <strong>正式链路返回的提示</strong>
        <p v-if="result.error" class="test-error">{{ result.error }}</p>
        <ul v-if="result.warnings.length"><li v-for="(warning, index) in result.warnings" :key="index">{{ warning }}</li></ul>
      </div>
      <details v-if="Object.keys(result.metadata).length" class="test-details">
        <summary>查看返回的元数据</summary>
        <dl class="test-metadata"><template v-for="(value, key) in result.metadata" :key="key"><dt>{{ key }}</dt><dd>{{ value }}</dd></template></dl>
      </details>
      <details v-for="item in artifacts" :key="item.label" class="test-details">
        <summary>查看{{ item.label }}内容</summary><pre>{{ item.artifact?.content }}</pre>
      </details>
      <section v-if="result.images.length" class="test-image-section" aria-label="提取的图片">
        <h3>图片预览 <span>{{ result.images.length }} 个地址</span></h3>
        <p class="test-help">图片地址已取得不代表预览一定可加载；预览失败可能与资源权限、来源限制或地址过期有关。</p>
        <div class="test-images">
          <figure v-for="(url, index) in result.images" :key="url">
            <div v-if="failedImages.has(url)" class="test-image-failed" role="status">预览加载失败<br /><small>图片地址已提取</small></div>
            <img v-else :src="url" :alt="`提取图片 ${index + 1}`" loading="lazy" referrerpolicy="no-referrer" @error="failedImages.add(url)" />
            <figcaption>{{ url }}</figcaption>
          </figure>
        </div>
      </section>
    </template>

    <section v-if="current.steps.length" class="test-step-section" aria-label="采集步骤">
      <h3>采集步骤 <span>工具返回不代表所需字段已取得</span></h3>
      <ol class="test-steps">
        <li v-for="step in current.steps" :key="step.index">
          <div class="test-step-heading"><strong>{{ step.index + 1 }}. {{ actionLabels[step.action] || step.action }}</strong><span>{{ seconds(step.elapsedMs) }}</span><span class="test-badge" :class="`status-${step.status}`">{{ statusLabels[step.status] }}</span></div>
          <p v-if="step.error" class="test-error">{{ step.error }}</p>
          <details v-if="step.data" class="test-details"><summary>查看该步骤原始输出（已脱敏）</summary><pre>{{ json(step.data) }}</pre></details>
        </li>
      </ol>
    </section>
    <details v-if="result" class="test-details"><summary>测试结果 JSON（已脱敏，长文本按诊断上限截断）</summary><pre>{{ json(result) }}</pre></details>
    <div v-if="!result && !busy && !current.error" class="test-empty"><FlaskConical :size="28" aria-hidden="true" /><strong>从一个真实详情链接开始</strong><p>先在应用网页中完成登录，再测试你想检查的内容。缺失字段和失败原因都会保留在结果中。</p></div>
  </section>
</template>

<style scoped>
.source-tests-page { display: grid; align-content: start; gap: 20px; min-width: 0; min-height: 0; overflow: auto; padding: 26px 28px 40px; scrollbar-gutter: stable; }
.test-heading, .test-result-heading, .test-check > div, .test-context, .test-step-heading { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.test-heading h2 { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.test-heading { padding: 0; border: 0; background: transparent; }
.test-heading p, .test-help, .test-result-heading p, .test-check p, .test-empty p { color: var(--text-muted); font-size: 13px; line-height: 1.65; }
.test-input-panel { display: grid; gap: 10px; padding: 20px; border: 1px solid var(--border); border-radius: 12px; background: var(--surface-raised); }
.test-platforms { display: flex; gap: 6px; margin: 0 0 6px; padding: 0; border: 0; }
.test-platforms label { display: flex; align-items: center; gap: 7px; min-height: 44px; padding: 0 16px; border: 1px solid var(--border); border-radius: 8px; cursor: pointer; background: var(--surface); font-size: 13px; font-weight: 700; }
.test-platforms label.is-active { border-color: var(--accent); background: var(--accent-soft); color: var(--accent-strong); }
.test-platforms input { accent-color: var(--accent); }
.test-platforms:disabled label { opacity: .6; cursor: default; }
.test-url-row { display: flex; align-items: stretch; gap: 10px; }
.test-url-row input { flex: 1; min-width: 0; min-height: 44px; border: 1px solid var(--border-strong); border-radius: 8px; padding: 0 12px; background: var(--surface); color: var(--text); }
.source-tests-page button { display: inline-flex; align-items: center; justify-content: center; gap: 7px; min-height: 40px; white-space: nowrap; }
.test-url-row button { min-height: 44px; }
.source-tests-page button:disabled { opacity: .5; cursor: not-allowed; }
.test-help, .test-context, .test-feedback, .test-running { margin: 0; font-size: 12px; }
.test-context { color: var(--text-muted); flex-wrap: wrap; }
.test-error { color: var(--danger); margin: 0; overflow-wrap: anywhere; line-height: 1.6; font-size: 13px; }
.test-feedback { color: var(--success); }
.test-feedback.is-error { color: var(--danger); }
.test-result-url { overflow-wrap: anywhere; font-size: 11px; }
.test-running { padding: 12px 16px; background: var(--accent-soft); color: var(--accent-strong); border-radius: 8px; line-height: 1.6; }
.test-result-heading strong { display: block; font-size: 15px; overflow-wrap: anywhere; }
.test-result-heading p { margin: 5px 0 0; }
.test-badge { display: inline-flex; flex-shrink: 0; padding: 4px 8px; border-radius: 6px; font-size: 11px; font-weight: 700; background: var(--surface-muted); color: var(--text-muted); }
.status-returned, .status-complete { background: var(--success-soft); color: var(--success); }
.status-missing, .status-partial, .status-running { background: var(--accent-soft); color: var(--accent-strong); }
.status-failed { color: var(--danger); }
.test-checks { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
.test-check { min-width: 0; border: 1px solid var(--border); border-radius: 10px; padding: 15px; background: var(--surface); }
.test-check strong { font-size: 14px; }
.test-check p { margin: 10px 0 0; overflow-wrap: anywhere; }
.test-notices { padding: 15px 18px; background: var(--surface-raised); border-left: 3px solid var(--accent); border-radius: 6px; font-size: 13px; line-height: 1.7; overflow-wrap: anywhere; }
.test-notices ul { margin: 8px 0 0; padding-left: 20px; }
.test-details { min-width: 0; font-size: 12px; }
.test-details summary { cursor: pointer; color: var(--accent-strong); font-weight: 700; line-height: 1.8; }
.test-details pre { max-height: 360px; overflow: auto; background: var(--surface-muted); border: 1px solid var(--border); border-radius: 8px; padding: 14px; margin: 10px 0 0; white-space: pre-wrap; overflow-wrap: anywhere; font-size: 12px; line-height: 1.6; }
.test-metadata { display: grid; grid-template-columns: minmax(90px, 160px) 1fr; gap: 8px 14px; padding: 14px; background: var(--surface-muted); border-radius: 8px; }
.test-metadata dt { color: var(--text-muted); }.test-metadata dd { margin: 0; overflow-wrap: anywhere; }
.test-image-section h3, .test-step-section h3 { margin: 0 0 12px; font-size: 14px; }.test-image-section h3 span, .test-step-section h3 span { margin-left: 6px; font-size: 11px; font-weight: 400; color: var(--text-muted); }
.test-images { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; margin-top: 12px; }
.test-images figure { min-width: 0; margin: 0; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; background: var(--surface-raised); }
.test-images img, .test-image-failed { display: block; width: 100%; height: 180px; object-fit: contain; background: var(--surface-muted); }
.test-image-failed { display: grid; place-content: center; text-align: center; color: var(--danger); font-size: 13px; }.test-image-failed small { color: var(--text-muted); }
.test-images figcaption { padding: 10px; font-size: 10px; color: var(--text-muted); overflow-wrap: anywhere; }
.test-steps { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }.test-steps > li { border: 1px solid var(--border); border-radius: 8px; padding: 13px 15px; }
.test-step-heading { font-size: 12px; }.test-step-heading strong { flex: 1; }.test-step-heading > span:not(.test-badge) { color: var(--text-muted); font-variant-numeric: tabular-nums; }.test-steps .test-details { margin-top: 9px; }
.test-empty { display: grid; justify-items: center; gap: 12px; text-align: center; padding: 35px 20px; color: var(--text-muted); }.test-empty strong { color: var(--text); font-size: 14px; }.test-empty p { max-width: 440px; margin: 0; }
.test-spinner { animation: test-spin 1s linear infinite; }@keyframes test-spin { to { transform: rotate(360deg); } }
@media (max-width: 1050px) { .test-checks { grid-template-columns: 1fr; }.test-images { grid-template-columns: repeat(2, minmax(0, 1fr)); }.test-heading { flex-wrap: wrap; } }
@media (prefers-reduced-motion: reduce) { .test-spinner { animation: none; } }
</style>
