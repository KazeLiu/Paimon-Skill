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
});
