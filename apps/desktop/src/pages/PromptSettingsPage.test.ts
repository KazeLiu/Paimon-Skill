import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import PromptSettingsPage from "./PromptSettingsPage.vue";
import {
  getPromptSettings,
  listPromptResources,
  resetPromptSettings,
  savePromptSettings,
} from "@/api/prompts";

vi.mock("@/api/prompts", () => ({
  getPromptSettings: vi.fn(),
  listPromptResources: vi.fn(),
  resetPromptSettings: vi.fn(),
  savePromptSettings: vi.fn(),
}));

const resources = [
  {
    id: "base-system",
    title: "基础系统规则",
    description: "基础规则",
    content: "default base",
    enabledByDefault: true,
    order: 10,
  },
  {
    id: "genshin-build-wiki",
    title: "原神攻略规则",
    description: "领域规则",
    content: "skill body",
    enabledByDefault: true,
    order: 40,
  },
];

describe("PromptSettingsPage", () => {
  beforeEach(() => {
    vi.mocked(listPromptResources).mockResolvedValue(resources);
    vi.mocked(getPromptSettings).mockResolvedValue({
      overrides: {},
      disabledResourceIds: [],
      customSuffix: "",
      updatedAt: null,
    });
    vi.mocked(savePromptSettings).mockResolvedValue({
      text: "custom base",
      resourceIds: ["base-system"],
    });
    vi.mocked(resetPromptSettings).mockResolvedValue({
      overrides: {},
      disabledResourceIds: [],
      customSuffix: "",
      updatedAt: null,
    });
  });

  it("edits, saves, disables, and resets prompt resources", async () => {
    const wrapper = mount(PromptSettingsPage);
    await flushPromises();

    await wrapper
      .get('[data-resource-id="base-system"] textarea')
      .setValue("custom base");
    await wrapper
      .get('[data-resource-id="genshin-build-wiki"] input[type="checkbox"]')
      .setValue(false);
    await wrapper.get('[data-action="save-prompts"]').trigger("click");
    await flushPromises();

    expect(savePromptSettings).toHaveBeenCalledWith({
      overrides: { "base-system": "custom base" },
      disabledResourceIds: ["genshin-build-wiki"],
      customSuffix: "",
      updatedAt: expect.any(String),
    });
    expect(wrapper.text()).toContain("已保存");

    await wrapper.get('[data-action="reset-prompts"]').trigger("click");
    await flushPromises();

    expect(resetPromptSettings).toHaveBeenCalledOnce();
    expect(
      (wrapper.get('[data-resource-id="base-system"] textarea').element as HTMLTextAreaElement)
        .value,
    ).toBe("default base");
  });

  it("keeps unsaved text visible when save fails", async () => {
    vi.mocked(savePromptSettings).mockRejectedValue(new Error("磁盘不可写"));
    const wrapper = mount(PromptSettingsPage);
    await flushPromises();

    const textarea = wrapper.get('[data-resource-id="base-system"] textarea');
    await textarea.setValue("keep me");
    await wrapper.get('[data-action="save-prompts"]').trigger("click");
    await flushPromises();

    expect((textarea.element as HTMLTextAreaElement).value).toBe("keep me");
    expect(wrapper.text()).toContain("磁盘不可写");
  });
});
