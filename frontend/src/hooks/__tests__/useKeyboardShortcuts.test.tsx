import { renderHook, act } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

const { mockUiState, mockSearchData } = vi.hoisted(() => ({
  mockUiState: {
    activeFolder: "INBOX",
    selectedMessageUid: null as number | null,
    searchActive: false,
    searchQuery: "",
    searchSortOrder: "date_desc" as const,
    selectMessage: vi.fn(),
    setActiveFolder: vi.fn(),
    setSearchActive: vi.fn(),
    clearSearch: vi.fn(),
    setShortcutsOpen: vi.fn(),
    setCommandPaletteOpen: vi.fn(),
    setKeyboardNav: vi.fn(),
  },
  mockSearchData: {
    results: [],
  },
}));

vi.mock("@/stores/useUiStore", () => {
  const useUiStore = (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState);
  useUiStore.getState = () => mockUiState;
  return { useUiStore };
});

vi.mock("@/hooks/useMessages", () => ({
  useUpdateFlags: () => ({ mutate: vi.fn() }),
  useMoveMessage: () => ({ mutate: vi.fn() }),
  useDeleteMessage: () => ({ mutate: vi.fn() }),
  useMessages: () => ({ data: { pages: [] } }),
}));

vi.mock("@/hooks/useSearch", () => ({
  useSearch: () => mockSearchData,
}));

import { useKeyboardShortcuts } from "../useKeyboardShortcuts";

describe("useKeyboardShortcuts", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    document.body.innerHTML = "";
    mockUiState.setSearchActive.mockClear();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it("focuses search without activating empty search", () => {
    const searchInput = document.createElement("input");
    searchInput.setAttribute("data-search-input", "");
    document.body.appendChild(searchInput);

    renderHook(() => useKeyboardShortcuts());

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true }));
      vi.runAllTimers();
    });

    expect(document.activeElement).toBe(searchInput);
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();
  });

  it("arrow keys blur search input and navigate, j/k allow typing", () => {
    const searchInput = document.createElement("input");
    searchInput.setAttribute("data-search-input", "");
    document.body.appendChild(searchInput);
    searchInput.focus();

    expect(document.activeElement).toBe(searchInput);

    mockUiState.searchActive = true;
    mockUiState.searchQuery = "world";

    renderHook(() => useKeyboardShortcuts());

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "j" }));
    });

    expect(document.activeElement).toBe(searchInput);

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "k" }));
    });

    expect(document.activeElement).toBe(searchInput);

    act(() => {
      document.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowDown" }));
    });

    expect(document.activeElement).not.toBe(searchInput);

    mockUiState.searchActive = false;
    mockUiState.searchQuery = "";
    document.body.removeChild(searchInput);
  });
});
