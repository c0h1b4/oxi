import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { mockUiState, mockUseSearch } = vi.hoisted(() => ({
  mockUiState: {
    searchQuery: "from:alice",
    setSearchQuery: vi.fn(),
    setSearchActive: vi.fn(),
    setActiveFolder: vi.fn(),
    selectMessage: vi.fn(),
    activeFolder: "INBOX",
    selectedMessageUid: null as number | null,
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
  },
  mockUseSearch: vi.fn(),
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@/hooks/useSearch", () => ({
  useSearch: mockUseSearch,
}));

import { SearchResults } from "../SearchResults";

describe("SearchResults motion transitions", () => {
  it("animates results list mount/unmount in non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUseSearch.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        total_count: 1,
        results: [
          {
            uid: 10,
            folder: "INBOX",
            from_name: "Alice",
            from_address: "alice@example.com",
            subject: "Hello",
            snippet: "Snippet",
            date: "2026-03-14T00:00:00Z",
            flags: [],
            has_attachments: false,
          },
        ],
      },
    });

    const { rerender } = render(<SearchResults />);
    expect(screen.getByTestId("search-results-list-transition")).toBeTruthy();

    mockUseSearch.mockReturnValue({
      isLoading: false,
      isError: false,
      data: { total_count: 0, results: [] },
    });
    rerender(<SearchResults />);
    expect(screen.queryByTestId("search-results-list-transition")).toBeNull();
  });

  it("animates individual result items in non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUseSearch.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        total_count: 2,
        results: [
          {
            uid: 10,
            folder: "INBOX",
            from_name: "Alice",
            from_address: "alice@example.com",
            subject: "Hello",
            snippet: "Snippet",
            date: "2026-03-14T00:00:00Z",
            flags: [],
            has_attachments: false,
          },
          {
            uid: 11,
            folder: "INBOX",
            from_name: "Bob",
            from_address: "bob@example.com",
            subject: "World",
            snippet: "Snippet",
            date: "2026-03-14T00:00:00Z",
            flags: [],
            has_attachments: false,
          },
        ],
      },
    });

    render(<SearchResults />);

    const items = screen.getAllByTestId("search-results-item-transition");
    expect(items.length).toBe(2);
  });

  it("bypasses motion wrappers in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUseSearch.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        total_count: 1,
        results: [
          {
            uid: 10,
            folder: "INBOX",
            from_name: "Alice",
            from_address: "alice@example.com",
            subject: "Hello",
            snippet: "Snippet",
            date: "2026-03-14T00:00:00Z",
            flags: [],
            has_attachments: false,
          },
        ],
      },
    });

    render(<SearchResults />);

    expect(screen.queryByTestId("search-results-list-transition")).toBeNull();
    expect(screen.queryByTestId("search-results-item-transition")).toBeNull();
    expect(screen.getByText("Alice")).toBeTruthy();
  });
});
