import { invoke } from "@tauri-apps/api/core";

import type {
  BrowserTab,
  PageContextSearchResult,
  PageSnapshot,
} from "@/types/browser";

export const listBrowserTabs = () => invoke<BrowserTab[]>("list_browser_tabs");
export const openBrowserTab = (url: string) =>
  invoke<BrowserTab>("open_browser_tab", { input: { url } });
export const closeBrowserTab = (id: string) => invoke<void>("close_browser_tab", { id });
export const selectBrowserTab = (id: string) => invoke<void>("select_browser_tab", { id });
export const navigateBrowserTab = (id: string, url: string) =>
  invoke<BrowserTab>("navigate_browser_tab", { id, url });
export const reloadBrowserTab = (id: string) => invoke<void>("reload_browser_tab", { id });
export const browserHistory = (id: string, direction: "back" | "forward") =>
  invoke<void>("browser_history", { id, direction });
export const extractBrowserPage = (id: string) =>
  invoke<PageSnapshot>("extract_browser_page", { id });
export const findBrowserContext = (id: string, query: string) =>
  invoke<PageContextSearchResult>("find_browser_context", { id, query });
export const clickBrowserElement = (id: string, elementId: string) =>
  invoke<PageSnapshot>("click_browser_element", { id, elementId });
export const scrollBrowserPage = (id: string, maxRounds = 6) =>
  invoke<PageSnapshot>("scroll_browser_page", { id, maxRounds });
export const clickBrowserTarget = (id: string, target: string) =>
  invoke<PageSnapshot>("click_browser_target", { id, target });
export const clearBrowserProfile = () => invoke<void>("clear_browser_profile");

export interface SourceConnectionResult {
  reachable: boolean;
  statusCode: number | null;
  message: string;
}

export const checkSourceConnection = (url: string) =>
  invoke<SourceConnectionResult>("check_source_connection", { url });

export interface SourceLoginResult {
  status: "logged-in" | "logged-out";
  message: string;
}

export const checkSourceLogin = (sourceId: string) =>
  invoke<SourceLoginResult>("check_source_login", { sourceId });
