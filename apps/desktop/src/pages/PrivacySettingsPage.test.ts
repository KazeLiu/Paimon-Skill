import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { clearBrowserProfile } from "@/api/browser";
import PrivacySettingsPage from "@/pages/PrivacySettingsPage.vue";

vi.mock("@/api/browser", () => ({
  clearBrowserProfile: vi.fn(),
}));

describe("PrivacySettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("requires confirmation before clearing the persistent browser profile", async () => {
    vi.mocked(clearBrowserProfile).mockResolvedValue();
    const wrapper = mount(PrivacySettingsPage);

    await wrapper.get('[data-action="request-clear-browser-profile"]').trigger("click");
    expect(clearBrowserProfile).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("将退出应用内网页的登录状态");

    await wrapper.get('[data-action="confirm-clear-browser-profile"]').trigger("click");

    expect(clearBrowserProfile).toHaveBeenCalledTimes(1);
    expect(wrapper.text()).toContain("浏览器登录态已清理");
  });

  it("can cancel browser profile cleanup without invoking the command", async () => {
    const wrapper = mount(PrivacySettingsPage);

    await wrapper.get('[data-action="request-clear-browser-profile"]').trigger("click");
    await wrapper.get('[data-action="cancel-clear-browser-profile"]').trigger("click");

    expect(clearBrowserProfile).not.toHaveBeenCalled();
    expect(wrapper.find('[data-action="confirm-clear-browser-profile"]').exists()).toBe(false);
  });
});
