import { invoke } from "@tauri-apps/api/core";
import type { SourceTestInput, SourceTestResult } from "@/types/sourceTests";

export const runSourceTest = (input: SourceTestInput) =>
  invoke<SourceTestResult>("run_source_test", { input });
