export interface BrowserTab {
  id: string;
  title: string;
  url: string;
  loading: boolean;
  profileId: string;
}

export interface BrowserEvent {
  kind: "load_started" | "load_finished" | "title_changed" | "closed";
  tab: BrowserTab;
}

export interface PageLink {
  text: string;
  url: string;
}

export interface PageSnapshot {
  title: string;
  url: string;
  text: string;
  links: PageLink[];
  loginChallenge: boolean;
  challengeReason: string | null;
  truncated: boolean;
  structure?: Record<string, unknown> | null;
  snapshotGeneration?: number | null;
  pageFingerprint?: Record<string, unknown> | null;
  interactiveTotal?: number;
  shadowRootCount?: number;
}

export interface PageContextMatch {
  index: number;
  score: number;
  matchedIn: string[];
  match: Record<string, unknown>;
  relation: {
    type: "context";
    text: string;
    ancestors: Record<string, unknown>[];
  };
}

export interface PageContextSearchResult {
  url: string;
  title: string;
  query: string;
  matchCount: number;
  returnedMatches: number;
  matches: PageContextMatch[];
  truncated?: boolean;
  snapshotGeneration?: number;
  pageFingerprint?: Record<string, unknown>;
}
