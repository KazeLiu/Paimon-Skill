import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const api = vi.hoisted(() => ({
  listBrowserTabs: vi.fn(),
  openBrowserTab: vi.fn(),
  selectBrowserTab: vi.fn(),
  setBrowserVisibility: vi.fn(),
  clickBrowserTarget: vi.fn(),
}));

vi.mock("@/api/browser", () => api);

import { useBrowserStore } from "@/stores/browserStore";

describe("browserStore", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    api.listBrowserTabs.mockResolvedValue([]);
  });

  it("keeps existing tabs when the browser surface is hidden and shown again", async () => {
    const tab = {
      id: "tab-1",
      title: "Example",
      url: "https://example.com",
      loading: false,
      profileId: "default",
    };
    api.openBrowserTab.mockResolvedValue(tab);
    const store = useBrowserStore();

    await store.open("https://example.com");
    await store.setVisible(false);
    await store.setVisible(true);

    expect(store.tabs).toEqual([tab]);
    expect(store.activeTabId).toBe("tab-1");
    expect(api.setBrowserVisibility).toHaveBeenNthCalledWith(1, false);
    expect(api.setBrowserVisibility).toHaveBeenNthCalledWith(2, true);
  });

  it("keeps a read-only click failure visible in the browser toolbar", async () => {
    const store = useBrowserStore();
    store.tabs = [{
      id: "tab-1",
      title: "Example",
      url: "https://example.com",
      loading: false,
      profileId: "default",
    }];
    store.activeTabId = "tab-1";
    api.clickBrowserTarget.mockRejectedValue(new Error("只读浏览器拒绝执行该点击"));

    await expect(store.click("立即发布")).rejects.toThrow("只读浏览器拒绝执行该点击");

    expect(store.error).toBe("只读浏览器拒绝执行该点击");
    expect(store.loadingAction).toBe(false);
  });
});
