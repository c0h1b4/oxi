import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { Children, isValidElement, type Key, type ReactNode } from "react";

vi.mock("framer-motion", async () => {
  const React = await import("react");
  const { useRef, useEffect, useState } = React;

  function getKeyedChildren(children: ReactNode) {
    return Children.toArray(children).filter(
      (child): child is React.ReactElement => isValidElement(child) && child.key != null,
    );
  }

  function AnimatePresence({ children }: { children: ReactNode }) {
    const prevChildrenByKeyRef = useRef(new Map<Key, React.ReactElement>());
    const prevKeysRef = useRef(new Set<Key>());
    const [displayChildren, setDisplayChildren] = useState<React.ReactElement[]>([]);

    useEffect(() => {
      const keyedChildren = getKeyedChildren(children);
      const currentKeys = new Set(keyedChildren.map((child) => child.key as Key));

      const exitingChildren = Array.from(prevKeysRef.current)
        .filter((key) => !currentKeys.has(key))
        .map((key) => prevChildrenByKeyRef.current.get(key))
        .filter((child): child is React.ReactElement => child != null);

      const nextChildren = [...keyedChildren, ...exitingChildren];
      setDisplayChildren(nextChildren);

      // Update refs for next render
      for (const child of keyedChildren) {
        prevChildrenByKeyRef.current.set(child.key as Key, child);
      }
      prevKeysRef.current = currentKeys;
    }, [children]);

    return <>{displayChildren}</>;
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
    searchSortOrder: "date_desc" as "date_desc" | "date_asc",
    setSearchSortOrder: vi.fn(),
    setSearchResultCount: vi.fn(),
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
    mockUiState.searchQuery = "from:alice";
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
    mockUiState.searchQuery = "from:alice";
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
    mockUiState.searchQuery = "from:alice";
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

  it("does not show empty-results copy for invalid committed search", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.searchQuery = "   ";
    mockUseSearch.mockReturnValue({
      isLoading: false,
      isError: false,
      data: { total_count: 0, results: [] },
    });

    render(<SearchResults />);

    expect(screen.queryByText("No results found")).toBeNull();
  });

  it("clears committed query when filter removal leaves invalid query", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.searchQuery = "a has:attachment";
    mockUiState.setSearchQuery.mockClear();
    mockUiState.setSearchActive.mockClear();
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
            has_attachments: true,
          },
        ],
      },
    });

    render(<SearchResults />);

    fireEvent.click(screen.getByRole("button", { name: /remove has filter/i }));

    expect(mockUiState.setSearchQuery).toHaveBeenCalledWith("");
    expect(mockUiState.setSearchActive).toHaveBeenCalledWith(false);
  });

  it("persists normalized query when filter removal leaves valid query", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.searchQuery = "  report   has:attachment  ";
    mockUiState.setSearchQuery.mockClear();
    mockUiState.setSearchActive.mockClear();
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
            has_attachments: true,
          },
        ],
      },
    });

    render(<SearchResults />);

    fireEvent.click(screen.getByRole("button", { name: /remove has filter/i }));

    expect(mockUiState.setSearchQuery).toHaveBeenCalledWith("report");
    expect(mockUiState.setSearchActive).toHaveBeenCalledWith(true);
  });

  describe("keyboard scroll-to-selected", () => {
    const originalGetBoundingClientRect = Element.prototype.getBoundingClientRect;

    beforeEach(() => {
      mockUiState.effectiveAnimationMode = "medium";
      mockUiState.searchQuery = "test";
      mockUiState.selectedMessageUid = null;
      mockUiState.activeFolder = "INBOX";
    });

    afterEach(() => {
      Element.prototype.getBoundingClientRect = originalGetBoundingClientRect;
    });

    it("scrolls selected row into view with buffer when selection changes", () => {
      const rowHeight = 50;
      const viewportHeight = 100;
      const buffer = rowHeight * 3;

      const results = Array.from({ length: 10 }, (_, i) => ({
        uid: i + 1,
        folder: "INBOX",
        from_name: `User ${i + 1}`,
        from_address: `user${i + 1}@test.com`,
        subject: `Subject ${i + 1}`,
        snippet: `Snippet ${i + 1}`,
        date: "2026-03-14T00:00:00Z",
        flags: [],
        has_attachments: false,
      }));

      mockUseSearch.mockReturnValue({
        isLoading: false,
        isError: false,
        data: { total_count: 10, results },
      });

      Element.prototype.getBoundingClientRect = function (
        this: HTMLElement,
      ): DOMRect {
        if (this.dataset?.testid === "search-results-list-transition") {
          return {
            top: 0,
            bottom: viewportHeight,
            left: 0,
            right: 200,
            width: 200,
            height: viewportHeight,
            x: 0,
            y: 0,
            toJSON: () => ({}),
          } as DOMRect;
        }
        const folder = this.dataset?.searchResultFolder;
        const uid = parseInt(this.dataset?.searchResultUid ?? "0", 10);
        if (folder === "INBOX" && uid > 0) {
          const top = (uid - 1) * rowHeight;
          return {
            top,
            bottom: top + rowHeight,
            left: 0,
            right: 200,
            width: 200,
            height: rowHeight,
            x: 0,
            y: top,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      };

      mockUiState.selectedMessageUid = 1;
      const { rerender } = render(<SearchResults />);

      const listEl = screen.getByTestId("search-results-list-transition");
      let scrollTopValue = 0;
      Object.defineProperty(listEl, "scrollTop", {
        get: () => scrollTopValue,
        set: (v) => {
          scrollTopValue = v;
        },
        configurable: true,
      });
      Object.defineProperty(listEl, "clientHeight", {
        value: viewportHeight,
        writable: false,
        configurable: true,
      });

      mockUiState.selectedMessageUid = 8;
      rerender(<SearchResults />);

      const row8Top = (8 - 1) * rowHeight;
      const row8Bottom = row8Top + rowHeight;
      const expectedScrollTop = row8Bottom - viewportHeight + buffer;

      expect(scrollTopValue).toBe(expectedScrollTop);
      expect(scrollTopValue).toBeGreaterThan(0);
    });

    it("does not scroll when same selection is re-selected", () => {
      const rowHeight = 50;
      const viewportHeight = 100;

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

      Element.prototype.getBoundingClientRect = function (
        this: HTMLElement,
      ): DOMRect {
        if (this.dataset?.testid === "search-results-list-transition") {
          return {
            top: 0,
            bottom: viewportHeight,
            left: 0,
            right: 200,
            width: 200,
            height: viewportHeight,
            x: 0,
            y: 0,
            toJSON: () => ({}),
          } as DOMRect;
        }
        const folder = this.dataset?.searchResultFolder;
        const uid = parseInt(this.dataset?.searchResultUid ?? "0", 10);
        if (folder === "INBOX" && uid === 10) {
          return {
            top: 0,
            bottom: rowHeight,
            left: 0,
            right: 200,
            width: 200,
            height: rowHeight,
            x: 0,
            y: 0,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      };

      mockUiState.selectedMessageUid = 10;
      const { rerender } = render(<SearchResults />);

      const listEl = screen.getByTestId("search-results-list-transition");
      let scrollTopValue = 42;
      let scrollSetCount = 0;
      Object.defineProperty(listEl, "scrollTop", {
        get: () => scrollTopValue,
        set: (v) => {
          scrollTopValue = v;
          scrollSetCount++;
        },
        configurable: true,
      });

      const initialScrollSetCount = scrollSetCount;
      const initialScrollTopValue = scrollTopValue;

      mockUiState.selectedMessageUid = 10;
      rerender(<SearchResults />);

      expect(scrollSetCount).toBe(initialScrollSetCount);
      expect(scrollTopValue).toBe(initialScrollTopValue);
    });

    it("scrolls when navigating between results with same uid in different folders", () => {
      const rowHeight = 50;
      const viewportHeight = 100;
      const results = [
        {
          uid: 10,
          folder: "INBOX",
          from_name: "Inbox Message",
          from_address: "inbox@test.com",
          subject: "Inbox Subject",
          snippet: "Inbox snippet",
          date: "2026-03-14T00:00:00Z",
          flags: [],
          has_attachments: false,
        },
        {
          uid: 10,
          folder: "Sent",
          from_name: "Sent Message",
          from_address: "sent@test.com",
          subject: "Sent Subject",
          snippet: "Sent snippet",
          date: "2026-03-14T00:00:00Z",
          flags: [],
          has_attachments: false,
        },
      ];

      mockUseSearch.mockReturnValue({
        isLoading: false,
        isError: false,
        data: { total_count: 2, results },
      });

      Element.prototype.getBoundingClientRect = function (
        this: HTMLElement,
      ): DOMRect {
        if (this.dataset?.testid === "search-results-list-transition") {
          return {
            top: 0,
            bottom: viewportHeight,
            left: 0,
            right: 200,
            width: 200,
            height: viewportHeight,
            x: 0,
            y: 0,
            toJSON: () => ({}),
          } as DOMRect;
        }
        const folder = this.dataset?.searchResultFolder;
        const uid = parseInt(this.dataset?.searchResultUid ?? "0", 10);
        if (uid === 10) {
          const idx = folder === "INBOX" ? 0 : 9;
          const top = idx * rowHeight;
          return {
            top,
            bottom: top + rowHeight,
            left: 0,
            right: 200,
            width: 200,
            height: rowHeight,
            x: 0,
            y: top,
            toJSON: () => ({}),
          } as DOMRect;
        }
        return originalGetBoundingClientRect.call(this);
      };

      mockUiState.selectedMessageUid = 10;
      mockUiState.activeFolder = "INBOX";
      const { rerender } = render(<SearchResults />);

      const listEl = screen.getByTestId("search-results-list-transition");
      let scrollTopValue = 0;
      let scrollSetCount = 0;
      Object.defineProperty(listEl, "scrollTop", {
        get: () => scrollTopValue,
        set: (v) => {
          scrollTopValue = v;
          scrollSetCount++;
        },
        configurable: true,
      });

      mockUiState.selectedMessageUid = 10;
      mockUiState.activeFolder = "Sent";
      rerender(<SearchResults />);

      expect(scrollSetCount).toBeGreaterThan(0);
      expect(scrollTopValue).toBeGreaterThan(0);
    });
  });
});
