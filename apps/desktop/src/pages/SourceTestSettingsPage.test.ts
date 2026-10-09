import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SourceTestPlatform, SourceTestProgress, SourceTestResult } from "@/types/sourceTests";

const events = vi.hoisted(() => ({
  handler: null as ((event: { payload: SourceTestProgress }) => void) | null,
  unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn(async (_name, handler) => { events.handler = handler; return events.unlisten; }) }));
vi.mock("@/api/sourceTests", () => ({ runSourceTest: vi.fn() }));
vi.mock("@/api/browser", () => ({ openBrowserTab: vi.fn().mockResolvedValue({}) }));

import SourceTestSettingsPage from "./SourceTestSettingsPage.vue";
import { runSourceTest } from "@/api/sourceTests";
import { openBrowserTab } from "@/api/browser";
import { useSourceTestStore } from "@/stores/sourceTestStore";

function fixture(platform: SourceTestPlatform = "bilibili"): SourceTestResult {
  return {
    testId: "test-id", platform, url: "https://www.bilibili.com/video/BV1abc", startedAt: "2026-10-09T10:00:00Z", elapsedMs: 1250,
    status: "partial", checks: [
      { key: "subtitles", label: "字幕", status: "missing", detail: "正式报告未取得字幕" },
      { key: "comments", label: "评论", status: "missing", detail: "没有实际评论，头部不计作评论" },
      { key: "views", label: "播放量", status: "missing", detail: "未返回播放量" },
    ], metadata: {}, images: [],
    report: { title: "测试视频", body: null, comments: null, transcript: null, warnings: [] },
    steps: [{ index: 0, action: "extract_subtitles", status: "returned", elapsedMs: 5, data: { title: "视频", url: "https://www.bilibili.com/video/BV1abc", content: "{}", metadata: {} }, error: null }],
    warnings: ["字幕页面数据无效：missing field `version`"], error: null,
  };
}

describe("SourceTestSettingsPage", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    events.handler = null;
    vi.mocked(runSourceTest).mockResolvedValue(fixture());
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
  });

  it("uses the test command and displays missing fields, actual parsing warnings and raw output", async () => {
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get("#source-test-url").setValue("https://www.bilibili.com/video/BV1abc");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(runSourceTest).toHaveBeenCalledWith({ testId: expect.any(String), platform: "bilibili", url: "https://www.bilibili.com/video/BV1abc" });
    expect(wrapper.get('[data-check="subtitles"]').text()).toContain("未取得");
    expect(wrapper.text()).toContain("missing field `version`");
    expect(wrapper.text()).toContain('"content": "{}"');
    expect(wrapper.text()).toContain("工具返回不代表所需字段已取得");
    await wrapper.get('[data-action="copy-source-test"]').trigger("click");
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(JSON.stringify(fixture(), null, 2));
    expect(wrapper.text()).toContain("已复制脱敏测试结果");
  });

  it("preserves each platform's URL and result across switches", async () => {
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get("#source-test-url").setValue("https://www.bilibili.com/video/BV1abc");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    await wrapper.get('[data-platform="xiaohongshu"]').setValue();
    await wrapper.get("#source-test-url").setValue("https://www.xiaohongshu.com/explore/note1?xsec_token=local-navigation-token");
    await wrapper.get('[data-platform="bilibili"]').setValue();
    expect((wrapper.get("#source-test-url").element as HTMLInputElement).value).toContain("BV1abc");
    expect(wrapper.text()).toContain("测试视频");
    await wrapper.get('[data-platform="xiaohongshu"]').setValue();
    expect((wrapper.get("#source-test-url").element as HTMLInputElement).value).toContain("local-navigation-token");
    expect(wrapper.find('[data-check="subtitles"]').exists()).toBe(false);
  });

  it("distinguishes extracted image URLs from image preview loading failures", async () => {
    const result = fixture("xiaohongshu");
    result.checks = [{ key: "images", label: "图片", status: "returned", detail: "正式结果返回 1 个可预览图片地址" }];
    result.images = ["https://cdn.example.com/note.jpg"];
    vi.mocked(runSourceTest).mockResolvedValue(result);
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get('[data-platform="xiaohongshu"]').setValue();
    await wrapper.get("#source-test-url").setValue("https://www.xiaohongshu.com/explore/note1");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('img[alt="提取图片 1"]').attributes("src")).toBe(result.images[0]);
    await wrapper.get('img[alt="提取图片 1"]').trigger("error");
    expect(wrapper.text()).toContain("预览加载失败");
    expect(wrapper.get('[data-check="images"]').text()).toContain("已返回");
  });

  it("does not turn a Zhihu visual candidate into an extracted image", async () => {
    const result = fixture("zhihu");
    result.checks = [{ key: "images", label: "图片", status: "missing", detail: "发现图片线索，但未返回图片 URL" }];
    result.metadata = { visualCandidate: "true" };
    vi.mocked(runSourceTest).mockResolvedValue(result);
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get('[data-platform="zhihu"]').setValue();
    await wrapper.get("#source-test-url").setValue("https://www.zhihu.com/question/1/answer/2");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('[data-check="images"]').text()).toContain("未取得");
    expect(wrapper.find('img').exists()).toBe(false);
  });

  it("shows progress for this test only and prevents duplicate submissions", async () => {
    let finish!: (value: SourceTestResult) => void;
    vi.mocked(runSourceTest).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get("#source-test-url").setValue("https://www.bilibili.com/video/BV1abc");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('[data-action="run-source-test"]').attributes("disabled")).toBeDefined();
    const store = useSourceTestStore();
    const step = { index: 0, action: "open_page", status: "running" as const, elapsedMs: 0, data: null, error: null };
    events.handler!({ payload: { testId: "another-test", step } });
    expect(store.cases.bilibili.steps).toHaveLength(0);
    events.handler!({ payload: { testId: vi.mocked(runSourceTest).mock.calls[0][0].testId, step } });
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain("打开详情页");
    expect(wrapper.text()).toContain("进行中");
    await wrapper.get("form").trigger("submit");
    expect(runSourceTest).toHaveBeenCalledTimes(1);
    finish(fixture());
    await flushPromises();
    expect(events.unlisten).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-action="run-source-test"]').attributes("disabled")).toBeUndefined();
  });

  it("shows validation and command errors and opens the formal browser for manual login", async () => {
    const wrapper = mount(SourceTestSettingsPage);
    await wrapper.get("#source-test-url").setValue("file:///local");
    await wrapper.get("form").trigger("submit");
    expect(runSourceTest).not.toHaveBeenCalled();
    expect(wrapper.text()).toContain("HTTP/HTTPS");
    await wrapper.get("#source-test-url").setValue("https://www.zhihu.com/question/1/answer/2");
    vi.mocked(runSourceTest).mockRejectedValue("链接与选择的平台不匹配");
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.text()).toContain("链接与选择的平台不匹配");
    expect(useSourceTestStore().runningPlatform).toBeNull();
    await wrapper.get('[data-action="open-test-page"]').trigger("click");
    expect(openBrowserTab).toHaveBeenCalledWith("https://www.zhihu.com/question/1/answer/2");
  });
});
