import { invoke } from "@tauri-apps/api/core";

import type { BrowserBounds, BrowserTab, PageSnapshot } from "@/types/browser";

export const listBrowserTabs = () => invoke<BrowserTab[]>("list_browser_tabs");
export const openBrowserTab = (url: string, bounds?: BrowserBounds) =>
  invoke<BrowserTab>("open_browser_tab", { input: { url, bounds: bounds ?? null } });
export const closeBrowserTab = (id: string) => invoke<void>("close_browser_tab", { id });
export const selectBrowserTab = (id: string) => invoke<void>("select_browser_tab", { id });
export const setBrowserVisibility = (visible: boolean) =>
  invoke<void>("set_browser_visibility", { visible });
export const setBrowserBounds = (bounds: BrowserBounds) =>
  invoke<void>("set_browser_bounds", { bounds });
export const navigateBrowserTab = (id: string, url: string) =>
  invoke<BrowserTab>("navigate_browser_tab", { id, url });
export const reloadBrowserTab = (id: string) => invoke<void>("reload_browser_tab", { id });
export const browserHistory = (id: string, direction: "back" | "forward") =>
  invoke<void>("browser_history", { id, direction });
export const extractBrowserPage = (id: string) =>
  invoke<PageSnapshot>("extract_browser_page", { id });
export const scrollBrowserPage = (id: string, maxRounds = 6) =>
  invoke<PageSnapshot>("scroll_browser_page", { id, maxRounds });
export const clickBrowserTarget = (id: string, target: string) =>
  invoke<PageSnapshot>("click_browser_target", { id, target });

