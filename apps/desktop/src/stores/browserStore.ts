import { defineStore } from "pinia";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import {
  browserHistory,
  clickBrowserTarget,
  closeBrowserTab,
  extractBrowserPage,
  listBrowserTabs,
  navigateBrowserTab,
  openBrowserTab,
  reloadBrowserTab,
  scrollBrowserPage,
  selectBrowserTab,
} from "@/api/browser";
import type { BrowserEvent, BrowserTab, PageSnapshot } from "@/types/browser";

const errorMessage = (error: unknown) => error instanceof Error ? error.message : String(error);

export const useBrowserStore = defineStore("browser", {
  state: () => ({
    tabs: [] as BrowserTab[],
    activeTabId: null as string | null,
    loadingAction: false,
    error: "",
    lastSnapshot: null as PageSnapshot | null,
    unlisten: null as UnlistenFn | null,
  }),
  getters: {
    activeTab(state) {
      return state.tabs.find((tab) => tab.id === state.activeTabId) ?? null;
    },
  },
  actions: {
    async initialize() {
      this.tabs = await listBrowserTabs();
      this.activeTabId = this.activeTabId ?? this.tabs[0]?.id ?? null;
    },
    async bindEvents() {
      if (this.unlisten) return;
      this.unlisten = await listen<BrowserEvent>("browser-event", (event) => {
        if (event.payload.kind === "closed") {
          this.tabs = this.tabs.filter((tab) => tab.id !== event.payload.tab.id);
          if (this.activeTabId === event.payload.tab.id) {
            this.activeTabId = this.tabs[this.tabs.length - 1]?.id ?? null;
          }
          return;
        }
        const index = this.tabs.findIndex((tab) => tab.id === event.payload.tab.id);
        if (index >= 0) this.tabs[index] = event.payload.tab;
      });
    },
    async open(url: string) {
      this.error = "";
      try {
        const tab = await openBrowserTab(url);
        this.tabs.push(tab);
        this.activeTabId = tab.id;
        return tab;
      } catch (error) {
        this.error = errorMessage(error);
        throw error;
      }
    },
    async close(id: string) {
      await closeBrowserTab(id);
      const index = this.tabs.findIndex((tab) => tab.id === id);
      this.tabs = this.tabs.filter((tab) => tab.id !== id);
      if (this.activeTabId === id) {
        this.activeTabId = this.tabs[Math.max(0, index - 1)]?.id ?? null;
        if (this.activeTabId) await selectBrowserTab(this.activeTabId);
      }
    },
    async select(id: string) {
      await selectBrowserTab(id);
      this.activeTabId = id;
    },
    async navigate(url: string) {
      if (!this.activeTabId) return;
      const tab = await navigateBrowserTab(this.activeTabId, url);
      const index = this.tabs.findIndex((item) => item.id === tab.id);
      if (index >= 0) this.tabs[index] = tab;
    },
    async reload() {
      if (this.activeTabId) await reloadBrowserTab(this.activeTabId);
    },
    async history(direction: "back" | "forward") {
      if (this.activeTabId) await browserHistory(this.activeTabId, direction);
    },
    async extract(id?: string) {
      const tabId = id ?? this.activeTabId;
      if (!tabId) return null;
      this.loadingAction = true;
      this.error = "";
      try {
        this.lastSnapshot = await extractBrowserPage(tabId);
        return this.lastSnapshot;
      } catch (error) {
        this.error = errorMessage(error);
        throw error;
      } finally {
        this.loadingAction = false;
      }
    },
    async scroll(maxRounds = 6) {
      if (!this.activeTabId) return null;
      this.loadingAction = true;
      this.error = "";
      try {
        this.lastSnapshot = await scrollBrowserPage(this.activeTabId, maxRounds);
        return this.lastSnapshot;
      } catch (error) {
        this.error = errorMessage(error);
        throw error;
      } finally {
        this.loadingAction = false;
      }
    },
    async click(target: string) {
      if (!this.activeTabId) return null;
      this.loadingAction = true;
      this.error = "";
      try {
        this.lastSnapshot = await clickBrowserTarget(this.activeTabId, target);
        return this.lastSnapshot;
      } catch (error) {
        this.error = errorMessage(error);
        throw error;
      } finally {
        this.loadingAction = false;
      }
    },
  },
});
