import { invoke } from "@tauri-apps/api/core";

import type { ComposedPrompt, PromptFragment, PromptSettings } from "@/types/prompts";

export const getPromptSettings = () => invoke<PromptSettings>("get_prompt_settings");

export const listPromptResources = () =>
  invoke<PromptFragment[]>("list_prompt_resources");

export const savePromptSettings = (settings: PromptSettings) =>
  invoke<ComposedPrompt>("save_prompt_settings", { settings });

export const resetPromptSettings = () =>
  invoke<PromptSettings>("reset_prompt_settings");
