export type SourceTestPlatform = "bilibili" | "xiaohongshu" | "zhihu";

export interface SourceTestInput {
  testId: string;
  platform: SourceTestPlatform;
  url: string;
}

export interface SourceTestArtifact {
  title: string;
  url: string;
  content: string;
  metadata: Record<string, string>;
}

export interface SourceTestStep {
  index: number;
  action: string;
  status: "running" | "returned" | "failed";
  elapsedMs: number;
  data: SourceTestArtifact | null;
  error: string | null;
}

export interface SourceTestResult {
  testId: string;
  platform: SourceTestPlatform;
  url: string;
  startedAt: string;
  elapsedMs: number;
  status: "complete" | "partial" | "failed";
  checks: { key: string; label: string; status: "returned" | "missing"; detail: string }[];
  metadata: Record<string, string>;
  images: string[];
  report: {
    title: string;
    body: SourceTestArtifact | null;
    comments: SourceTestArtifact | null;
    transcript: SourceTestArtifact | null;
    warnings: string[];
  } | null;
  steps: SourceTestStep[];
  warnings: string[];
  error: string | null;
}

export interface SourceTestProgress {
  testId: string;
  step: SourceTestStep;
}
