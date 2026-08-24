import { createPinia, setActivePinia } from "pinia";
import { flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  listBrowserTabs: vi.fn(),
  openBrowserTab: vi.fn(),
  closeBrowserTab: vi.fn(),
  selectBrowserTab: vi.fn(),
  navigateBrowserTab: vi.fn(),
  reloadBrowserTab: vi.fn(),
  browserHistory: vi.fn(),
  extractBrowserPage: vi.fn(),
  checkSourceLogin: vi.fn(),
  scrollBrowserPage: vi.fn(),
  clickBrowserTarget: vi.fn(),
  checkSourceConnection: vi.fn(),
}));

vi.mock("@/api/browser", () => api);
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }));

import SourceDirectoryPage from "./SourceDirectoryPage.vue";
import { useBrowserStore } from "@/stores/browserStore";

describe("SourceDirectoryPage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    localStorage.clear();
    api.listBrowserTabs.mockResolvedValue([]);
    api.checkSourceLogin.mockResolvedValue({ status: "logged-out", message: "未检测到有效登录 Cookie" });
  });

  afterEach(() => vi.useRealTimers());

  it("renders fixed sources with connection states and login only for gated sources", () => {
    const wrapper = mount(SourceDirectoryPage);

    expect(wrapper.text()).toContain("BWiki");
    expect(wrapper.text()).toContain("米游社观测枢");
    expect(wrapper.text()).toContain("B站视频");
    expect(wrapper.text()).toContain("小红书");
    expect(wrapper.text()).toContain("知乎");
    expect(wrapper.text()).toContain("Genshin Impact Wiki（英文）");
    expect(wrapper.text()).toContain("Genshin Impact Wiki（俄语）");
    expect(wrapper.text()).toContain("Project Amber（英文）");
    expect(wrapper.text()).toContain("Project Amber（俄语）");
    expect(wrapper.text()).toContain("Reddit / r/Genshin_Lore");
    expect(wrapper.findAll('[data-role="source-card"]').length).toBe(10);
    expect(wrapper.findAll('[data-status="connection"]').length).toBe(10);
    expect(wrapper.findAll('[data-status="login"]').length).toBe(2);
    expect(wrapper.findAll('[data-role="source-icon"] img').length).toBe(10);
  });

  it("shows each source purpose, category, and authority level", () => {
    const wrapper = mount(SourceDirectoryPage);

    const obc = wrapper.get('[data-source-id="obc"]');
    expect(obc.text()).toContain("第一方资料");
    expect(obc.text()).toContain("基础档案、材料、数值与官方说明");

    const wikiEn = wrapper.get('[data-source-id="genshin-wiki-en"]');
    expect(wikiEn.text()).toContain("多语言社区维护 Wiki");
    expect(wikiEn.text()).toContain("Etymology");

    const bilibili = wrapper.get('[data-source-id="bilibili-video"]');
    expect(bilibili.text()).toContain("玩家经验");
    expect(bilibili.text()).toContain("攻略参考");
  });

  it("keeps one profile description and provides a prefilled issue link for new sources", () => {
    const wrapper = mount(SourceDirectoryPage);
    const recommendation = wrapper.get('[data-action="recommend-source"]');

    expect(wrapper.text()).toContain("登录状态保存在应用独立的 WebView2 Profile 中。");
    expect(wrapper.text()).not.toContain("选择要参与查询的站点");
    expect(recommendation.attributes("href")).toContain("github.com/KazeLiu/Paimon-Skill/issues/new");
    expect(recommendation.attributes("href")).toContain("body=");
    expect(recommendation.text()).toContain("推荐一个信息源");
  });

  it("persists whether a fixed source is enabled", async () => {
    const wrapper = mount(SourceDirectoryPage);
    const toggle = wrapper.get('[data-source-id="xiaohongshu"] [data-action="toggle-source"]');

    await toggle.trigger("click");

    expect(toggle.attributes("aria-pressed")).toBe("false");
    expect(localStorage.getItem("paimon.source.enabled.xiaohongshu")).toBe("false");
  });

  it("checks only source connections sequentially when the page opens", async () => {
    const checked: string[] = [];
    api.checkSourceConnection.mockImplementation(async (url: string) => {
      checked.push(url);
      return { reachable: true, statusCode: 200, message: "HTTP 200" };
    });
    mount(SourceDirectoryPage);
    await flushPromises();

    expect(api.checkSourceConnection).toHaveBeenCalledTimes(10);
    expect(checked[0]).toBe("https://baike.mihoyo.com/ys/obc/?visit_device=pc&bbs_presentation_style=no_header");
    expect(checked[checked.length - 1]).toBe("https://www.bilibili.com/");
    expect(api.openBrowserTab).not.toHaveBeenCalled();
    expect(api.checkSourceLogin).not.toHaveBeenCalled();
  });

  it("reads cookies and opens a gated source only when no login cookie exists", async () => {
    const tab = {
      id: "tab-login",
      title: "小红书",
      url: "https://www.xiaohongshu.com/explore",
      loading: false,
      profileId: "default",
    };
    api.checkSourceConnection.mockResolvedValue({ reachable: true, statusCode: 200, message: "HTTP 200" });
    api.openBrowserTab.mockResolvedValue(tab);
    api.checkSourceLogin.mockResolvedValue({ status: "logged-out", message: "未检测到有效登录 Cookie" });
    const wrapper = mount(SourceDirectoryPage);
    await flushPromises();

    expect(api.openBrowserTab).not.toHaveBeenCalled();
    await wrapper.get('[data-source-id="xiaohongshu"] [data-status="login"]').trigger("click");
    await flushPromises();

    expect(api.checkSourceLogin).toHaveBeenCalledWith("xiaohongshu");
    expect(api.openBrowserTab).toHaveBeenCalledTimes(1);
    expect(api.openBrowserTab).toHaveBeenCalledWith("https://www.xiaohongshu.com/explore");
    expect(api.extractBrowserPage).not.toHaveBeenCalled();
    const card = wrapper.get('[data-source-id="xiaohongshu"]');
    expect(card.get('[data-status="connection"]').text()).toContain("可连接");
    expect(card.get('[data-status="login"]').text()).toContain("未登录");
    expect(wrapper.findAll('[data-action="login-source"]').length).toBe(0);
    expect(wrapper.findAll('[data-action="check-source-status"]').length).toBe(0);
  });

  it("rechecks a single source when its connection tag is clicked", async () => {
    api.checkSourceConnection.mockResolvedValue({ reachable: true, statusCode: 200, message: "HTTP 200" });
    const wrapper = mount(SourceDirectoryPage);
    await flushPromises();
    const before = api.checkSourceConnection.mock.calls.length;
    await wrapper.get('[data-source-id="obc"] [data-status="connection"]').trigger("click");
    await flushPromises();
    expect(api.checkSourceConnection.mock.calls.length).toBe(before + 1);
  });

  it("does not open a login window when an authenticated cookie exists", async () => {
    api.checkSourceConnection.mockResolvedValue({ reachable: true, statusCode: 200, message: "HTTP 200" });
    api.checkSourceLogin.mockResolvedValue({ status: "logged-in", message: "检测到有效登录 Cookie" });
    const wrapper = mount(SourceDirectoryPage);
    await flushPromises();

    await wrapper.get('[data-source-id="xiaohongshu"] [data-status="login"]').trigger("click");
    await flushPromises();

    expect(api.checkSourceLogin).toHaveBeenCalledTimes(1);
    expect(api.openBrowserTab).not.toHaveBeenCalled();
    expect(wrapper.get('[data-source-id="xiaohongshu"] [data-status="login"]').text()).toContain("已登录");
    wrapper.unmount();
  });

  it("reopens a gated source after its previous browser window was closed", async () => {
    const firstTab = {
      id: "tab-login-1",
      title: "小红书",
      url: "https://www.xiaohongshu.com/explore",
      loading: false,
      profileId: "default",
    };
    const secondTab = { ...firstTab, id: "tab-login-2" };
    api.checkSourceConnection.mockResolvedValue({ reachable: true, statusCode: 200, message: "HTTP 200" });
    api.openBrowserTab.mockResolvedValueOnce(firstTab).mockResolvedValueOnce(secondTab);
    api.checkSourceLogin.mockResolvedValue({ status: "logged-out", message: "未检测到有效登录 Cookie" });
    const wrapper = mount(SourceDirectoryPage);
    await flushPromises();

    const loginTag = () => wrapper.get('[data-source-id="xiaohongshu"] [data-status="login"]');
    await loginTag().trigger("click");
    await flushPromises();

    const browserStore = useBrowserStore();
    browserStore.tabs = [];
    await loginTag().trigger("click");
    await flushPromises();

    expect(api.openBrowserTab).toHaveBeenCalledTimes(2);
    expect(api.checkSourceLogin).toHaveBeenCalledTimes(2);
    expect(api.openBrowserTab).toHaveBeenLastCalledWith("https://www.xiaohongshu.com/explore");
    wrapper.unmount();
  });

  it("rechecks cookies after login and keeps the closed login window closed", async () => {
    const firstTab = {
      id: "tab-login-1",
      title: "小红书",
      url: "https://www.xiaohongshu.com/explore",
      loading: false,
      profileId: "default",
    };
    api.checkSourceConnection.mockResolvedValue({ reachable: true, statusCode: 200, message: "HTTP 200" });
    api.openBrowserTab.mockResolvedValue(firstTab);
    api.checkSourceLogin
      .mockResolvedValueOnce({ status: "logged-out", message: "未检测到有效登录 Cookie" })
      .mockResolvedValueOnce({ status: "logged-in", message: "检测到有效登录 Cookie" });
    const wrapper = mount(SourceDirectoryPage);
    await flushPromises();

    const loginTag = () => wrapper.get('[data-source-id="xiaohongshu"] [data-status="login"]');
    await loginTag().trigger("click");
    await flushPromises();
    useBrowserStore().tabs = [];
    await loginTag().trigger("click");
    await flushPromises();

    expect(api.checkSourceLogin).toHaveBeenCalledTimes(2);
    expect(api.openBrowserTab).toHaveBeenCalledTimes(1);
    expect(loginTag().text()).toContain("已登录");
    wrapper.unmount();
  });
});
