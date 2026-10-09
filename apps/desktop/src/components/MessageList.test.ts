import { mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { describe, expect, it, vi } from "vitest";

import MessageList from "./MessageList.vue";

const providers = [
  { id: "deepseek", label: "DeepSeek · deepseek-chat" },
  { id: "relay", label: "Relay · gpt-5.6-luna" },
];

describe("MessageList", () => {
  it("shows run errors with details and emits retry", async () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [{
          id: "pending:run-1",
          conversationId: "c1",
          role: "assistant",
          content: "",
          status: "error",
          error: "HTTP 503: Service temporarily unavailable",
          retryable: true,
          attachments: [],
          createdAt: "1",
        }],
      },
    });

    expect(wrapper.text()).toContain("请求失败");
    expect(wrapper.text()).toContain("HTTP 503");
    await wrapper.get('button[aria-label="重试这条消息"]').trigger("click");
    expect(wrapper.emitted("retry")).toEqual([["pending:run-1"]]);
  });

  it("shows a stopped reply with an in-place retry action", async () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [{
          id: "pending:run-1",
          conversationId: "c1",
          role: "assistant",
          content: "",
          status: "cancelled",
          error: "已停止生成，可以重试这条回复",
          retryable: true,
          retryMessageId: "m1",
          createdAt: "1",
        }],
      },
    });

    expect(wrapper.text()).toContain("已停止生成");
    await wrapper.get('button[aria-label="重试这条消息"]').trigger("click");
    expect(wrapper.emitted("retry")).toEqual([["pending:run-1"]]);
  });

  it("places the retry notice under the active assistant reply", () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: true,
        retryNotice: "请求失败，正在进行第 3 次重试…",
        messages: [
          { id: "u1", conversationId: "c1", role: "user", content: "问题", createdAt: "1" },
          { id: "pending:run-1", conversationId: "c1", role: "assistant", content: "已有内容", status: "streaming", createdAt: "2" },
        ],
      },
    });

    expect(wrapper.get(".retry-notice").text()).toContain("第 3 次重试");
    expect(wrapper.find(".composer-status").exists()).toBe(false);
  });

  it("lets the user choose another model before retrying a failed message", async () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        providers,
        selectedProviderId: "deepseek",
        messages: [{ id: "failed", conversationId: "c1", role: "assistant", content: "", status: "error", error: "timeout", retryable: true, createdAt: "1" }],
      },
    });

    await wrapper.get('[data-role="retry-provider"]').setValue("relay");
    await wrapper.get('button[aria-label="使用所选模型重试这条消息"]').trigger("click");

    expect(wrapper.emitted("selectProvider")).toEqual([["relay"]]);
    expect(wrapper.emitted("retry")).toEqual([["failed"]]);
  });

  it("renders image and file attachments on a user message", () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [{
          id: "m1",
          conversationId: "c1",
          role: "user",
          content: "请分析",
          attachments: [
            { name: "screen.png", mimeType: "image/png", dataUrl: "data:image/png;base64,YQ==", size: 1, kind: "image" },
            { name: "notes.pdf", mimeType: "application/pdf", dataUrl: "data:application/pdf;base64,YQ==", size: 1, kind: "file" },
          ],
          createdAt: "1",
        }],
      },
    });

    expect(wrapper.get('img[alt="screen.png"]').attributes("src")).toContain("data:image/png");
    expect(wrapper.text()).toContain("notes.pdf");
  });

  it("keeps an anchor at the bottom so the parent can follow new messages", () => {
    const wrapper = mount(MessageList, { props: { streaming: false, messages: [] } });

    expect(wrapper.find('[data-role="message-list-end"]').exists()).toBe(true);
  });

  it("renders one outline entry per question and shows the paired AI reply", async () => {
    const scrollIntoView = vi.fn();
    Element.prototype.scrollIntoView = scrollIntoView;
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [
          { id: "m1", conversationId: "c1", role: "user", content: "第一个问题", createdAt: "1" },
          { id: "m2", conversationId: "c1", role: "assistant", content: "第一个回答", createdAt: "2" },
          { id: "m3", conversationId: "c1", role: "user", content: "第二个问题", createdAt: "3" },
          { id: "m4", conversationId: "c1", role: "assistant", content: "第二个回答", createdAt: "4" },
        ],
      },
    });

    const outline = wrapper.get('[aria-label="当前对话目录"]');
    expect(outline.findAll('[data-role="message-outline-item"]')).toHaveLength(2);
    expect(outline.text()).toContain("第一个问题");
    expect(outline.text()).toContain("第一个回答");
    expect(outline.text()).not.toContain("旅行者：");

    await outline.findAll('button')[1].trigger("click");
    expect(scrollIntoView).toHaveBeenCalledWith({ behavior: "smooth", block: "center" });
  });

  it("lets the user scroll upward without forcing the list back to the bottom", async () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [{ id: "m1", conversationId: "c1", role: "user", content: "历史内容", createdAt: "1" }],
      },
    });
    const list = wrapper.get('.message-list').element as HTMLElement;
    await nextTick();
    Object.defineProperties(list, {
      scrollHeight: { value: 1000, configurable: true },
      clientHeight: { value: 400, configurable: true },
    });
    list.scrollTop = 100;

    await wrapper.get('.message-list').trigger("scroll");

    expect(list.scrollTop).toBe(100);
  });

  it("renders steps collapsed inside the matching assistant reply", async () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [
          { id: "u1", conversationId: "c1", role: "user", content: "查一下", createdAt: "1" },
          { id: "a1", conversationId: "c1", role: "assistant", runId: "run-1", content: "结果", createdAt: "2" },
        ],
        stepsByRun: {
          "run-1": [{ id: "s1", runId: "run-1", kind: "source", title: "网页详情", status: "completed", timestamp: "2", url: "https://example.com", platform: "网页" }],
        },
      },
    });

    const steps = wrapper.get('[data-role="conversation-steps"]');
    expect((steps.element as HTMLDetailsElement).open).toBe(false);
    expect(steps.text()).toContain("查询步骤");
    await steps.get("summary").trigger("click");
    expect(steps.text()).toContain("网页详情");
  });

  it("renders assistant text and each tool step in chronological order", () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [{
          id: "a1",
          conversationId: "c1",
          role: "assistant",
          content: "第一段\n\n第二段\n\n第三段",
          createdAt: "1",
          segments: [
            { id: "text-1", kind: "text", content: "第一段" },
            { id: "tool-1", kind: "tool", content: "", toolName: "搜索网页", status: "completed" },
            { id: "tool-1b", kind: "tool", content: "", toolName: "筛选结果", status: "completed" },
            { id: "text-2", kind: "text", content: "第二段" },
            { id: "tool-2", kind: "tool", content: "", toolName: "读取详情", status: "completed" },
            { id: "text-3", kind: "text", content: "第三段" },
          ],
        }],
      },
    });

    const body = wrapper.get(".message-body").element;
    const timeline = Array.from(body.querySelectorAll(":scope > .markdown-content, :scope > .conversation-tool-batch"));
    expect(timeline.map((item) => item.textContent?.trim())).toEqual([
      "第一段",
      "已执行筛选结果2 个工具搜索网页已完成筛选结果已完成",
      "第二段",
      "已执行读取详情1 个工具读取详情已完成",
      "第三段",
    ]);
    expect(body.querySelectorAll(':scope > [data-role="conversation-tool-batch"]')).toHaveLength(2);
    expect(body.querySelectorAll('[data-role="conversation-tool-batch"] [data-role="conversation-tool"]')).toHaveLength(3);
  });

  it("uses the traveler avatar for user messages and the app icon for Paimon", () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [
          { id: "u1", conversationId: "c1", role: "user", content: "问题", createdAt: "1" },
          { id: "a1", conversationId: "c1", role: "assistant", content: "回答", createdAt: "2" },
        ],
      },
    });
    const avatars = wrapper.findAll(".message-avatar img");
    expect(avatars[0].attributes("src")).toContain("traveler-dialogue-avatar");
    expect(avatars[1].attributes("src")).toContain("paimon-dialogue-avatar");
  });

  it("places traveler messages on the right and Paimon messages on the left", () => {
    const wrapper = mount(MessageList, {
      props: {
        streaming: false,
        messages: [
          { id: "u1", conversationId: "c1", role: "user", content: "问题", createdAt: "1" },
          { id: "a1", conversationId: "c1", role: "assistant", content: "回答", createdAt: "2" },
        ],
      },
    });

    const messages = wrapper.findAll(".chat-message");
    expect(messages[0].classes()).toContain("is-user");
    expect(messages[1].classes()).toContain("is-assistant");
    expect(messages[0].find(".message-avatar").classes()).toContain("is-traveler");
    expect(messages[1].find(".message-avatar").classes()).toContain("is-paimon");
  });
});
