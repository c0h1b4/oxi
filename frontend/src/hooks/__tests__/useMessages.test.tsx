import { describe, it, expect, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useMessages, useMoveMessage } from "../useMessages";
import { apiGet, apiPost } from "@/lib/api";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn(), apiPatch: vi.fn(), apiPost: vi.fn(), apiDelete: vi.fn() }));
vi.mock("@/lib/ws-context", () => ({ useWsStatus: () => ({ status: "connected" }) }));

describe("folder navigation", () => {
  it("never inserts the source UID into Spam during or after a move", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const source = { pages: [{ messages: [{ uid: 16891, folder: "INBOX" }], page: 0, per_page: 50, total_count: 1 }], pageParams: [0] };
    const destination = { pages: [{ messages: [], page: 0, per_page: 50, total_count: 0 }], pageParams: [0] };
    client.setQueryData(["messages", "INBOX"], source);
    client.setQueryData(["messages", "Spam"], destination);
    client.setQueryData(["search", "test"], { results: [{ uid: 16891, folder: "INBOX" }], total_count: 1, query: "test" });
    let finish!: () => void;
    vi.mocked(apiPost).mockImplementation(() => new Promise(resolve => { finish = () => resolve({ status: "ok" }); }));
    const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
    const { result, unmount } = renderHook(() => useMoveMessage(), { wrapper });
    act(() => result.current.mutate({ fromFolder: "INBOX", toFolder: "Spam", uid: 16891 }));
    await waitFor(() => expect(finish).toBeDefined());
    expect(client.getQueryData(["messages", "Spam"])).toEqual(destination);
    expect(client.getQueryData(["search", "test"])).toEqual({ results: [], total_count: 0, query: "test" });
    act(() => finish());
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(client.getQueryData(["messages", "Spam"])).toEqual(destination);
    unmount();
    client.clear();
  });
  it("restores an optimistically hidden search result when moving fails", async () => {
    const client = new QueryClient();
    const search = { results: [{ uid: 7, folder: "INBOX" }], total_count: 1, query: "test" };
    client.setQueryData(["search", "test"], search);
    let fail!: () => void;
    vi.mocked(apiPost).mockImplementation(() => new Promise((_resolve, reject) => { fail = () => reject(new Error("Move failed")); }));
    const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
    const { result, unmount } = renderHook(() => useMoveMessage(), { wrapper });
    act(() => result.current.mutate({ fromFolder: "INBOX", toFolder: "Spam", uid: 7 }));
    await waitFor(() => expect(fail).toBeDefined());
    expect(client.getQueryData(["search", "test"])).toEqual({ ...search, results: [], total_count: 0 });
    act(() => fail());
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect(client.getQueryData(["search", "test"])).toEqual(search);
    unmount();
    client.clear();
  });

  it("does not show Inbox data while Junk is loading", async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    vi.mocked(apiGet).mockImplementation((url) => url.includes("/INBOX/")
      ? Promise.resolve({ messages: [{ uid: 1, folder: "INBOX" }], page: 0, per_page: 50, total_count: 1 })
      : new Promise(() => {}));
    const wrapper = ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    );
    const { result, rerender, unmount } = renderHook(({ folder }) => useMessages(folder), {
      initialProps: { folder: "INBOX" }, wrapper,
    });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    rerender({ folder: "Junk" });
    expect(result.current.data).toBeUndefined();
    expect(result.current.isPending).toBe(true);
    unmount();
    client.clear();
  });
});
