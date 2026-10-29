import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { useSaveDraft, useSendMessage, useDeleteDraft } from "../useCompose";
import { apiPost } from "@/lib/api";
vi.mock("@/lib/api", () => ({ apiPost: vi.fn(), apiGet: vi.fn(), apiDelete: vi.fn(), apiPostFormData: vi.fn() }));
let client: QueryClient;
beforeEach(() => { vi.resetAllMocks(); client = new QueryClient(); });
afterEach(() => { cleanup(); client.clear(); });
function setup() {
  const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  return renderHook(() => ({ save: useSaveDraft(), send: useSendMessage(), remove: useDeleteDraft() }), { wrapper });
}
const fields = { to: "bob@example.com", cc: "", bcc: "", subject: "edited", htmlBody: null, inReplyTo: null, references: null };
it("waits for an in-flight save before sending and removes the reopened draft from cache", async () => {
  let finishSave!: (value: unknown) => void;
  vi.mocked(apiPost).mockImplementationOnce(() => new Promise(resolve => { finishSave = resolve; })).mockResolvedValueOnce({ status: "sent" });
  client.setQueryData(["drafts"], { drafts: [{ id: "draft-1" }, { id: "draft-2" }] });
  client.setQueryData(["drafts", "draft-1"], { id: "draft-1" });
  const { result } = setup();
  act(() => { result.current.save.mutate({ ...fields, id: "draft-1", textBody: "body" }); });
  await waitFor(() => expect(apiPost).toHaveBeenCalledTimes(1));
  act(() => { result.current.send.mutate({ ...fields, draftId: "draft-1", body: "edited body", fromIdentityId: null }); });
  await waitFor(() => expect(result.current.send.isPaused).toBe(true));
  expect(apiPost).toHaveBeenCalledTimes(1);
  act(() => finishSave({ id: "draft-1", status: "saved" }));
  await waitFor(() => expect(result.current.send.isSuccess).toBe(true));
  expect(apiPost).toHaveBeenLastCalledWith("/messages/send", expect.objectContaining({ draft_id: "draft-1", text_body: "edited body" }));
  expect(client.getQueryData(["drafts"])).toEqual({ drafts: [{ id: "draft-2" }] });
  expect(client.getQueryData(["drafts", "draft-1"])).toBeUndefined();
});
it("keeps a saved draft when SMTP sending fails", async () => {
  vi.mocked(apiPost).mockRejectedValue(new Error("SMTP unavailable"));
  client.setQueryData(["drafts"], { drafts: [{ id: "draft-1" }] });
  const { result } = setup();
  act(() => result.current.send.mutate({ ...fields, draftId: "draft-1", body: "body", fromIdentityId: null }));
  await waitFor(() => expect(result.current.send.isError).toBe(true));
  expect(client.getQueryData(["drafts"])).toEqual({ drafts: [{ id: "draft-1" }] });
});
