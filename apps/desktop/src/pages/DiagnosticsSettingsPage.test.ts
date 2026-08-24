import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import DiagnosticsSettingsPage from "./DiagnosticsSettingsPage.vue";
import { clearLogs, exportLogs, listLogs } from "@/api/diagnostics";

vi.mock("@/api/diagnostics", () => ({
  clearLogs: vi.fn(),
  exportLogs: vi.fn(),
  listLogs: vi.fn(),
}));

describe("DiagnosticsSettingsPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(listLogs).mockResolvedValue([
      {
        id: "log-1",
        createdAt: "2026-08-20T10:00:00Z",
        level: "error",
        category: "model",
        message: "模型返回 HTTP 503",
        details: { httpStatus: 503, authorization: "[REDACTED]" },
      },
    ]);
    vi.mocked(exportLogs).mockResolvedValue("# 派蒙助手诊断日志\n\nHTTP 503");
    vi.mocked(clearLogs).mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("loads sanitized logs and copies the AI-readable export", async () => {
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();

    expect(wrapper.text()).toContain("模型返回 HTTP 503");
    await wrapper.get('[data-action="copy-logs"]').trigger("click");
    expect(navigator.clipboard.writeText).toHaveBeenCalledWith("# 派蒙助手诊断日志\n\nHTTP 503");
    expect(wrapper.text()).toContain("已复制");
  });

  it("clears logs after explicit confirmation", async () => {
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();
    await wrapper.get('[data-action="clear-logs"]').trigger("click");
    expect(clearLogs).toHaveBeenCalled();
    expect(listLogs).toHaveBeenCalledTimes(2);
  });
});
