import { createElement, type ReactNode } from "react";
import { act, cleanup, renderHook } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, expect, it, vi } from "vitest";
import { apiPost, apiPut } from "@/lib/api";
import { useCreateEvent, useSendEventInvitations, useUpdateEvent } from "../useCalendar";

vi.mock("@/lib/api", () => ({ apiGet: vi.fn(), apiPost: vi.fn(), apiPut: vi.fn(), apiDelete: vi.fn() }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });
function wrapper({ children }: { children: ReactNode }) {
  const client = new QueryClient({ defaultOptions: { mutations: { retry: false } } });
  return createElement(QueryClientProvider, { client }, children);
}

it("resends using the saved event invitation endpoint", async () => {
  vi.mocked(apiPost).mockResolvedValue({ sent: 2 });
  const { result } = renderHook(() => useSendEventInvitations(), { wrapper });
  await act(async () => { await result.current.mutateAsync("meeting"); });
  expect(apiPost).toHaveBeenCalledWith("/calendar/events/meeting/invitations", {});
});

it("passes the explicit edit invitation decision to the server", async () => {
  vi.mocked(apiPut).mockResolvedValue({ id: "meeting" });
  const { result } = renderHook(() => useUpdateEvent(), { wrapper });
  await act(async () => {
    await result.current.mutateAsync({ id: "meeting", title: "Updated", send_invitations: true });
  });
  expect(apiPut).toHaveBeenCalledWith("/calendar/events/meeting", { title: "Updated", send_invitations: true });
});

it("returns invitation failures on successful event creation", async () => {
  const response = { id: "meeting", invitation_error: "SMTP unavailable" };
  vi.mocked(apiPost).mockResolvedValue(response);
  const { result } = renderHook(() => useCreateEvent(), { wrapper });
  let saved;
  await act(async () => {
    saved = await result.current.mutateAsync({ title: "Meeting", start_time: "2026-11-15T09:00:00", end_time: "2026-11-15T10:00:00" });
  });
  expect(saved).toEqual(response);
});
