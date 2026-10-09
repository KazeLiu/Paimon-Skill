import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

import DiagnosticsSettingsPage from "./DiagnosticsSettingsPage.vue";
import { clearLogs, exportLogs, getDiagnosticsSettings, listLogs, setDiagnosticsSettings } from "@/api/diagnostics";

vi.mock("@/api/diagnostics", () => ({
  clearLogs: vi.fn(),
  exportLogs: vi.fn(),
  getDiagnosticsSettings: vi.fn(),
  listLogs: vi.fn(),
  setDiagnosticsSettings: vi.fn(),
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
    vi.mocked(getDiagnosticsSettings).mockResolvedValue({ verbose: false });
    vi.mocked(setDiagnosticsSettings).mockImplementation(async (settings) => settings);
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } });
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("loads sanitized logs and copies the AI-readable export", async () => {
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();

    expect(wrapper.text()).toContain("模型返回 HTTP 503");
    await wrapper.get('[data-action="copy-logs"]').trigger("click");
    expect(exportLogs).toHaveBeenCalledWith(["log-1"]);
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

  it("copies only the log entries selected for AI review", async () => {
    vi.mocked(listLogs).mockResolvedValue([
      { id: "log-1", createdAt: "2026-08-20T10:00:00Z", level: "error", category: "chat", message: "HTTP 503", details: null },
      { id: "log-2", createdAt: "2026-08-20T10:01:00Z", level: "info", category: "tool", message: "完成", details: null },
    ]);
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();

    await wrapper.get('[data-log-id="log-1"]').setValue(false);
    await wrapper.get('[data-action="copy-logs"]').trigger("click");

    expect(exportLogs).toHaveBeenCalledWith(["log-2"]);
  });

  it("selects all logs and clears the selection", async () => {
    vi.mocked(listLogs).mockResolvedValue([
      { id: "log-1", createdAt: "2026-08-20T10:00:00Z", level: "error", category: "chat", message: "失败", details: null },
      { id: "log-2", createdAt: "2026-08-20T10:01:00Z", level: "info", category: "tool", message: "完成", details: null },
    ]);
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();

    await wrapper.get('[data-action="clear-selection"]').trigger("click");
    expect(wrapper.get('[data-action="copy-logs"]').attributes("disabled")).toBeDefined();
    await wrapper.get('[data-action="select-all"]').trigger("click");
    expect(wrapper.get('[data-action="copy-logs"]').text()).toContain("2");
    expect(wrapper.get('[data-action="clear-selection"]').attributes("disabled")).toBeUndefined();
  });

  it("loads and persists the detailed log mode", async () => {
    vi.mocked(getDiagnosticsSettings).mockResolvedValue({ verbose: true });
    const wrapper = mount(DiagnosticsSettingsPage);
    await flushPromises();

    const toggle = wrapper.get('[data-action="toggle-verbose"]');
    expect((toggle.element as HTMLInputElement).checked).toBe(true);
    await toggle.setValue(false);
    expect(setDiagnosticsSettings).toHaveBeenCalledWith({ verbose: false });
  });
});
