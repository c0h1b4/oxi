import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Children, isValidElement, useRef, type Key, type ReactNode } from "react";

vi.mock("framer-motion", async () => {
  const React = await import("react");

  function getKeyedChildren(children: ReactNode) {
    return Children.toArray(children).filter(
      (child): child is React.ReactElement => isValidElement(child) && child.key != null,
    );
  }

  function AnimatePresence({ children }: { children: ReactNode }) {
    const prevChildrenByKeyRef = useRef(new Map<Key, React.ReactElement>());
    const prevKeysRef = useRef(new Set<Key>());

    const keyedChildren = getKeyedChildren(children);
    const currentKeys = new Set(keyedChildren.map((child) => child.key as Key));

    const exitingChildren = Array.from(prevKeysRef.current)
      .filter((key) => !currentKeys.has(key))
      .map((key) => prevChildrenByKeyRef.current.get(key))
      .filter((child): child is React.ReactElement => child != null);

    for (const child of keyedChildren) {
      prevChildrenByKeyRef.current.set(child.key as Key, child);
    }
    prevKeysRef.current = currentKeys;

    return <>{[...keyedChildren, ...exitingChildren]}</>;
  }

  return {
    AnimatePresence,
    motion: {
      div: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
        <div {...props}>{children}</div>
      ),
    },
  };
});

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
  it("keeps the list mounted for exit when results become empty", () => {
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
    expect(screen.getByText("No results found")).toBeTruthy();
    expect(screen.queryByTestId("search-results-list-transition")).toBeTruthy();
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
