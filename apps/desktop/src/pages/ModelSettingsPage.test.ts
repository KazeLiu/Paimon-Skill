import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import ModelSettingsPage from "./ModelSettingsPage.vue";
import {
  listProviderModels,
  listProviders,
  saveProvider,
  testProvider,
} from "@/api/providers";

vi.mock("@/api/providers", () => ({
  listProviders: vi.fn(),
  listProviderModels: vi.fn(),
  saveProvider: vi.fn(),
  deleteProvider: vi.fn(),
  testProvider: vi.fn(),
}));

describe("ModelSettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listProviders).mockResolvedValue([]);
    vi.mocked(listProviderModels).mockResolvedValue(["gpt-5.1", "gpt-5.2"]);
    vi.mocked(saveProvider).mockImplementation(async (input) => ({
      config: input.config,
      hasApiKey: Boolean(input.apiKey),
    }));
    vi.mocked(testProvider).mockResolvedValue({
      basicRequest: { supported: true, message: "基础请求成功", diagnostic: null },
    });
  });

  it("shows one expandable sanitized diagnostic for a failed Responses request", async () => {
    vi.mocked(testProvider).mockResolvedValue({
      basicRequest: {
        supported: false,
        message: "模型返回 HTTP 503：Service temporarily unavailable",
        diagnostic: {
          stage: "http_status",
          endpoint: "https://relay.example/v1/responses",
          httpStatus: 503,
          contentType: "application/json",
          responseHeaders: ["server: cloudflare", "cf-ray: test"],
          responseBody: '{"error":{"message":"Service temporarily unavailable"}}',
          parseError: "模型返回 HTTP 503：Service temporarily unavailable",
          truncated: false,
          requestBody: '{"model":"gpt-5.2","stream":false}',
        },
      },
    });
    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    await wrapper.get('[data-field="provider-name"]').setValue("Relay");
    await wrapper.get('[data-field="base-url"]').setValue("https://relay.example/v1");
    await wrapper.get('[data-field="api-key"]').setValue("secret-key");
    await wrapper.get('[data-action="fetch-models"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-action="test-provider"]').trigger("click");
    await flushPromises();

    expect(wrapper.findAll('[data-role="capability-diagnostic"]')).toHaveLength(1);
    const details = wrapper.get('[data-role="capability-diagnostic"]');
    expect(details.text()).toContain("HTTP 503");
    expect(details.text()).toContain("/v1/responses");
    expect(details.text()).toContain("Service temporarily unavailable");
    expect(details.text()).not.toContain("secret-key");
    expect(details.text()).toContain("请求正文");
  });

  it("fetches, selects, saves, and tests a Responses provider", async () => {
    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    expect(wrapper.text()).not.toContain("温度");
    expect(wrapper.text()).not.toContain("最大输出 Token");
    expect(wrapper.text()).not.toContain("Chat Completions");

    await wrapper.get('[data-field="provider-name"]').setValue("Relay");
    await wrapper.get('[data-field="base-url"]').setValue("https://relay.example/v1");
    await wrapper.get('[data-field="api-key"]').setValue("secret-key");
    await wrapper.get('[data-action="fetch-models"]').trigger("click");
    await flushPromises();

    expect(listProviderModels).toHaveBeenCalledWith(expect.objectContaining({
      config: expect.objectContaining({ baseUrl: "https://relay.example/v1" }),
      apiKey: "secret-key",
    }));
    expect((wrapper.get('[data-field="model-id"]').element as HTMLSelectElement).value).toBe("gpt-5.1");

    await wrapper.get('[data-field="model-id"]').setValue("gpt-5.2");
    await wrapper.get('[data-action="save-provider"]').trigger("click");
    await flushPromises();

    expect(saveProvider).toHaveBeenCalledWith(expect.objectContaining({
      config: expect.objectContaining({
        name: "Relay",
        baseUrl: "https://relay.example/v1",
        model: "gpt-5.2",
      }),
      apiKey: "secret-key",
    }));
    const savedConfig = vi.mocked(saveProvider).mock.calls[0][0].config as unknown as Record<string, unknown>;
    expect(savedConfig).not.toHaveProperty("temperature");
    expect(savedConfig).not.toHaveProperty("maxTokens");
    expect(savedConfig).not.toHaveProperty("wireApi");

    await wrapper.get('[data-action="test-provider"]').trigger("click");
    await flushPromises();
    expect(testProvider).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("基础请求成功");
  });

  it("shows saved credential state without exposing the key", async () => {
    vi.mocked(listProviders).mockResolvedValue([{
      config: {
        id: "saved",
        name: "已保存模型",
        baseUrl: "https://relay.example/v1",
        model: "gpt-5.2",
        secretRef: "provider:saved",
        timeoutMs: 30000,
        streaming: true,
        toolCalling: true,
        enabled: true,
      },
      hasApiKey: true,
    }]);

    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    expect(wrapper.text()).toContain("已安全保存 API Key");
    expect((wrapper.get('[data-field="api-key"]').element as HTMLInputElement).value).toBe("");
    expect((wrapper.get('[data-field="model-id"]').element as HTMLSelectElement).value).toBe("gpt-5.2");
  });

  it("marks one saved provider as the default for new conversations", async () => {
    vi.mocked(listProviders).mockResolvedValue([{
      config: {
        id: "saved", name: "已保存模型", baseUrl: "https://relay.example/v1", model: "gpt-5.2",
        secretRef: "provider:saved", timeoutMs: 30000, streaming: true, toolCalling: true, enabled: true,
      },
      hasApiKey: true,
    }]);
    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    await wrapper.get('[data-field="default-provider"]').setValue(true);
    await wrapper.get('[data-action="save-provider"]').trigger("click");
    await flushPromises();

    expect(saveProvider).toHaveBeenCalledWith(expect.objectContaining({
      config: expect.objectContaining({ id: "saved", isDefault: true }),
    }));
  });

  it("starts with empty endpoint fields and requires model discovery", async () => {
    const wrapper = mount(ModelSettingsPage);
    await flushPromises();

    expect((wrapper.get('[data-field="base-url"]').element as HTMLInputElement).value).toBe("");
    expect(wrapper.get('[data-field="base-url"]').attributes("placeholder")).toBe("https://api.example.com/v1");
    expect(wrapper.get('[data-field="model-id"]').attributes("disabled")).toBeDefined();
    expect(wrapper.get('[data-action="test-provider"]').attributes("disabled")).toBeDefined();
  });
});
