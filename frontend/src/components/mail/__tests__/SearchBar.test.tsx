import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const { mockUiState, mockUseSearch } = vi.hoisted(() => ({
  mockUiState: {
    searchQuery: "",
    searchActive: false,
    setSearchQuery: vi.fn(),
    setSearchActive: vi.fn(),
    clearSearch: vi.fn(),
  },
  mockUseSearch: vi.fn(),
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@/hooks/useSearch", () => ({
  useSearch: mockUseSearch,
}));

import { SearchBar } from "../SearchBar";

describe("SearchBar", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockUiState.searchQuery = "";
    mockUiState.searchActive = false;
    mockUiState.setSearchQuery.mockReset();
    mockUiState.setSearchActive.mockReset();
    mockUiState.clearSearch.mockReset();
    mockUseSearch.mockReturnValue({ data: null });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders with empty input initially", () => {
    render(<SearchBar />);

    expect(
      (screen.getByPlaceholderText("Search mail... (Ctrl+K)") as HTMLInputElement).value,
    ).toBe("");
    expect(mockUiState.setSearchQuery).not.toHaveBeenCalled();
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();
    expect(mockUiState.clearSearch).not.toHaveBeenCalled();
  });

  it("commits a valid query after 300ms debounce", () => {
    render(<SearchBar />);

    const input = screen.getByPlaceholderText("Search mail... (Ctrl+K)") as HTMLInputElement;

    fireEvent.change(input, {
      target: { value: "ab" },
    });

    expect(input.value).toBe("ab");
    expect(mockUiState.setSearchQuery).not.toHaveBeenCalled();
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300);
    });

    expect(input.value).toBe("ab");
    expect(mockUiState.setSearchQuery).toHaveBeenCalledWith("ab");
    expect(mockUiState.setSearchActive).toHaveBeenCalledWith(true);
    expect(mockUiState.clearSearch).not.toHaveBeenCalled();
  });

  it("clears committed search immediately when input is emptied", () => {
    render(<SearchBar />);

    const input = screen.getByPlaceholderText("Search mail... (Ctrl+K)");

    fireEvent.change(input, { target: { value: "ab" } });
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(mockUiState.setSearchQuery).toHaveBeenCalledWith("ab");
    expect(mockUiState.setSearchActive).toHaveBeenCalledWith(true);

    mockUiState.setSearchQuery.mockClear();
    mockUiState.setSearchActive.mockClear();

    fireEvent.change(input, { target: { value: "   " } });

    expect(mockUiState.clearSearch).toHaveBeenCalledTimes(1);
    expect(mockUiState.setSearchQuery).not.toHaveBeenCalled();
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(300);
    });

    expect(mockUiState.setSearchQuery).not.toHaveBeenCalled();
    expect(mockUiState.setSearchActive).not.toHaveBeenCalled();
  });

  it("clears when the last filter chip is removed", () => {
    // Initial state with a filter
    mockUiState.searchQuery = "from:alice@example.com";
    render(<SearchBar />);

    const input = screen.getByPlaceholderText("Search mail... (Ctrl+K)") as HTMLInputElement;
    expect(input.value).toBe("from:alice@example.com");

    const removeBtn = screen.getByLabelText("Remove from filter");
    fireEvent.click(removeBtn);

    expect(input.value).toBe("");
    expect(mockUiState.clearSearch).toHaveBeenCalled();
  });
});
