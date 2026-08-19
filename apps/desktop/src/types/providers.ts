export interface ProviderConfig {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  secretRef: string;
  timeoutMs: number;
  temperature: number;
  maxTokens: number;
  streaming: boolean;
  toolCalling: boolean;
  enabled: boolean;
}

export interface ProviderView {
  config: ProviderConfig;
  hasApiKey: boolean;
}

export interface SaveProviderInput {
  config: ProviderConfig;
  apiKey?: string;
}

export interface CapabilityStatus {
  supported: boolean;
  message: string;
}

export interface CapabilityReport {
  basicRequest: CapabilityStatus;
  streaming: CapabilityStatus;
  toolCalling: CapabilityStatus;
}
