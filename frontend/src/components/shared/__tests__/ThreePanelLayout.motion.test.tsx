import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const { mockUiState } = vi.hoisted(() => ({
  mockUiState: {
    sidebarWidth: 200,
    messageListWidth: 420,
    selectedMessageUid: null as number | null,
    searchActive: false,
    searchQuery: "",
    readingPaneVisible: true,
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
    setSidebarWidth: vi.fn(),
    setMessageListWidth: vi.fn(),
  },
}));

vi.mock("@/stores/useUiStore", () => {
  const useUiStore = (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState);
  useUiStore.getState = () => mockUiState;
  return { useUiStore };
});

vi.mock("@/components/mail/SearchBar", () => ({
  SearchBar: vi.fn(() => <div data-testid="search-bar" />),
}));

vi.mock("@/components/mail/SearchResults", () => ({
  SearchResults: vi.fn(() => <div data-testid="search-results" />),
}));

vi.mock("@/components/mail/MessageActionBar", () => ({
  MessageActionBar: vi.fn(() => <div data-testid="message-action-bar" />),
}));

import { ThreePanelLayout } from "../ThreePanelLayout";

function renderLayout() {
  return render(
    <ThreePanelLayout
      navRail={<div data-testid="nav-rail" />}
      sidebar={<div data-testid="sidebar" />}
      messageList={<div data-testid="message-list" />}
      readingPane={<div data-testid="reading-pane" />}
    />,
  );
}

describe("ThreePanelLayout motion transitions", () => {
  it("animates reading pane enter path for non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = false;

    renderLayout();

    expect(screen.getByTestId("three-panel-reading-pane-transition")).toBeTruthy();
  });

  it("uses a concrete wrapper for reading pane motion", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = false;

    renderLayout();

    const readingPane = screen.getByTestId("three-panel-reading-pane-transition");

    expect(readingPane.className.includes("contents")).toBe(false);
    expect(readingPane.className.includes("flex")).toBe(true);
  });

  it("animates search/list swap for non-off modes", async () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = false;
    mockUiState.searchQuery = "";

    const { rerender } = renderLayout();
    expect(screen.getByTestId("three-panel-list-transition")).toBeTruthy();

    mockUiState.searchActive = true;
    mockUiState.searchQuery = "from:alice";
    rerender(
      <ThreePanelLayout
        navRail={<div data-testid="nav-rail" />}
        sidebar={<div data-testid="sidebar" />}
        messageList={<div data-testid="message-list" />}
        readingPane={<div data-testid="reading-pane" />}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("three-panel-search-transition")).toBeTruthy();
    });
  });

  it("renders static paths without motion wrappers in off mode", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = true;
    mockUiState.searchQuery = "from:alice";

    renderLayout();

    expect(screen.queryByTestId("three-panel-reading-pane-transition")).toBeNull();
    expect(screen.queryByTestId("three-panel-search-transition")).toBeNull();
    expect(screen.queryByTestId("three-panel-list-transition")).toBeNull();
    expect(screen.getByTestId("search-results")).toBeTruthy();
  });

  it("keeps message-list as fallback when committed search is invalid", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = true;
    mockUiState.searchQuery = "   ";

    renderLayout();

    expect(screen.getByTestId("three-panel-list-transition")).toBeTruthy();
    expect(screen.getByTestId("message-list")).toBeTruthy();
    expect(screen.queryByTestId("search-results")).toBeNull();
  });

  it("uses transform/opacity-only transition declarations", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = false;

    renderLayout();

    const readingPane = screen.getByTestId("three-panel-reading-pane-transition");
    const list = screen.getByTestId("three-panel-list-transition");

    const serialized = [
      readingPane.getAttribute("data-motion-props") ?? "",
      list.getAttribute("data-motion-props") ?? "",
    ].join(" ");

    for (const forbidden of ["width", "height", "top", "left"]) {
      expect(serialized.includes(forbidden)).toBe(false);
    }
  });

  it("animated search wrapper has flex layout for scroll containment", async () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.readingPaneVisible = true;
    mockUiState.searchActive = false;
    mockUiState.searchQuery = "";

    const { rerender } = renderLayout();
    expect(screen.getByTestId("three-panel-list-transition")).toBeTruthy();

    mockUiState.searchActive = true;
    mockUiState.searchQuery = "test";
    rerender(
      <ThreePanelLayout
        navRail={<div data-testid="nav-rail" />}
        sidebar={<div data-testid="sidebar" />}
        messageList={<div data-testid="message-list" />}
        readingPane={<div data-testid="reading-pane" />}
      />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("three-panel-search-transition")).toBeTruthy();
    });

    const searchWrapper = screen.getByTestId("three-panel-search-transition");
    const classList = searchWrapper.className;

    expect(classList).toContain("flex");
    expect(classList).toContain("min-h-0");
    expect(classList).toContain("flex-1");
    expect(classList).toContain("flex-col");
  });
});
