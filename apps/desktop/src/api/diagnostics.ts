import { invoke } from "@tauri-apps/api/core";

import type { DiagnosticsSettings, LogEntry } from "@/types/diagnostics";

export const listLogs = () => invoke<LogEntry[]>("list_logs");
export const clearLogs = () => invoke<void>("clear_logs");
export const exportLogs = (ids?: string[]) => invoke<string>("export_logs", { ids });
export const getDiagnosticsSettings = () => invoke<DiagnosticsSettings>("get_diagnostics_settings");
export const setDiagnosticsSettings = (settings: DiagnosticsSettings) =>
  invoke<DiagnosticsSettings>("set_diagnostics_settings", { settings });
