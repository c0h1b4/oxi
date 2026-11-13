import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useDeleteMessage, useMessages, useMoveMessage, useMessage } from "../useMessages";
import { useMessageRemovals } from "../pending-message-removals";
import { apiDelete, apiGet, apiPatch, apiPost } from "@/lib/api";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn(), apiPatch: vi.fn(), apiPost: vi.fn(), apiDelete: vi.fn() }));
vi.mock("@/lib/ws-context", () => ({ useWsStatus: () => ({ status: "connected" }) }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const page = (folder: string, uids = [7, 8]) => ({
  messages: uids.map(uid => ({ uid, folder })), total_count: uids.length, page: 0, per_page: 50,
});
let client: QueryClient;
let previousAccount: string | null;
beforeEach(() => {
  vi.resetAllMocks();
  client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ activeFolder: "Trash", selectedMessageUid: 7 });
});
afterEach(() => {
  cleanup();
  client.clear();
  useAuthStore.setState({ activeAccountId: previousAccount });
  useUiStore.setState({ selectedMessageUid: null });
});
function setup() {
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(() => ({
    list: useMessages("Trash"),
    remove: useDeleteMessage({ confirmation: true }),
    move: useMoveMessage({ confirmation: true }),
    pending: useMessageRemovals({ includeUnconfirmed: true }),
    body: useMessage("Trash", 8),
  }), { wrapper });
}

it("keeps rows readable until confirmation and runs two writes while another body loads", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash", [7, 8, 9, 10])], pageParams: [0] });
  const a = deferred<unknown>(), b = deferred<unknown>(), c = deferred<unknown>(), body = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementationOnce(() => a.promise).mockImplementationOnce(() => c.promise);
  vi.mocked(apiPost).mockImplementation(() => b.promise);
  vi.mocked(apiGet).mockImplementation(path => path === "/messages/Trash/8" ? body.promise : Promise.resolve(page("Trash", [8, 9, 10])));
  const { result } = setup();
  act(() => {
    result.current.remove.mutate({ folder: "Trash", uid: 7 });
    result.current.move.mutate({ fromFolder: "Trash", toFolder: "Junk", uid: 8 });
    result.current.remove.mutate({ folder: "Trash", uid: 9 });
    result.current.move.mutate({ fromFolder: "Trash", toFolder: "Junk", uid: 7 });
  });
  await waitFor(() => expect(apiPost).toHaveBeenCalledOnce());
  expect(apiDelete).toHaveBeenCalledTimes(1);
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([7, 8, 9, 10]);
  expect(useUiStore.getState().selectedMessageUid).toBe(7);
  await act(async () => body.resolve({ uid: 8, folder: "Trash", text: "Body B" }));
  await waitFor(() => expect(result.current.body.data?.text).toBe("Body B"));
  expect(result.current.pending.map(m => m.uid)).toEqual([7, 8, 9]);
  act(() => useUiStore.setState({ selectedMessageUid: 10 }));
  await act(async () => a.resolve({}));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(2));
  expect(useUiStore.getState().selectedMessageUid).toBe(10);
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([8, 9, 10]);
  await act(async () => { b.resolve({}); c.resolve({}); });
  await waitFor(() => expect(result.current.pending).toEqual([]));
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([10]);
});

it("retains a failed row and does not steal selection from later navigation", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash")], pageParams: [0] });
  const deletion = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiGet).mockResolvedValue(page("Trash"));
  const { result } = setup();
  act(() => result.current.remove.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  act(() => useUiStore.setState({ selectedMessageUid: 8 }));
  await act(async () => deletion.reject(new Error("Server unavailable")));
  await waitFor(() => expect(result.current.remove.isError).toBe(true));
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([7, 8]);
  expect(result.current.pending).toEqual([]);
  expect(useUiStore.getState().selectedMessageUid).toBe(8);
});

it("advances only after confirmation when the removed email is still selected", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash")], pageParams: [0] });
  const deletion = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiGet).mockResolvedValue(page("Trash", [8]));
  const { result } = setup();
  act(() => result.current.remove.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  expect(useUiStore.getState().selectedMessageUid).toBe(7);
  await act(async () => deletion.resolve({}));
  await waitFor(() => expect(result.current.remove.isSuccess).toBe(true));
  expect(useUiStore.getState().selectedMessageUid).toBe(8);
});

it("cancels queued writes and ignores old confirmations after an account switch", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash", [7, 8, 9])], pageParams: [0] });
  const first = deferred<unknown>(), second = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
  vi.mocked(apiGet).mockResolvedValue(page("Trash"));
  const { result } = setup();
  act(() => {
    for (const uid of [7, 8, 9]) result.current.remove.mutate({ folder: "Trash", uid });
  });
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(2));
  act(() => {
    useAuthStore.setState({ activeAccountId: "account-b" });
    client.clear();
    client.setQueryData(["messages", "Trash"], { pages: [page("Trash", [7])], pageParams: [0] });
  });
  await act(async () => { first.resolve({}); second.resolve({}); });
  expect(apiDelete).toHaveBeenCalledTimes(2);
  expect(useUiStore.getState().selectedMessageUid).toBe(7);
  expect(client.getQueryData<{ pages: ReturnType<typeof page>[] }>(["messages", "Trash"])?.pages[0].messages.map(m => m.uid)).toEqual([7]);
});

it("aborts an obsolete body read when its observer unmounts", async () => {
  vi.mocked(apiGet).mockImplementation(() => new Promise(() => {}));
  const { unmount } = setup();
  await waitFor(() => expect(apiGet).toHaveBeenCalledWith("/messages/Trash/8", expect.any(AbortSignal)));
  const signal = vi.mocked(apiGet).mock.calls.find(([path]) => path === "/messages/Trash/8")![1]!;
  expect(signal.aborted).toBe(false);
  unmount();
  expect(signal.aborted).toBe(true);
});
