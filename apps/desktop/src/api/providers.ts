import { invoke } from "@tauri-apps/api/core";

import type {
  CapabilityReport,
  ProviderView,
  SaveProviderInput,
} from "@/types/providers";

export const listProviders = () => invoke<ProviderView[]>("list_providers");

export const saveProvider = (input: SaveProviderInput) =>
  invoke<ProviderView>("save_provider", { input });

export const deleteProvider = (id: string) => invoke<void>("delete_provider", { id });

export const testProvider = (input: SaveProviderInput) =>
  invoke<CapabilityReport>("test_provider", { input });

export const listProviderModels = (input: SaveProviderInput) =>
  invoke<string[]>("list_provider_models", { input });
