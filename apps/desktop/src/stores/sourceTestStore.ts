import { reactive, ref } from "vue";
import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { runSourceTest } from "@/api/sourceTests";
import type { SourceTestPlatform, SourceTestProgress, SourceTestResult, SourceTestStep } from "@/types/sourceTests";

interface TestCase {
  url: string;
  result: SourceTestResult | null;
  steps: SourceTestStep[];
  error: string;
}

export const useSourceTestStore = defineStore("sourceTests", () => {
  const selectedPlatform = ref<SourceTestPlatform>("bilibili");
  const runningPlatform = ref<SourceTestPlatform | null>(null);
  const cases = reactive<Record<SourceTestPlatform, TestCase>>({
    bilibili: { url: "", result: null, steps: [], error: "" },
    xiaohongshu: { url: "", result: null, steps: [], error: "" },
    zhihu: { url: "", result: null, steps: [], error: "" },
  });

  async function start(platform: SourceTestPlatform) {
    if (runningPlatform.value) return;
    const current = cases[platform];
    const url = current.url.trim();
    current.error = "";
    try {
      const parsed = new URL(url);
      if (!["https:", "http:"].includes(parsed.protocol) || parsed.username || parsed.password) throw new Error("invalid");
    } catch {
      current.error = "请填写完整的 HTTP/HTTPS 详情链接，不要包含账号密码。";
      return;
    }
    runningPlatform.value = platform;
    current.result = null;
    current.steps = [];
    let unlisten: UnlistenFn | undefined;
    try {
      const testId = crypto.randomUUID();
      unlisten = await listen<SourceTestProgress>("source-test-progress", ({ payload }) => {
        if (payload.testId !== testId) return;
        const index = current.steps.findIndex((step) => step.index === payload.step.index);
        if (index < 0) current.steps.push(payload.step);
        else current.steps[index] = payload.step;
      }).catch(() => undefined);
      current.result = await runSourceTest({ testId, platform, url });
      current.steps = current.result.steps;
    } catch (error) {
      current.error = String(error);
    } finally {
      unlisten?.();
      runningPlatform.value = null;
    }
  }

  return { cases, selectedPlatform, runningPlatform, start };
});
