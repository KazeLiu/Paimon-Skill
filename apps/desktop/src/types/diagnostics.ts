export interface LogEntry {
  id: string;
  createdAt: string;
  level: string;
  category: string;
  message: string;
  details: Record<string, unknown> | null;
}

export interface DiagnosticsSettings {
  verbose: boolean;
}
