import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useWebSocket } from "../useWebSocket";
import { useAuthStore } from "@/stores/useAuthStore";

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  onopen: (() => void) | null = null;
  onclose: (() => void) | null = null;
  onerror: (() => void) | null = null;
  onmessage: ((event: { data: string }) => void) | null = null;
  close = vi.fn();
  constructor(public url: URL) {
    MockWebSocket.instances.push(this);
  }
  open() { this.onopen?.(); }
  message(event: unknown, accountId = "account-a") {
    this.onmessage?.({ data: JSON.stringify({ accountId, event }) });
  }
}

let client: QueryClient;
let previousAccount: string | null;

beforeEach(() => {
  vi.useFakeTimers();
  MockWebSocket.instances = [];
  vi.stubGlobal("WebSocket", MockWebSocket);
  vi.spyOn(navigator, "onLine", "get").mockReturnValue(true);
  vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
  previousAccount = useAuthStore.getState().activeAccountId;
  useAuthStore.setState({ activeAccountId: "account-a" });
  client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
});

afterEach(() => {
  cleanup();
  client.clear();
  useAuthStore.setState({ activeAccountId: previousAccount });
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function setup(onEvent = vi.fn()) {
  const invalidate = vi.spyOn(client, "invalidateQueries");
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return { ...renderHook(() => useWebSocket(onEvent), { wrapper }), invalidate, onEvent };
}

function expectMailRefresh(invalidate: ReturnType<typeof vi.spyOn>) {
  for (const key of ["messages", "message", "folders", "search"]) {
    expect(invalidate).toHaveBeenCalledWith({ queryKey: [key] });
  }
}

describe("mail recovery", () => {
  it("refreshes missed mail on initial connection and backoff reconnect", () => {
    const { result, invalidate } = setup();
    expect(result.current.status).toBe("connecting");
    const first = MockWebSocket.instances[0];
    act(() => first.open());
    expect(result.current.status).toBe("connected");
    expectMailRefresh(invalidate);
    invalidate.mockClear();
    act(() => first.onclose?.());
    expect(result.current).toEqual({ status: "disconnected", failCount: 1 });
    act(() => vi.advanceTimersByTime(1000));
    expect(MockWebSocket.instances).toHaveLength(2);
    act(() => MockWebSocket.instances[1].open());
    expect(result.current).toEqual({ status: "connected", failCount: 0 });
    expectMailRefresh(invalidate);
  });

  it.each(["focus", "pageshow", "visibilitychange", "resume"])("replaces a half-open socket on %s and ignores its late callbacks", (event) => {
    const { result, invalidate, onEvent } = setup();
    const first = MockWebSocket.instances[0];
    act(() => first.open());
    const lateClose = first.onclose;
    const lateMessage = first.onmessage;
    invalidate.mockClear();
    act(() => {
      (["visibilitychange", "resume"].includes(event) ? document : window).dispatchEvent(new Event(event));
      vi.advanceTimersByTime(0);
    });
    expect(first.close).toHaveBeenCalledOnce();
    expect(MockWebSocket.instances).toHaveLength(2);
    expectMailRefresh(invalidate); // Refresh even if reconnecting is slow.
    act(() => {
      MockWebSocket.instances[1].open();
      lateClose?.();
      lateMessage?.({ data: JSON.stringify({ accountId: "account-a", event: { type: "NewMessages" } }) });
      vi.advanceTimersByTime(1000);
    });
    expect(result.current.status).toBe("connected");
    expect(MockWebSocket.instances).toHaveLength(2);
    expect(onEvent).not.toHaveBeenCalled();
  });

  it("coalesces resume events, cancels backoff and waits for a visible online page", () => {
    const { invalidate } = setup();
    act(() => MockWebSocket.instances[0].onclose?.());
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("hidden");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("focus"));
      vi.advanceTimersByTime(0);
    });
    expect(MockWebSocket.instances).toHaveLength(1);
    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    act(() => {
      window.dispatchEvent(new Event("pageshow"));
      document.dispatchEvent(new Event("visibilitychange"));
      window.dispatchEvent(new Event("focus"));
      vi.advanceTimersByTime(0);
    });
    expect(MockWebSocket.instances).toHaveLength(2);
    expectMailRefresh(invalidate);
    act(() => { MockWebSocket.instances[1].open(); vi.advanceTimersByTime(1000); });
    expect(MockWebSocket.instances).toHaveLength(2);
  });

  it("reconnects immediately on network recovery without waiting for an old close event", () => {
    const { result, invalidate } = setup();
    act(() => MockWebSocket.instances[0].open());
    invalidate.mockClear();
    vi.spyOn(navigator, "onLine", "get").mockReturnValue(false);
    act(() => { window.dispatchEvent(new Event("offline")); vi.advanceTimersByTime(30_000); });
    expect(result.current.status).toBe("disconnected");
    expect(MockWebSocket.instances).toHaveLength(1);
    vi.spyOn(navigator, "onLine", "get").mockReturnValue(true);
    act(() => { window.dispatchEvent(new Event("online")); vi.advanceTimersByTime(0); });
    expect(MockWebSocket.instances).toHaveLength(2);
    expectMailRefresh(invalidate);
    act(() => MockWebSocket.instances[1].open());
    expect(result.current.status).toBe("connected");
  });

  it("retries a stuck CONNECTING socket and cleans up timers/listeners on unmount", () => {
    const { result, unmount, invalidate } = setup();
    act(() => vi.advanceTimersByTime(15_000));
    expect(result.current.status).toBe("disconnected");
    expect(MockWebSocket.instances[0].close).toHaveBeenCalledOnce();
    act(() => vi.advanceTimersByTime(1000));
    expect(MockWebSocket.instances).toHaveLength(2);
    act(() => {
      MockWebSocket.instances[1].open();
      MockWebSocket.instances[1].message({ type: "FolderUpdated" });
      window.dispatchEvent(new Event("focus"));
    });
    invalidate.mockClear();
    unmount();
    act(() => {
      window.dispatchEvent(new Event("online"));
      window.dispatchEvent(new Event("focus"));
      document.dispatchEvent(new Event("visibilitychange"));
      vi.advanceTimersByTime(60_000);
    });
    expect(MockWebSocket.instances[1].close).toHaveBeenCalledOnce();
    expect(MockWebSocket.instances).toHaveLength(2);
    expect(invalidate).not.toHaveBeenCalled();
  });

  it("refreshes message lists after background sync and filters other accounts' events", () => {
    const { invalidate, onEvent } = setup();
    const socket = MockWebSocket.instances[0];
    act(() => socket.open());
    invalidate.mockClear();
    act(() => {
      socket.message({ type: "NewMessages", data: { folder: "INBOX" } }, "account-b");
      vi.advanceTimersByTime(300);
    });
    expect(invalidate).not.toHaveBeenCalled();
    expect(onEvent).not.toHaveBeenCalled();
    act(() => { socket.message({ type: "FolderUpdated" }); vi.advanceTimersByTime(300); });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["messages"] });
    expect(invalidate).toHaveBeenCalledWith({ queryKey: ["folders"] });
    expect(onEvent).toHaveBeenCalledWith({ type: "FolderUpdated" });
  });
});
