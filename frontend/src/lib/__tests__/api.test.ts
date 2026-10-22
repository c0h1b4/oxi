import { readFileSync } from "node:fs";
import { URL } from "node:url";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAuthStore } from "@/stores/useAuthStore";
import {
  apiDelete,
  apiGet,
  apiPatch,
  apiPost,
  apiPostFormData,
  apiPut,
  fetchAccounts,
} from "@/lib/api";
import { toast } from "sonner";

vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

const expiredAccountResponse = JSON.parse(readFileSync(
  new URL("../../../../backend/tests/fixtures/account-session-expired.json", import.meta.url),
  "utf8",
));

const expiredAccount = {
  id: expiredAccountResponse.error.accountId,
  email: "expired@example.com",
  imapHost: "imap.example.com",
  smtpHost: "smtp.example.com",
};
const otherAccount = { ...expiredAccount, id: "other-account", email: "other@example.com" };
const fetchMock = vi.fn<typeof fetch>();

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  useAuthStore.setState({
    accounts: [expiredAccount, otherAccount],
    activeAccountId: expiredAccount.id,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetAllMocks();
  useAuthStore.setState({ accounts: [], activeAccountId: null });
  localStorage.clear();
});

const requests = [
  ["GET", () => apiGet("/folders")],
  ["POST", () => apiPost("/folders", { name: "Archive" })],
  ["PATCH", () => apiPatch("/messages/INBOX/1/flags", { flags: ["\\Seen"] })],
  ["PUT", () => apiPut("/settings", {})],
  ["DELETE", () => apiDelete("/folders/Archive")],
  ["form upload", () => apiPostFormData("/attachments", new FormData())],
  ["account list", fetchAccounts],
] as const;

describe("expired account API contract", () => {
  it.each(requests)("cleans up the account after %s fails", async (_name, request) => {
    fetchMock
      .mockResolvedValueOnce(new Response(JSON.stringify(expiredAccountResponse), { status: 401 }))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));

    await expect(request()).rejects.toThrow(expiredAccountResponse.error.message);

    expect(useAuthStore.getState().accounts).toEqual([otherAccount]);
    expect(useAuthStore.getState().activeAccountId).toBe(otherAccount.id);
    expect(localStorage.getItem("oxi-active-account-id")).toBe(otherAccount.id);
    expect(toast.error).toHaveBeenCalledWith(expiredAccountResponse.error.message);
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenLastCalledWith(`/api/auth/accounts/${expiredAccount.id}`, {
      method: "DELETE",
      headers: { "X-Requested-With": "XMLHttpRequest" },
      credentials: "same-origin",
    });
  });

  it("does not remove accounts for a generic unauthorized response", async () => {
    fetchMock.mockResolvedValueOnce(new Response(JSON.stringify({
      error: { code: "UNAUTHORIZED", message: "Invalid or expired session", status: 401 },
    }), { status: 401 }));

    await expect(apiGet("/folders")).rejects.toThrow("Invalid or expired session");

    expect(useAuthStore.getState().accounts).toEqual([expiredAccount, otherAccount]);
    expect(toast.error).not.toHaveBeenCalled();
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("keeps local cleanup when the cookie cleanup request fails", async () => {
    fetchMock
      .mockResolvedValueOnce(new Response(JSON.stringify(expiredAccountResponse), { status: 401 }))
      .mockRejectedValueOnce(new Error("offline"));

    await expect(apiGet("/folders")).rejects.toThrow(expiredAccountResponse.error.message);

    expect(useAuthStore.getState().accounts).toEqual([otherAccount]);
    expect(toast.error).toHaveBeenCalledTimes(1);
  });
});
