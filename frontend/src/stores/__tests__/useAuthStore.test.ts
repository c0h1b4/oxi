import { beforeEach, describe, expect, it, vi } from "vitest";

const ACTIVE_ACCOUNT_STORAGE_KEY = "oxi-active-account-id";

async function loadFreshStore() {
  vi.resetModules();
  return import("../useAuthStore");
}

describe("useAuthStore active account persistence", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("restores active account from localStorage after reload", async () => {
    localStorage.setItem(ACTIVE_ACCOUNT_STORAGE_KEY, "acc-2");

    const { useAuthStore } = await loadFreshStore();

    useAuthStore.getState().setAccounts([
      { id: "acc-1", email: "one@example.com", imapHost: "imap.one", smtpHost: "smtp.one" },
      { id: "acc-2", email: "two@example.com", imapHost: "imap.two", smtpHost: "smtp.two" },
    ]);

    expect(useAuthStore.getState().activeAccountId).toBe("acc-2");
  });

  it("falls back to first account when stored account no longer exists", async () => {
    localStorage.setItem(ACTIVE_ACCOUNT_STORAGE_KEY, "missing-account");

    const { useAuthStore } = await loadFreshStore();

    useAuthStore.getState().setAccounts([
      { id: "acc-1", email: "one@example.com", imapHost: "imap.one", smtpHost: "smtp.one" },
      { id: "acc-2", email: "two@example.com", imapHost: "imap.two", smtpHost: "smtp.two" },
    ]);

    expect(useAuthStore.getState().activeAccountId).toBe("acc-1");
    expect(localStorage.getItem(ACTIVE_ACCOUNT_STORAGE_KEY)).toBe("acc-1");
  });
});
