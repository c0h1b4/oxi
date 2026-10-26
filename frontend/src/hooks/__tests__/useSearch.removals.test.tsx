import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useSearch } from "../useSearch";
import { useDeleteMessage, useMoveMessage } from "../useMessages";
import { useWebSocket } from "../useWebSocket";
import { hideRemovedSearchResults } from "../pending-message-removals";
import { apiDelete, apiGet, apiPost } from "@/lib/api";
import { useAuthStore } from "@/stores/useAuthStore";
import { useUiStore } from "@/stores/useUiStore";
import type { SearchResponse, SearchResultItem } from "@/types/message";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn(), apiPatch: vi.fn(), apiPost: vi.fn(), apiDelete: vi.fn() }));
vi.mock("@/lib/ws-context", () => ({ useWsStatus: () => ({ status: "connected" }) }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
class MockWebSocket {
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  close() {}
}
const key = (query = "test") => ["search", query, undefined, "date_desc", undefined];
const item = (uid: number, folder = "INBOX"): SearchResultItem => ({
  uid, folder, score: 1, subject: `${folder} ${uid}`, from_address: "a@example.com",
  from_name: "A", to_addresses: "[]", date: "2026-01-01", flags: "[]",
  has_attachments: false, snippet: "test",
});
const response = (results: SearchResultItem[], total_count = results.length): SearchResponse => ({ results, total_count, query: "test" });
let client: QueryClient;
let previousAccount: string | null;
beforeEach(() => {
  vi.resetAllMocks();
  vi.stubGlobal("WebSocket", MockWebSocket);
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity, refetchOnWindowFocus: false } } });
  previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ selectedMessageUid: null });
});
afterEach(() => {
  cleanup();
  client.clear();
  useAuthStore.setState({ activeAccountId: previousAccount });
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});
function setup() {
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(({ query }) => {
    useWebSocket();
    return { search: useSearch(query), move: useMoveMessage(), remove: useDeleteMessage(), other: useDeleteMessage() };
  }, { wrapper, initialProps: { query: "test" } });
}

it.each(["move", "delete"] as const)("overlays an unresolved %s during WebSocket recovery and search changes, with folder-local counts", async action => {
  const original = response([item(7), item(8), item(7, "Junk")], 10);
  client.setQueryData(key(), original);
  // Inactive searches need the same settlement cleanup.
  client.setQueryData(key("inactive"), original);
  const operation = deferred<unknown>();
  vi.mocked(apiPost).mockImplementation(() => operation.promise);
  vi.mocked(apiDelete).mockImplementation(() => operation.promise);
  vi.mocked(apiGet).mockResolvedValue(original);
  const { result, rerender } = setup();
  act(() => {
    if (action === "move") result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    else result.current.remove.mutate({ folder: "INBOX", uid: 7 });
  });
  await waitFor(() => expect(action === "move" ? apiPost : apiDelete).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.data).toEqual(response([item(8), item(7, "Junk")], 9)));
  // The hook, not a one-time raw cache edit, supplies the optimistic view.
  expect(client.getQueryData(key())).toEqual(original);
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(apiGet).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.isFetching).toBe(false));
  expect(result.current.search.data).toEqual(response([item(8), item(7, "Junk")], 9));
  expect(result.current[action === "move" ? "move" : "remove"].isPending).toBe(true);
  const changed = deferred<SearchResponse>();
  vi.mocked(apiGet).mockImplementationOnce(() => changed.promise);
  rerender({ query: "changed" });
  expect(result.current.search.isPlaceholderData).toBe(true);
  expect(result.current.search.data?.total_count).toBe(9);
  act(() => changed.resolve(original));
  await waitFor(() => expect(result.current.search.isPlaceholderData).toBe(false));
  expect(result.current.search.data).toEqual(response([item(8), item(7, "Junk")], 9));
  // Even a stale successful reconciliation response must not restore the row.
  act(() => operation.resolve({}));
  await waitFor(() => expect(result.current[action === "move" ? "move" : "remove"].isSuccess).toBe(true));
  expect(result.current.search.data).toEqual(response([item(8), item(7, "Junk")], 9));
  expect(client.getQueryData(key("changed"))).toEqual(response([item(8), item(7, "Junk")], 9));
  expect(client.getQueryData(key("inactive"))).toEqual(response([item(8), item(7, "Junk")], 9));
});

it.each(["move", "delete"] as const)("replaces an in-flight recovery fetch before settling a successful %s and preserves success on refetch failure", async action => {
  const original = response([item(7), item(7, "Junk")]);
  client.setQueryData(key(), original);
  const operation = deferred<unknown>(), stale = deferred<SearchResponse>(), fresh = deferred<SearchResponse>();
  vi.mocked(apiPost).mockImplementation(() => operation.promise);
  vi.mocked(apiDelete).mockImplementation(() => operation.promise);
  vi.mocked(apiGet).mockImplementationOnce(() => stale.promise).mockImplementationOnce(() => fresh.promise);
  const { result } = setup();
  act(() => {
    if (action === "move") result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    else result.current.remove.mutate({ folder: "INBOX", uid: 7 });
  });
  await waitFor(() => expect(action === "move" ? apiPost : apiDelete).toHaveBeenCalledOnce());
  act(() => window.dispatchEvent(new Event("pageshow")));
  await waitFor(() => expect(apiGet).toHaveBeenCalledOnce());
  act(() => operation.resolve({}));
  await waitFor(() => expect(apiGet).toHaveBeenCalledTimes(2));
  expect(result.current[action === "move" ? "move" : "remove"].isPending).toBe(true);
  act(() => stale.resolve(original));
  expect(result.current.search.data).toEqual(response([item(7, "Junk")]));
  act(() => fresh.reject(new Error("Offline")));
  await waitFor(() => expect(result.current[action === "move" ? "move" : "remove"].isSuccess).toBe(true));
  expect(client.isMutating({ mutationKey: ["move-message"] })).toBe(0);
  expect(client.getQueryData(key())).toEqual(response([item(7, "Junk")]));
  expect(result.current.search.data).toEqual(response([item(7, "Junk")]));
});

it.each(["move", "delete"] as const)("rolls back a failed %s from retained search data when recovery and reconciliation fail", async action => {
  const original = response([item(7), item(8)], 12);
  client.setQueryData(key(), original);
  const operation = deferred<unknown>();
  vi.mocked(apiPost).mockImplementation(() => operation.promise);
  vi.mocked(apiDelete).mockImplementation(() => operation.promise);
  vi.mocked(apiGet).mockRejectedValue(new Error("Offline"));
  const { result } = setup();
  act(() => {
    if (action === "move") result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    else result.current.remove.mutate({ folder: "INBOX", uid: 7 });
  });
  await waitFor(() => expect(action === "move" ? apiPost : apiDelete).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.data).toEqual(response([item(8)], 11)));
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(result.current.search.isRefetchError).toBe(true));
  act(() => operation.reject(new Error("Removal failed")));
  await waitFor(() => expect(result.current[action === "move" ? "move" : "remove"].isError).toBe(true));
  expect(result.current.search.data).toEqual(original);
  expect(client.getQueryData(key())).toEqual(original);
});

it("rolls back only a failed action while successful and unresolved siblings stay hidden across later recovery", async () => {
  const original = response([item(7), item(8), item(9), item(7, "Junk")], 10);
  client.setQueryData(key(), original);
  const failed = deferred<unknown>(), moved = deferred<unknown>(), pending = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementationOnce(() => failed.promise).mockImplementationOnce(() => pending.promise);
  vi.mocked(apiPost).mockImplementation(() => moved.promise);
  vi.mocked(apiGet).mockResolvedValue(original);
  const { result } = setup();
  act(() => {
    result.current.remove.mutate({ folder: "INBOX", uid: 7 });
    result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 });
    result.current.other.mutate({ folder: "INBOX", uid: 9 });
  });
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(apiPost).toHaveBeenCalledOnce());
  act(() => window.dispatchEvent(new Event("focus")));
  await waitFor(() => expect(apiGet).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.data).toEqual(response([item(7, "Junk")], 7)));
  act(() => moved.resolve({}));
  await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
  act(() => failed.reject(new Error("Delete failed")));
  await waitFor(() => expect(result.current.remove.isError).toBe(true));
  expect(result.current.other.isPending).toBe(true);
  expect(result.current.search.data).toEqual(response([item(7), item(7, "Junk")], 8));
  // A subsequent stale recovery cannot resurrect the already successful move.
  await act(async () => { await client.invalidateQueries({ queryKey: ["search"] }); });
  expect(result.current.search.data).toEqual(response([item(7), item(7, "Junk")], 8));
  vi.mocked(apiGet).mockRejectedValue(new Error("Offline"));
  act(() => pending.reject(new Error("Other delete failed")));
  await waitFor(() => expect(result.current.other.isError).toBe(true));
  expect(result.current.search.data).toEqual(response([item(7), item(9), item(7, "Junk")], 9));
  expect(client.getQueryData(key())).toEqual(response([item(7), item(9), item(7, "Junk")], 9));
});

it("counts overlapping actions on the same folder/UID once and does not roll back a successful sibling", async () => {
  const original = response([item(7), item(7, "Junk")], 20);
  client.setQueryData(key(), original);
  const deletion = deferred<unknown>(), move = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiPost).mockImplementation(() => move.promise);
  vi.mocked(apiGet).mockRejectedValue(new Error("Offline"));
  const { result } = setup();
  act(() => {
    result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    result.current.remove.mutate({ folder: "INBOX", uid: 7 });
  });
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.data).toEqual(response([item(7, "Junk")], 19)));
  act(() => move.resolve({}));
  await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
  act(() => deletion.reject(new Error("Already moved")));
  await waitFor(() => expect(result.current.remove.isError).toBe(true));
  expect(result.current.search.data).toEqual(response([item(7, "Junk")], 19));
});

it.each(["resolve", "reject"] as const)("isolates a pending old-account removal when it later %ss", async settlement => {
  const original = response([item(7)]);
  client.setQueryData(key(), original);
  const operation = deferred<unknown>();
  vi.mocked(apiPost).mockImplementation(() => operation.promise);
  vi.mocked(apiGet).mockResolvedValue(original);
  const { result } = setup();
  act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
  await waitFor(() => expect(apiPost).toHaveBeenCalledOnce());
  await waitFor(() => expect(result.current.search.data?.results).toEqual([]));
  const otherAccount = response([{ ...item(7), subject: "Account B" }]);
  // Even before clear(), the account subscription must stop hiding B's UID.
  act(() => {
    useAuthStore.setState({ activeAccountId: "account-b" });
    client.setQueryData(key(), otherAccount);
  });
  await waitFor(() => expect(result.current.search.data).toEqual(otherAccount));
  act(() => { client.clear(); client.setQueryData(key(), otherAccount); });
  const invalidate = vi.spyOn(client, "invalidateQueries");
  act(() => {
    if (settlement === "resolve") operation.resolve({});
    else operation.reject(new Error("Old account failed"));
  });
  await waitFor(() => expect(result.current.move.isPending).toBe(false));
  expect(client.getQueryData(key())).toEqual(otherAccount);
  expect(invalidate).not.toHaveBeenCalled();
});

it("keeps simultaneous move/delete settlements protected until their shared search refresh finishes", async () => {
  const original = response([item(7), item(8), item(7, "Junk")]);
  client.setQueryData(key(), original);
  const move = deferred<unknown>(), deletion = deferred<unknown>();
  const first = deferred<SearchResponse>(), replacement = deferred<SearchResponse>();
  vi.mocked(apiPost).mockImplementation(() => move.promise);
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiGet).mockImplementationOnce(() => first.promise).mockImplementationOnce(() => replacement.promise);
  const { result } = setup();
  act(() => {
    result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    result.current.remove.mutate({ folder: "INBOX", uid: 8 });
  });
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  act(() => { move.resolve({}); deletion.resolve({}); });
  await waitFor(() => expect(apiGet).toHaveBeenCalledTimes(2));
  expect(result.current.move.isPending && result.current.remove.isPending).toBe(true);
  act(() => first.resolve(original));
  expect(result.current.search.data).toEqual(response([item(7, "Junk")]));
  act(() => replacement.reject(new Error("Offline")));
  await waitFor(() => expect(result.current.move.isSuccess && result.current.remove.isSuccess).toBe(true));
  expect(result.current.search.data).toEqual(response([item(7, "Junk")]));
  expect(client.getQueryData(key())).toEqual(response([item(7, "Junk")]));
});

it.each([false, true])("does not prune another account after search reconciliation resumes (return to original: %s)", async switchBack => {
  const original = response([item(7)]);
  client.setQueryData(key(), original);
  const refresh = deferred<SearchResponse>();
  vi.mocked(apiPost).mockResolvedValue({});
  vi.mocked(apiGet).mockImplementation(() => refresh.promise);
  const { result, rerender } = setup();
  act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
  await waitFor(() => expect(apiGet).toHaveBeenCalledOnce());
  expect(result.current.move.isPending).toBe(true);
  const newData = response([{ ...item(7), subject: "New session" }]);
  act(() => {
    useAuthStore.setState({ activeAccountId: "account-b" }); client.clear();
    if (switchBack) { useAuthStore.setState({ activeAccountId: "account-a" }); client.clear(); }
    client.setQueryData(key("new session"), newData);
  });
  rerender({ query: "new session" });
  act(() => refresh.resolve(original));
  await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
  expect(result.current.search.data).toEqual(newData);
  expect(client.getQueryData(key("new session"))).toEqual(newData);
});

it("does not double-decrement already filtered or unrelated search results", () => {
  const data = response([item(7), item(7, "Junk"), item(8)], 1);
  const removals = [{ fromFolder: "INBOX", uid: 7 }, { fromFolder: "INBOX", uid: 7 }, { fromFolder: "INBOX", uid: 99 }];
  const hidden = hideRemovedSearchResults(data, removals);
  expect(hidden).toEqual(response([item(7, "Junk"), item(8)], 0));
  expect(hideRemovedSearchResults(hidden, removals)).toBe(hidden);
  expect(hideRemovedSearchResults(data, [{ fromFolder: "Trash", uid: 7 }])).toBe(data);
});
