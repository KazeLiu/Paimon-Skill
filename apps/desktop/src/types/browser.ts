export interface BrowserBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface BrowserTab {
  id: string;
  title: string;
  url: string;
  loading: boolean;
  profileId: string;
}

export interface BrowserEvent {
  kind: "load_started" | "load_finished" | "title_changed";
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
}
