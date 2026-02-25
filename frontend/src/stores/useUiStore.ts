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

  setActiveFolder: (folder: string) => void;
  selectMessage: (uid: number | null) => void;
  setSidebarWidth: (width: number) => void;
  setMessageListWidth: (width: number) => void;
  setReadingPaneVisible: (visible: boolean) => void;
  setDensity: (density: "compact" | "comfortable") => void;
}

const initial = loadSettings();

export const useUiStore = create<UiState>((set) => ({
  activeFolder: "INBOX",
  selectedMessageUid: null,
  sidebarWidth: initial.sidebarWidth,
  messageListWidth: initial.messageListWidth,
  readingPaneVisible: true,
  density: "comfortable",

  setActiveFolder: (folder) =>
    set({ activeFolder: folder, selectedMessageUid: null }),
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
}));
