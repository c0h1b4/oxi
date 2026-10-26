import { it, expect, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider, useQuery, focusManager, onlineManager } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { hideMovedMessages, useMessages, useMoveMessage, useUpdateFlags } from "../useMessages";
import { apiGet, apiPost, apiPatch } from "@/lib/api";
import type { MessageHeader } from "@/types/message";
import { useUiStore } from "@/stores/useUiStore";
import { useAuthStore } from "@/stores/useAuthStore";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn(), apiPatch: vi.fn(), apiPost: vi.fn(), apiDelete: vi.fn() }));
vi.mock("@/lib/ws-context", () => ({ useWsStatus: () => ({ status: "connected" }) }));

it("keeps a pending Junk move hidden through a flags refresh and reconciliation", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const moved = { uid: 7, folder: "INBOX" };
  const next = { uid: 8, folder: "INBOX" };
  let serverMessages = [moved, next];
  const page = () => ({ messages: [...serverMessages], page: 0, per_page: 50, total_count: serverMessages.length });
  client.setQueryData(["messages", "INBOX"], { pages: [page()], pageParams: [0] });
  vi.mocked(apiGet).mockImplementation(async () => page());
  vi.mocked(apiPatch).mockResolvedValue({ status: "ok" });
  let finish!: () => void;
  vi.mocked(apiPost).mockImplementation(() => new Promise(resolve => {
    finish = () => { serverMessages = [next]; resolve({ status: "ok" }); };
  }));
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), move: useMoveMessage(), flags: useUpdateFlags() }), { wrapper });
  const uids = () => result.current.list.data?.pages.flatMap(p => p.messages.map(m => m.uid));
  try {
    act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(finish).toBeDefined());
    await waitFor(() => expect(uids()).toEqual([8]));
    // Auto-advance opens the next unread message; ReadingPane marks it read.
    act(() => result.current.flags.mutate({ folder: "INBOX", uid: 8, flags: ["\\Seen"], add: true }));
    await waitFor(() => expect(result.current.flags.isSuccess).toBe(true));
    await waitFor(() => expect(result.current.list.isFetching).toBe(false));
    expect(uids()).toEqual([8]);
    expect(result.current.move.isPending).toBe(true);
    act(() => finish());
    await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
    await waitFor(() => expect(uids()).toEqual([8]));
  } finally {
    unmount();
    client.clear();
  }
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

it("replaces a stale in-flight refresh and keeps the overlay until the fresh response arrives", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const source = { messages: [{ uid: 7, folder: "INBOX" }], page: 0, per_page: 50, total_count: 1 };
  client.setQueryData(["messages", "INBOX"], { pages: [source], pageParams: [0] });
  const move = deferred<unknown>();
  const stale = deferred<typeof source>();
  const fresh = deferred<typeof source>();
  vi.mocked(apiPost).mockImplementation(() => move.promise);
  vi.mocked(apiGet).mockReset().mockImplementationOnce(() => stale.promise).mockImplementationOnce(() => fresh.promise);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), move: useMoveMessage() }), { wrapper });
  try {
    act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(result.current.list.data?.pages[0].messages).toEqual([]));
    await act(async () => { void client.invalidateQueries({ queryKey: ["messages", "INBOX"] }); });
    await waitFor(() => expect(apiGet).toHaveBeenCalledTimes(1));
    act(() => move.resolve({ status: "ok" }));
    await waitFor(() => expect(apiGet).toHaveBeenCalledTimes(2));
    await act(async () => stale.resolve(source));
    expect(result.current.move.isPending).toBe(true);
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
    act(() => fresh.resolve({ ...source, messages: [], total_count: 0 }));
    await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
  } finally { unmount(); client.clear(); }
});

it("protects sibling moves across hook instances and invalidates search after simultaneous settlement", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  let messages = [{ uid: 7, folder: "INBOX" }, { uid: 8, folder: "INBOX" }];
  const page = () => ({ messages: [...messages], page: 0, per_page: 50, total_count: messages.length });
  client.setQueryData(["messages", "INBOX"], { pages: [page()], pageParams: [0] });
  client.setQueryData(["messages", "Junk"], { pages: [{ ...page(), messages: [{ uid: 7, folder: "Junk" }], total_count: 1 }], pageParams: [0] });
  const a = deferred<unknown>(), b = deferred<unknown>();
  vi.mocked(apiPost).mockReset().mockImplementationOnce(() => a.promise).mockImplementationOnce(() => b.promise);
  vi.mocked(apiGet).mockImplementation(async () => page());
  const invalidations = vi.spyOn(client, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), junk: useMessages("Junk"), a: useMoveMessage(), b: useMoveMessage() }), { wrapper });
  try {
    act(() => { result.current.a.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }); result.current.b.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 }); });
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(2));
    await act(async () => { await client.invalidateQueries({ queryKey: ["messages", "INBOX"] }); });
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
    expect(result.current.list.data?.pages[0].total_count).toBe(0);
    expect(result.current.junk.data?.pages[0].messages).toEqual([{ uid: 7, folder: "Junk" }]);
    messages = [];
    act(() => { a.resolve({}); b.resolve({}); });
    await waitFor(() => expect(result.current.a.isSuccess && result.current.b.isSuccess).toBe(true));
    await waitFor(() => expect(invalidations).toHaveBeenCalledWith({ queryKey: ["search"] }));
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
  } finally { unmount(); client.clear(); }
});

it("rolls back only the failed row without resurrecting a successful sibling or replacing destination changes", async () => {
  const client = new QueryClient();
  const source = { pages: [{ messages: [{ uid: 7, folder: "INBOX" }, { uid: 8, folder: "INBOX" }], page: 0, per_page: 50, total_count: 2 }], pageParams: [0] };
  client.setQueryData(["messages", "INBOX"], source);
  const a = deferred<unknown>(), b = deferred<unknown>();
  vi.mocked(apiPost).mockReset().mockImplementationOnce(() => a.promise).mockImplementationOnce(() => b.promise);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ a: useMoveMessage(), b: useMoveMessage() }), { wrapper });
  try {
    // The failed move's snapshot predates the successful move.
    act(() => result.current.a.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(1));
    act(() => result.current.b.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 }));
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(2));
    act(() => b.resolve({}));
    await waitFor(() => expect(result.current.b.isSuccess).toBe(true));
    const destination = { pages: [{ ...source.pages[0], messages: [{ uid: 99, folder: "Junk" }], total_count: 1 }], pageParams: [0] };
    client.setQueryData(["messages", "Junk"], destination);
    act(() => a.reject(new Error("Move failed")));
    await waitFor(() => expect(result.current.a.isError).toBe(true));
    expect(client.getQueryData(["messages", "INBOX"])).toEqual({ ...source, pages: [{ ...source.pages[0], messages: [{ uid: 7, folder: "INBOX" }], total_count: 1 }] });
    expect(client.getQueryData(["messages", "Junk"])).toEqual(destination);
  } finally { unmount(); client.clear(); }
});

it("counts distinct filtered UIDs across pages without double-decrementing optimistic data", () => {
  const message = (uid: number, folder = "INBOX") => ({ uid, folder } as MessageHeader);
  const data = { pages: [
    { messages: [message(7), message(9)], total_count: 4, page: 0, per_page: 2 },
    { messages: [message(8), message(10)], total_count: 4, page: 1, per_page: 2 },
  ], pageParams: [0, 1] };
  const filtered = hideMovedMessages(data, "INBOX", new Set([7, 8, 99]));
  expect(filtered.pages.map(page => page.total_count)).toEqual([2, 2]);
  expect(filtered.pages.flatMap(page => page.messages.map(item => item.uid))).toEqual([9, 10]);
  expect(filtered.pageParams).toBe(data.pageParams);
  expect(hideMovedMessages(filtered, "INBOX", new Set([7, 8]))).toBe(filtered);
  expect(hideMovedMessages(data, "Junk", new Set([7, 8]))).toBe(data);
});

it("does not resurrect a successful move when final reconciliation fails", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const source = { messages: [{ uid: 7, folder: "INBOX" }], page: 0, per_page: 50, total_count: 1 };
  client.setQueryData(["messages", "INBOX"], { pages: [source], pageParams: [0] });
  const move = deferred<unknown>();
  vi.mocked(apiPost).mockImplementation(() => move.promise);
  vi.mocked(apiGet).mockReset().mockResolvedValueOnce(source).mockRejectedValue(new Error("Offline"));
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), move: useMoveMessage() }), { wrapper });
  try {
    act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(result.current.list.data?.pages[0].messages).toEqual([]));
    await act(async () => { await client.invalidateQueries({ queryKey: ["messages", "INBOX"] }); });
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
    act(() => move.resolve({}));
    await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
    expect(client.isMutating({ mutationKey: ["move-message"] })).toBe(0);
  } finally { unmount(); client.clear(); }
});

it("keeps a slower sibling hidden after the first move reconciles", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  let messages = [{ uid: 7, folder: "INBOX" }, { uid: 8, folder: "INBOX" }];
  const page = () => ({ messages: [...messages], total_count: messages.length, page: 0, per_page: 50 });
  client.setQueryData(["messages", "INBOX"], { pages: [page()], pageParams: [0] });
  const a = deferred<unknown>(), b = deferred<unknown>();
  vi.mocked(apiPost).mockReset().mockImplementationOnce(() => a.promise).mockImplementationOnce(() => b.promise);
  vi.mocked(apiGet).mockImplementation(async () => page());
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), a: useMoveMessage(), b: useMoveMessage() }), { wrapper });
  try {
    act(() => { result.current.a.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }); result.current.b.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 }); });
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(2));
    messages = [{ uid: 8, folder: "INBOX" }];
    act(() => a.resolve({}));
    await waitFor(() => expect(result.current.a.isSuccess).toBe(true));
    expect(result.current.b.isPending).toBe(true);
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
    messages = [];
    act(() => b.resolve({}));
    await waitFor(() => expect(result.current.b.isSuccess).toBe(true));
    expect(result.current.list.data?.pages[0].messages).toEqual([]);
  } finally { unmount(); client.clear(); }
});

it("queues rapid Junk clicks locally, deduplicates, and continues FIFO after a failure", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  let messages = [{ uid: 7, folder: "INBOX" }, { uid: 8, folder: "INBOX" }, { uid: 9, folder: "INBOX" }];
  const page = () => ({ messages: [...messages], total_count: messages.length, page: 0, per_page: 50 });
  client.setQueryData(["messages", "INBOX"], { pages: [page()], pageParams: [0] });
  const a = deferred<unknown>(), b = deferred<unknown>();
  vi.mocked(apiPost).mockReset().mockImplementationOnce(() => a.promise).mockImplementationOnce(() => b.promise);
  vi.mocked(apiGet).mockImplementation(async () => page());
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({ list: useMessages("INBOX"), queue: useMoveMessage({ queued: true }) }), { wrapper });
  try {
    useUiStore.setState({ activeFolder: "INBOX", selectedMessageUid: 7 });
    act(() => {
      result.current.queue.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
      result.current.queue.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
    });
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(1));
    expect(useUiStore.getState().selectedMessageUid).toBe(8);
    act(() => result.current.queue.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 }));
    await waitFor(() => expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([9]));
    expect(useUiStore.getState().selectedMessageUid).toBe(9);
    expect(apiPost).toHaveBeenCalledTimes(1);
    expect(client.isMutating({ mutationKey: ["move-message"] })).toBe(2);
    act(() => a.reject(new Error("First move failed")));
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(result.current.list.data?.pages[0].messages.map(m => m.uid)).toEqual([7, 9]));
    expect(useUiStore.getState().selectedMessageUid).toBe(9);
    messages = messages.filter(m => m.uid !== 8);
    act(() => b.resolve({}));
    await waitFor(() => expect(client.isMutating({ mutationKey: ["move-message"] })).toBe(0));
    expect(vi.mocked(apiPost).mock.calls.map(call => call[1])).toEqual([
      { from_folder: "INBOX", to_folder: "Junk", uid: 7 },
      { from_folder: "INBOX", to_folder: "Junk", uid: 8 },
    ]);
  } finally { unmount(); client.clear(); useUiStore.setState({ selectedMessageUid: null }); }
});

it("does not remove a new account's same UID when an old move finishes reconciliation", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
  const refresh = deferred<unknown>();
  const page = (subject: string) => ({ pages: [{ messages: [{ uid: 7, folder: "INBOX", subject }], page: 0, per_page: 50, total_count: 1 }], pageParams: [0] });
  const previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ activeFolder: "INBOX", selectedMessageUid: null });
  client.setQueryData(["messages", "INBOX"], page("Account A"));
  client.setQueryData(["folders"], {});
  vi.mocked(apiPost).mockReset().mockResolvedValue({});
  vi.mocked(apiGet).mockReset().mockImplementation(() => refresh.promise);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => ({
    move: useMoveMessage(),
    folders: useQuery({ queryKey: ["folders"], queryFn: () => apiGet("/folders") }),
  }), { wrapper });
  try {
    act(() => result.current.move.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(apiGet).toHaveBeenCalledTimes(1));
    act(() => {
      useAuthStore.setState({ activeAccountId: "account-b" });
      client.clear();
      client.setQueryData(["messages", "INBOX"], page("Account B"));
    });
    await waitFor(() => expect(result.current.move.isSuccess).toBe(true));
    expect(client.getQueryData(["messages", "INBOX"])).toEqual(page("Account B"));
  } finally {
    unmount(); client.clear(); refresh.resolve({});
    useAuthStore.setState({ activeAccountId: previousAccount });
  }
});

it.each([false, true])("rejects cleared queued promises even offline/unfocused (%s)", async (offline) => {
  const client = new QueryClient();
  const first = deferred<unknown>();
  const previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ selectedMessageUid: null });
  vi.mocked(apiPost).mockReset().mockImplementationOnce(() => first.promise).mockResolvedValue({});
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => useMoveMessage({ queued: true }), { wrapper });
  let firstResult!: Promise<unknown>;
  let queuedResult!: Promise<unknown>;
  try {
    act(() => {
      firstResult = result.current.mutateAsync({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
      queuedResult = result.current.mutateAsync({ fromFolder: "INBOX", toFolder: "Junk", uid: 8 });
    });
    const queuedSettled = queuedResult.then(() => "unexpected success", error => error.message);
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(1));
    const queuedMutation = client.getMutationCache().getAll().find(move => (move.state.variables as { uid: number }).uid === 8)!;
    if (offline) { focusManager.setFocused(false); onlineManager.setOnline(false); }
    act(() => { useAuthStore.setState({ activeAccountId: "account-b" }); client.clear(); });
    expect(await queuedSettled).toBe("Move cancelled because the account changed");
    expect(queuedMutation.state.status).toBe("error");
    expect(apiPost).toHaveBeenCalledTimes(1);
    // A new account can use the queue immediately, without the old requests.
    await act(async () => { await result.current.mutateAsync({ fromFolder: "INBOX", toFolder: "Junk", uid: 9 }); });
    expect(apiPost).toHaveBeenCalledTimes(2);
    await act(async () => { first.resolve({}); await firstResult; });
    expect(apiPost).toHaveBeenCalledTimes(2);
  } finally {
    focusManager.setFocused(true); onlineManager.setOnline(true);
    first.resolve({}); unmount(); client.clear();
    useAuthStore.setState({ activeAccountId: previousAccount });
  }
});

it.each([false, true])("does not roll back old rows after switching accounts (return to original: %s)", async (switchBack) => {
  const client = new QueryClient();
  const move = deferred<unknown>();
  const previousAccount = useAuthStore.getState().activeAccountId;
  const source = { pages: [{ messages: [{ uid: 7, folder: "INBOX" }, { uid: 8, folder: "INBOX" }], page: 0, per_page: 50, total_count: 2 }], pageParams: [0] };
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ activeFolder: "INBOX", selectedMessageUid: 7 });
  client.setQueryData(["messages", "INBOX"], source);
  vi.mocked(apiPost).mockReset().mockImplementation(() => move.promise);
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => useMoveMessage(), { wrapper });
  try {
    act(() => result.current.mutate({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 }));
    await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(1));
    act(() => {
      useAuthStore.setState({ activeAccountId: "account-b" }); client.clear();
      if (switchBack) { useAuthStore.setState({ activeAccountId: "account-a" }); client.clear(); }
      client.setQueryData(["messages", "INBOX"], { ...source, pages: [{ ...source.pages[0], messages: [{ uid: 8, folder: "INBOX" }], total_count: 1 }] });
      useUiStore.setState({ selectedMessageUid: 8 });
    });
    const newCache = client.getQueryData(["messages", "INBOX"]);
    act(() => move.reject(new Error("Old account move failed")));
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(["messages", "INBOX"])).toBe(newCache);
    expect(useUiStore.getState().selectedMessageUid).toBe(8);
  } finally {
    unmount(); client.clear(); useAuthStore.setState({ activeAccountId: previousAccount });
    useUiStore.setState({ selectedMessageUid: null });
  }
});

it("does not run a cleared move's search refresh after returning to the same account", async () => {
  const client = new QueryClient();
  const previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  useUiStore.setState({ selectedMessageUid: null });
  vi.mocked(apiPost).mockReset().mockResolvedValue({});
  const invalidations = vi.spyOn(client, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  const { result, unmount } = renderHook(() => useMoveMessage(), { wrapper });
  try {
    await act(async () => {
      await result.current.mutateAsync({ fromFolder: "INBOX", toFolder: "Junk", uid: 7 });
      invalidations.mockClear();
      useAuthStore.setState({ activeAccountId: "account-b" }); client.clear();
      useAuthStore.setState({ activeAccountId: "account-a" }); client.clear();
    });
    await act(async () => { await new Promise(resolve => setTimeout(resolve, 0)); });
    expect(invalidations).not.toHaveBeenCalledWith({ queryKey: ["search"] });
  } finally {
    unmount(); client.clear(); useAuthStore.setState({ activeAccountId: previousAccount });
  }
});
