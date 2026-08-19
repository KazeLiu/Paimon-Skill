export interface PromptFragment {
  id: string;
  title: string;
  description: string;
  content: string;
  enabledByDefault: boolean;
  order: number;
}

export interface PromptSettings {
  overrides: Record<string, string>;
  disabledResourceIds: string[];
  customSuffix: string;
  updatedAt: string | null;
}

export interface ComposedPrompt {
  text: string;
  resourceIds: string[];
}
