import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import ModelSettingsPage from "./ModelSettingsPage.vue";
import { listProviders, saveProvider, testProvider } from "@/api/providers";

vi.mock("@/api/providers", () => ({
  listProviders: vi.fn(),
  saveProvider: vi.fn(),
  deleteProvider: vi.fn(),
  testProvider: vi.fn(),
}));

describe("ModelSettingsPage", () => {
  beforeEach(() => {
    vi.mocked(listProviders).mockResolvedValue([]);
    vi.mocked(saveProvider).mockImplementation(async (input) => ({
      config: input.config,
      hasApiKey: Boolean(input.apiKey),
    }));
    vi.mocked(testProvider).mockResolvedValue({
      basicRequest: { supported: true, message: "基础请求成功" },
      streaming: { supported: true, message: "流式输出成功" },
      toolCalling: { supported: false, message: "未检测到工具调用" },
    });
  });

  it("creates, saves, and tests an OpenAI-compatible provider", async () => {
    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    await wrapper.get('[data-field="provider-name"]').setValue("DeepSeek");
    await wrapper.get('[data-field="base-url"]').setValue("https://api.deepseek.com");
    await wrapper.get('[data-field="model-id"]').setValue("deepseek-chat");
    await wrapper.get('[data-field="api-key"]').setValue("secret-key");
    await wrapper.get('[data-action="save-provider"]').trigger("click");
    await flushPromises();

    expect(saveProvider).toHaveBeenCalledWith(
      expect.objectContaining({
        config: expect.objectContaining({
          name: "DeepSeek",
          baseUrl: "https://api.deepseek.com",
          model: "deepseek-chat",
        }),
        apiKey: "secret-key",
      }),
    );
    expect(wrapper.text()).toContain("已保存模型配置");

    await wrapper.get('[data-action="test-provider"]').trigger("click");
    await flushPromises();

    expect(wrapper.text()).toContain("基础请求成功");
    expect(wrapper.text()).toContain("流式输出成功");
    expect(wrapper.text()).toContain("未检测到工具调用");
  });

  it("shows saved credential state without exposing the key", async () => {
    vi.mocked(listProviders).mockResolvedValue([
      {
        config: {
          id: "saved",
          name: "已保存模型",
          baseUrl: "http://127.0.0.1:11434/v1",
          model: "qwen3",
          secretRef: "provider:saved",
          timeoutMs: 30000,
          temperature: 0.3,
          maxTokens: 2048,
          streaming: true,
          toolCalling: true,
          enabled: true,
        },
        hasApiKey: true,
      },
    ]);

    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    expect(wrapper.text()).toContain("已安全保存 API Key");
    expect((wrapper.get('[data-field="api-key"]').element as HTMLInputElement).value).toBe("");
  });
});
