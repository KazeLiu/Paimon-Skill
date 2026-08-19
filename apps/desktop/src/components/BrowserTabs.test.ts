import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  listBrowserTabs: vi.fn(),
  openBrowserTab: vi.fn(),
  closeBrowserTab: vi.fn(),
  selectBrowserTab: vi.fn(),
  setBrowserVisibility: vi.fn(),
  setBrowserBounds: vi.fn(),
  navigateBrowserTab: vi.fn(),
  reloadBrowserTab: vi.fn(),
  browserHistory: vi.fn(),
  extractBrowserPage: vi.fn(),
  scrollBrowserPage: vi.fn(),
  clickBrowserTarget: vi.fn(),
}));

vi.mock("@/api/browser", () => api);
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }));

import BrowserTabs from "./BrowserTabs.vue";

describe("BrowserTabs", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    api.listBrowserTabs.mockResolvedValue([]);
    api.setBrowserVisibility.mockResolvedValue(undefined);
    api.setBrowserBounds.mockResolvedValue(undefined);
  });

  it("opens an address and emits the extracted page for the current conversation", async () => {
    const tab = {
      id: "tab-1",
      title: "Example",
      url: "https://example.com",
      loading: false,
      profileId: "default",
    };
    const snapshot = {
      title: "Example",
      url: "https://example.com",
      text: "Visible page text",
      links: [],
      loginChallenge: false,
      challengeReason: null,
      truncated: false,
    };
    api.openBrowserTab.mockResolvedValue(tab);
    api.extractBrowserPage.mockResolvedValue(snapshot);
    const wrapper = mount(BrowserTabs);
    await flushPromises();

    await wrapper.get('[data-field="browser-address"]').setValue("example.com");
    await wrapper.get('[data-action="open-address"]').trigger("submit");
    await flushPromises();
    await wrapper.get('[data-action="ask-current-page"]').trigger("click");
    await flushPromises();

    expect(api.openBrowserTab).toHaveBeenCalledWith("example.com", expect.any(Object));
    expect(wrapper.emitted("ask-page")?.[0]).toEqual([snapshot]);
  });
});

