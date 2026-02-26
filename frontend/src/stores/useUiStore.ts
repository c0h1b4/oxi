"use client";

import { create } from "zustand";

const STORAGE_KEY = "oxi-ui-settings";

interface PersistedSettings {
  sidebarWidth: number;
  messageListWidth: number;
}

function loadSettings(): PersistedSettings {
  if (typeof window === "undefined") {
    return { sidebarWidth: 200, messageListWidth: 420 };
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      return {
        sidebarWidth: parsed.sidebarWidth ?? 200,
        messageListWidth: parsed.messageListWidth ?? 420,
      };
    }
  } catch {
    // ignore
  }
  return { sidebarWidth: 200, messageListWidth: 420 };
}

function saveSettings(settings: Partial<PersistedSettings>) {
  try {
    const current = loadSettings();
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ ...current, ...settings }),
    );
  } catch {
    // ignore
  }
}

interface UiState {
  activeFolder: string;
  selectedMessageUid: number | null;
  sidebarWidth: number;
  messageListWidth: number;
  readingPaneVisible: boolean;
  density: "compact" | "comfortable";
  searchQuery: string;
  searchActive: boolean;
  viewMode: "mail" | "contacts";
  selectedMessageUids: number[];
  bulkSelectMode: boolean;

  setActiveFolder: (folder: string) => void;
  selectMessage: (uid: number | null) => void;
  setSidebarWidth: (width: number) => void;
  setMessageListWidth: (width: number) => void;
  setReadingPaneVisible: (visible: boolean) => void;
  setDensity: (density: "compact" | "comfortable") => void;
  setSearchQuery: (query: string) => void;
  setSearchActive: (active: boolean) => void;
  clearSearch: () => void;
  setViewMode: (mode: "mail" | "contacts") => void;
  toggleBulkSelect: (uid: number) => void;
  selectAllMessages: (uids: number[]) => void;
  clearBulkSelection: () => void;
  setBulkSelectMode: (mode: boolean) => void;
}

const initial = loadSettings();

export const useUiStore = create<UiState>((set) => ({
  activeFolder: "INBOX",
  selectedMessageUid: null,
  sidebarWidth: initial.sidebarWidth,
  messageListWidth: initial.messageListWidth,
  readingPaneVisible: true,
  density: "comfortable",
  searchQuery: "",
  searchActive: false,
  viewMode: "mail",
  selectedMessageUids: [],
  bulkSelectMode: false,

  setActiveFolder: (folder) =>
    set({ activeFolder: folder, selectedMessageUid: null, selectedMessageUids: [], bulkSelectMode: false }),
  selectMessage: (uid) => set({ selectedMessageUid: uid }),
  setSidebarWidth: (width) => {
    saveSettings({ sidebarWidth: width });
    set({ sidebarWidth: width });
  },
  setMessageListWidth: (width) => {
    saveSettings({ messageListWidth: width });
    set({ messageListWidth: width });
  },
  setReadingPaneVisible: (visible) => set({ readingPaneVisible: visible }),
  setDensity: (density) => set({ density }),
  setSearchQuery: (query) => set({ searchQuery: query }),
  setSearchActive: (active) => set({ searchActive: active }),
  clearSearch: () => set({ searchQuery: "", searchActive: false }),
  setViewMode: (mode) => set({ viewMode: mode }),
  toggleBulkSelect: (uid) =>
    set((state) => {
      const exists = state.selectedMessageUids.includes(uid);
      const next = exists
        ? state.selectedMessageUids.filter((id) => id !== uid)
        : [...state.selectedMessageUids, uid];
      return {
        selectedMessageUids: next,
        bulkSelectMode: next.length > 0 ? true : state.bulkSelectMode,
      };
    }),
  selectAllMessages: (uids) => set({ selectedMessageUids: uids, bulkSelectMode: true }),
  clearBulkSelection: () => set({ selectedMessageUids: [], bulkSelectMode: false }),
  setBulkSelectMode: (mode) =>
    set({ bulkSelectMode: mode, selectedMessageUids: mode ? [] : [] }),
}));
