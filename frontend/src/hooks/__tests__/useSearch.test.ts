import { renderHook, waitFor } from "@testing-library/react";
import {
  QueryClient,
  QueryClientProvider,
} from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createElement, type ReactNode } from "react";

const { mockApiGet } = vi.hoisted(() => ({
  mockApiGet: vi.fn(),
}));

vi.mock("@/lib/api", () => ({
  apiGet: mockApiGet,
}));

import { useSearch } from "../useSearch";

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
      },
    },
  });

  return function Wrapper({ children }: { children: ReactNode }) {
    return createElement(
      QueryClientProvider,
      { client: queryClient },
      children,
    );
  };
}

describe("useSearch", () => {
  afterEach(() => {
    mockApiGet.mockReset();
  });

  it("normalizes committed query before building params", async () => {
    mockApiGet.mockResolvedValue({ total_count: 0, results: [] });

    renderHook(() => useSearch("   from:alice    report   "), {
      wrapper: createWrapper(),
    });

    await waitFor(() => {
      expect(mockApiGet).toHaveBeenCalledTimes(1);
    });

    expect(mockApiGet).toHaveBeenCalledWith(
      "/search?q=from%3Aalice+report&sort=date_desc",
    );
  });

  it("does not call apiGet for empty or whitespace queries", async () => {
    mockApiGet.mockResolvedValue({ total_count: 0, results: [] });

    const { rerender } = renderHook(
      ({ query }) => useSearch(query),
      {
        wrapper: createWrapper(),
        initialProps: { query: "" },
      },
    );

    rerender({ query: "     " });

    await waitFor(() => {
      expect(mockApiGet).not.toHaveBeenCalled();
    });
  });
});
