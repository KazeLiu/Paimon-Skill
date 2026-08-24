export interface ProviderConfig {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  secretRef: string;
  timeoutMs: number;
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
  diagnostic: CapabilityDiagnostic | null;
}

export interface CapabilityDiagnostic {
  stage: string;
  endpoint: string;
  httpStatus: number | null;
  contentType: string | null;
  responseHeaders: string[];
  responseBody: string;
  parseError: string | null;
  truncated: boolean;
}

export interface CapabilityReport {
  basicRequest: CapabilityStatus;
}
