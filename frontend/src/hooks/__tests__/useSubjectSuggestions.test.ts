import { createElement, type ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useSubjectSuggestions } from "../useSubjectSuggestions";
import { useAuthStore } from "@/stores/useAuthStore";
import { apiGet } from "@/lib/api";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn() }));
beforeEach(() => {
  vi.useFakeTimers();
  useAuthStore.setState({ activeAccountId: "account-a" });
  vi.mocked(apiGet).mockResolvedValue({ suggestions: ["Project update"] });
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.clearAllMocks(); });

function setup(value = "") {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => createElement(QueryClientProvider, { client }, children);
  return renderHook(({ text, enabled }) => useSubjectSuggestions(text, enabled), {
    wrapper, initialProps: { text: value, enabled: true },
  });
}

it("debounces typed subjects, hides stale suggestions and ignores short queries", async () => {
  const { result, rerender } = setup();
  rerender({ text: "P", enabled: true });
  await act(async () => { await vi.advanceTimersByTimeAsync(200); });
  expect(apiGet).not.toHaveBeenCalled();
  rerender({ text: "Pro", enabled: true });
  await act(async () => { await vi.advanceTimersByTimeAsync(199); });
  expect(apiGet).not.toHaveBeenCalled();
  await act(async () => { await vi.advanceTimersByTimeAsync(10); });
  await act(async () => { await vi.advanceTimersByTimeAsync(10); });
  expect(apiGet).toHaveBeenCalledWith("/messages/subject-suggestions?q=Pro");
  expect(result.current).toEqual(["Project update"]);
  rerender({ text: "Other", enabled: true });
  expect(result.current).toEqual([]);
  rerender({ text: "Other", enabled: false });
  await act(async () => { await vi.advanceTimersByTimeAsync(200); });
  expect(apiGet).toHaveBeenCalledTimes(1);
});

it("keeps account caches separate and suppresses failed lookups", async () => {
  const { result } = setup("Pro");
  await act(async () => { await vi.advanceTimersByTimeAsync(10); });
  expect(result.current).toEqual(["Project update"]);
  vi.mocked(apiGet).mockRejectedValue(new Error("offline"));
  act(() => useAuthStore.setState({ activeAccountId: "account-b" }));
  expect(result.current).toEqual([]);
  await act(async () => { await vi.advanceTimersByTimeAsync(10); });
  expect(result.current).toEqual([]);
  expect(apiGet).toHaveBeenCalledTimes(2);
});
