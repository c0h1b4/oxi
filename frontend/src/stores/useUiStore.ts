"use client";

import { create } from "zustand";

interface UiState {
  activeFolder: string;
  selectedMessageUid: number | null;
  sidebarWidth: number;
  readingPaneVisible: boolean;
  density: "compact" | "comfortable";

  setActiveFolder: (folder: string) => void;
  selectMessage: (uid: number | null) => void;
  setSidebarWidth: (width: number) => void;
  setReadingPaneVisible: (visible: boolean) => void;
  setDensity: (density: "compact" | "comfortable") => void;
}

export const useUiStore = create<UiState>((set) => ({
  activeFolder: "INBOX",
  selectedMessageUid: null,
  sidebarWidth: 240,
  readingPaneVisible: true,
  density: "comfortable",

  setActiveFolder: (folder) =>
    set({ activeFolder: folder, selectedMessageUid: null }),
  selectMessage: (uid) => set({ selectedMessageUid: uid }),
  setSidebarWidth: (width) => set({ sidebarWidth: width }),
  setReadingPaneVisible: (visible) => set({ readingPaneVisible: visible }),
  setDensity: (density) => set({ density }),
}));
