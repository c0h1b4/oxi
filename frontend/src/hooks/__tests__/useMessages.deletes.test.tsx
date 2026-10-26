import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useDeleteMessage, useMessages, useMoveMessage, useUpdateFlags } from "../useMessages";
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
function setup(folder = "Trash") {
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(() => ({ list: useMessages(folder), remove: useDeleteMessage(), move: useMoveMessage(), flags: useUpdateFlags() }), { wrapper });
}

it.each(["Trash", "Junk"])("keeps a permanent delete in %s hidden through flags, resume refetch and final reconciliation", async folder => {
  useUiStore.setState({ activeFolder: folder });
  client.setQueryData(["messages", folder], { pages: [page(folder)], pageParams: [0] });
  const deletion = deferred<unknown>();
  const fresh = deferred<ReturnType<typeof page>>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiGet).mockResolvedValue(page(folder));
  vi.mocked(apiPatch).mockResolvedValue({});
  const { result } = setup(folder);
  act(() => result.current.remove.mutate({ folder, uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledWith(`/messages/${folder}/7`));
  expect(useUiStore.getState().selectedMessageUid).toBe(8);
  act(() => result.current.flags.mutate({ folder, uid: 8, flags: ["\\Seen"], add: true }));
  await waitFor(() => expect(result.current.flags.isSuccess).toBe(true));
  await act(async () => { await client.invalidateQueries({ queryKey: ["messages"] }); });
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([8]);
  expect(result.current.list.data?.pages[0].total_count).toBe(1);
  vi.mocked(apiGet).mockImplementation(() => fresh.promise);
  act(() => deletion.resolve({}));
  await waitFor(() => expect(result.current.list.isFetching).toBe(true));
  expect(result.current.remove.isPending).toBe(true);
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([8]);
  act(() => fresh.resolve(page(folder, [8])));
  await waitFor(() => expect(result.current.remove.isSuccess).toBe(true));
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([8]);
});

it("cancels a pre-delete list response and preserves success when reconciliation fails", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash")], pageParams: [0] });
  const deletion = deferred<unknown>();
  const stale = deferred<ReturnType<typeof page>>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  vi.mocked(apiGet).mockImplementationOnce(() => stale.promise).mockRejectedValue(new Error("Offline"));
  const { result } = setup();
  act(() => result.current.remove.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  await act(async () => { void client.invalidateQueries({ queryKey: ["messages", "Trash"] }); });
  await waitFor(() => expect(apiGet).toHaveBeenCalledOnce());
  act(() => deletion.resolve({}));
  await waitFor(() => expect(result.current.remove.isSuccess).toBe(true));
  await act(async () => stale.resolve(page("Trash")));
  expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([8]);
  expect(result.current.list.data?.pages[0].total_count).toBe(1);
});

it("rolls back only a failed deletion, without resurrecting a successful move or another pending delete", async () => {
  client.setQueryData(["messages", "Trash"], { pages: [page("Trash", [7, 8, 9])], pageParams: [0] });
  const failed = deferred<unknown>(), pending = deferred<unknown>(), move = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementationOnce(() => failed.promise).mockImplementationOnce(() => pending.promise);
  vi.mocked(apiPost).mockImplementation(() => move.promise);
  // Mount only mutations so rollback assertions inspect their own cache writes.
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result } = renderHook(() => ({ a: useDeleteMessage(), b: useDeleteMessage(), move: useMoveMessage() }), { wrapper });
  act(() => result.current.a.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(1));
  act(() => {
    result.current.move.mutate({ fromFolder: "Trash", toFolder: "INBOX", uid: 8 });
    result.current.b.mutate({ folder: "Trash", uid: 9 });
  });
  await waitFor(() => expect(apiPost).toHaveBeenCalledOnce());
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(2));
  act(() => move.resolve({}));
  await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
  act(() => failed.reject(new Error("Delete failed")));
  await waitFor(() => expect(result.current.a.isError).toBe(true));
  const restored = client.getQueryData<{ pages: ReturnType<typeof page>[] }>(["messages", "Trash"]);
  expect(restored?.pages[0].messages.map(m => m.uid)).toEqual([7]);
  expect(restored?.pages[0].total_count).toBe(1);
  act(() => pending.resolve({}));
  await waitFor(() => expect(result.current.b.isSuccess).toBe(true));
});

it("restores a failed delete and its selection, but never changes another account's rows", async () => {
  const initial = { pages: [page("Trash")], pageParams: [0] };
  client.setQueryData(["messages", "Trash"], initial);
  vi.mocked(apiGet).mockResolvedValue(page("Trash"));
  const deletion = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementation(() => deletion.promise);
  const { result } = setup();
  act(() => result.current.remove.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledOnce());
  act(() => deletion.reject(new Error("Delete failed")));
  await waitFor(() => expect(result.current.remove.isError).toBe(true));
  expect(result.current.list.data).toEqual(initial);
  expect(useUiStore.getState().selectedMessageUid).toBe(7);
  const oldAccountDelete = deferred<unknown>();
  vi.mocked(apiDelete).mockImplementation(() => oldAccountDelete.promise);
  act(() => result.current.remove.mutate({ folder: "Trash", uid: 7 }));
  await waitFor(() => expect(apiDelete).toHaveBeenCalledTimes(2));
  act(() => {
    useAuthStore.setState({ activeAccountId: "account-b" });
    client.clear();
    client.setQueryData(["messages", "Trash"], initial);
  });
  act(() => oldAccountDelete.reject(new Error("Old request failed")));
  await waitFor(() => expect(result.current.remove.isError).toBe(true));
  expect(client.getQueryData(["messages", "Trash"])).toEqual(initial);
});
