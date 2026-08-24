import { invoke } from "@tauri-apps/api/core";

import type { LogEntry } from "@/types/diagnostics";

export const listLogs = () => invoke<LogEntry[]>("list_logs");
export const clearLogs = () => invoke<void>("clear_logs");
export const exportLogs = () => invoke<string>("export_logs");
