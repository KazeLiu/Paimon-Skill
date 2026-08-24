import { reactive, toRefs } from "vue";
import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const state = vi.hoisted(() => ({
  chat: null as Record<string, unknown> | null,
}));

vi.mock("pinia", async (importOriginal) => {
  const actual = await importOriginal<typeof import("pinia")>();
  return {
    ...actual,
    storeToRefs: (store: Record<string, unknown>) => toRefs(store),
  };
});

vi.mock("@/stores/chatStore", () => ({ useChatStore: () => state.chat }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(vi.fn()) }));

import App from "./App.vue";

describe("App shell", () => {
  beforeEach(() => {
    state.chat = reactive({
      providers: [],
      conversations: [],
      messages: [],
      selectedProviderId: null,
      activeConversationId: null,
      activeRunId: null,
      sending: false,
      error: "",
      sources: [],
      toolActivities: [],
      activeConversation: null,
      initialize: vi.fn().mockResolvedValue(undefined),
      bindEvents: vi.fn().mockResolvedValue(undefined),
      newConversation: vi.fn(),
      selectConversation: vi.fn(),
      remove: vi.fn(),
      send: vi.fn(),
      cancel: vi.fn(),
      retry: vi.fn(),
      handleSource: vi.fn(),
      handleTool: vi.fn(),
    });
  });

  function mountApp() {
    return mount(App, {
      global: {
        stubs: {
          MessageList: true,
          ModelSettingsPage: true,
          PromptSettingsPage: true,
          PrivacySettingsPage: true,
          DiagnosticsSettingsPage: { template: '<div data-testid="diagnostics-settings">诊断日志</div>' },
          SourceDirectoryPage: { template: '<div data-testid="source-directory">信息源目录</div>' },
        },
      },
    });
  }

  it("collapses and restores the query activity pane", async () => {
    const wrapper = mountApp();

    expect(wrapper.get(".app-shell").classes()).not.toContain("activity-pane-collapsed");
    await wrapper.get('button[aria-label="收起查询面板"]').trigger("click");
    expect(wrapper.get(".app-shell").classes()).toContain("activity-pane-collapsed");

    await wrapper.get('button[aria-label="打开查询面板"]').trigger("click");
    expect(wrapper.get(".app-shell").classes()).not.toContain("activity-pane-collapsed");
  });

  it("opens the information source directory instead of a browser page", async () => {
    const wrapper = mountApp();

    await wrapper.get('[data-action="open-sources"]').trigger("click");

    expect(wrapper.text()).toContain("信息源");
    expect(wrapper.find('[data-testid="source-directory"]').exists()).toBe(true);
  });

  it("hides the duplicate workspace header on the information source page", async () => {
    const wrapper = mountApp();

    await wrapper.get('[data-action="open-sources"]').trigger("click");

    expect(wrapper.find(".workspace-header").exists()).toBe(false);
  });

  it("hides research activity on the information source page", async () => {
    const wrapper = mountApp();

    await wrapper.get('[data-action="open-sources"]').trigger("click");

    expect(wrapper.find('.activity-pane').exists()).toBe(false);
    expect(wrapper.get('.app-shell').classes()).toContain('activity-pane-collapsed');
  });

  it("shows research activity only on the chat page", async () => {
    const wrapper = mountApp();
    expect(wrapper.find('.activity-pane').exists()).toBe(true);

    await wrapper.get('.rail-settings').trigger('click');

    expect(wrapper.find('.activity-pane').exists()).toBe(false);
    expect(wrapper.get('.app-shell').classes()).toContain('activity-pane-collapsed');
  });

  it("opens diagnostics from the settings tabs", async () => {
    const wrapper = mountApp();
    await wrapper.get('.rail-settings').trigger('click');
    await wrapper.get('[data-settings-section="diagnostics"]').trigger('click');
    expect(wrapper.find('[data-testid="diagnostics-settings"]').exists()).toBe(true);
  });

  it("confirms and deletes one conversation without selecting it", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    Object.assign(state.chat!, {
      conversations: [{ id: "c1", title: "待删除", providerId: "deepseek", createdAt: "1", updatedAt: "1" }],
    });
    const wrapper = mountApp();

    await wrapper.get('button[aria-label="删除对话：待删除"]').trigger("click");

    expect(confirm).toHaveBeenCalled();
    expect(state.chat!.remove).toHaveBeenCalledWith("c1");
  });

  it("keeps deleting the active conversation available while it is generating", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    Object.assign(state.chat!, {
      conversations: [{ id: "c1", title: "生成中", providerId: "deepseek", createdAt: "1", updatedAt: "1" }],
      activeConversationId: "c1",
      sending: true,
    });
    const wrapper = mountApp();

    const button = wrapper.get('button[aria-label="删除对话：生成中"]');
    expect(button.attributes("disabled")).toBeUndefined();
    await button.trigger("click");

    expect(confirm).toHaveBeenCalled();
    expect(state.chat!.remove).toHaveBeenCalledWith("c1");
  });

  it("shows a deletion error next to the conversation list", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    Object.assign(state.chat!, {
      conversations: [{ id: "c1", title: "待删除", providerId: "deepseek", createdAt: "1", updatedAt: "1" }],
      remove: vi.fn().mockRejectedValue(new Error("删除失败：对话仍在生成")),
    });
    const wrapper = mountApp();

    await wrapper.get('button[aria-label="删除对话：待删除"]').trigger("click");

    expect(confirm).toHaveBeenCalled();
    expect(wrapper.get('[role="alert"]').text()).toContain("删除失败：对话仍在生成");
  });

  it("filters conversations by the search query", async () => {
    Object.assign(state.chat!, {
      conversations: [
        { id: "c1", title: "芙宁娜配队", providerId: "deepseek", createdAt: "1", updatedAt: "1" },
        { id: "c2", title: "纳西妲圣遗物", providerId: "deepseek", createdAt: "2", updatedAt: "2" },
      ],
    });
    const wrapper = mountApp();
    const search = wrapper.get('input[placeholder="搜索对话"]');

    await search.setValue("圣遗物");

    expect(wrapper.find('button[aria-label="删除对话：纳西妲圣遗物"]').exists()).toBe(true);
    expect(wrapper.find('button[aria-label="删除对话：芙宁娜配队"]').exists()).toBe(false);

    await search.setValue("不存在的对话");
    expect(wrapper.text()).toContain("没有匹配的对话");
  });

  it("allows attachment-only messages and forwards selected files", async () => {
    Object.assign(state.chat!, {
      providers: [{ config: { id: "deepseek", name: "DeepSeek", model: "vision" } }],
      selectedProviderId: "deepseek",
    });
    const wrapper = mountApp();
    const file = new File(["hello"], "notes.txt", { type: "text/plain" });
    const input = wrapper.get('input[type="file"]');
    Object.defineProperty(input.element, "files", { value: [file], configurable: true });

    await input.trigger("change");
    await vi.waitFor(() => expect(wrapper.text()).toContain("notes.txt"));

    expect(wrapper.text()).toContain("notes.txt");
    expect(wrapper.get('button[aria-label="发送消息"]').attributes("disabled")).toBeUndefined();
    await wrapper.get("form.composer").trigger("submit");
    expect(state.chat!.send).toHaveBeenCalledWith("", [expect.objectContaining({ name: "notes.txt", kind: "file" })]);
  });

  it("does not open the attachment picker before a model is selected", async () => {
    const wrapper = mountApp();

    const button = wrapper.get('button[aria-label="添加图片或文件"]');
    expect(button.attributes("disabled")).toBeDefined();
    expect(wrapper.text()).toContain("请先选择模型");
  });

  it("normalizes MIME type for files whose browser MIME is empty", async () => {
    Object.assign(state.chat!, {
      providers: [{ config: { id: "relay", name: "Relay", model: "gpt-5.2" } }],
      selectedProviderId: "relay",
    });
    const wrapper = mountApp();
    const file = new File(["name,value"], "data.csv", { type: "" });
    const input = wrapper.get('input[type="file"]');
    Object.defineProperty(input.element, "files", { value: [file], configurable: true });

    await input.trigger("change");
    await vi.waitFor(() => expect(wrapper.text()).toContain("data.csv"));
    await wrapper.get("form.composer").trigger("submit");

    expect(state.chat!.send).toHaveBeenCalledWith("", [expect.objectContaining({
      name: "data.csv",
      mimeType: "text/csv",
    })]);
  });

  it("opens the full model control and selects a provider from the menu", async () => {
    Object.assign(state.chat!, {
      providers: [
        { config: { id: "luna", name: "mdkj", model: "5.6-luna" } },
        { config: { id: "terra", name: "mdkj", model: "5.6-terra" } },
      ],
      selectedProviderId: "luna",
    });
    const wrapper = mountApp();

    await wrapper.get('[data-action="model-selector"]').trigger("click");
    expect(wrapper.get('[role="listbox"]').isVisible()).toBe(true);

    await wrapper.get('[data-provider-id="terra"]').trigger("click");

    expect(state.chat!.selectedProviderId).toBe("terra");
    expect(wrapper.find('[role="listbox"]').exists()).toBe(false);
  });

  it("renders research activity newest first and exposes details", async () => {
    Object.assign(state.chat!, {
      toolActivities: [
        { conversationId: "c1", runId: "run-1", toolName: "read_url", status: "completed", createdAt: "2026-08-20T10:00:00Z", details: "旧工具详情" },
      ],
      sources: [
        { title: "新网页", url: "https://example.com/new", sourceLevel: "web", platform: "网页", capturedAt: "2026-08-20T11:00:00Z" },
      ],
    });
    const wrapper = mountApp();
    const items = wrapper.findAll('[data-role="activity-item"]');

    expect(items[0].text()).toContain("新网页");
    expect(items[0].attributes("data-url")).toBe("https://example.com/new");
    expect(wrapper.text()).toContain("旧工具详情");
  });

  it("groups adjacent tool activity into one expandable workflow card", () => {
    Object.assign(state.chat!, {
      toolActivities: [
        { conversationId: "c1", runId: "run-1", toolName: "browser_close_page", status: "running", createdAt: "2026-08-20T10:00:04Z" },
        { conversationId: "c1", runId: "run-1", toolName: "browser_close_page", status: "completed", createdAt: "2026-08-20T10:00:03Z" },
        { conversationId: "c1", runId: "run-1", toolName: "browser_close_page", status: "running", createdAt: "2026-08-20T10:00:02Z" },
        { conversationId: "c1", runId: "run-1", toolName: "browser_close_page", status: "completed", createdAt: "2026-08-20T10:00:01Z" },
      ],
      sources: [],
      toolRuns: [],
    });

    const wrapper = mountApp();
    const items = wrapper.findAll('[data-role="activity-item"]');

    expect(items).toHaveLength(1);
    expect(items[0].text()).toContain("browser_close_page");
    expect(items[0].text()).toContain("4 次");
    expect(items[0].findAll('[data-role="activity-step"]')).toHaveLength(4);
  });
});
