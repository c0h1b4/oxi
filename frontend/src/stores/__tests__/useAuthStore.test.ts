import { afterAll, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

const ACTIVE_ACCOUNT_STORAGE_KEY = "oxi-active-account-id";

async function loadFreshStore() {
  vi.resetModules();
  return import("../useAuthStore");
}

describe("useAuthStore active account persistence", () => {
  // Node 22+ exposes a native `localStorage` that lacks standard Storage API
  // methods (clear, getItem, setItem, removeItem). Replace it with a simple
  // in-memory implementation so the tests (and the store code they import)
  // work correctly.
  let origLocalStorage: Storage;

  beforeAll(() => {
    origLocalStorage = globalThis.localStorage;

    const store: Record<string, string> = {};
    const storage: Storage = {
      get length() { return Object.keys(store).length; },
      clear() { for (const k of Object.keys(store)) delete store[k]; },
      getItem(key: string) { return store[key] ?? null; },
      setItem(key: string, value: string) { store[key] = String(value); },
      removeItem(key: string) { delete store[key]; },
      key(index: number) { return Object.keys(store)[index] ?? null; },
    };

    Object.defineProperty(globalThis, "localStorage", {
      value: storage,
      writable: true,
      configurable: true,
    });
  });

  afterAll(() => {
    Object.defineProperty(globalThis, "localStorage", {
      value: origLocalStorage,
      writable: true,
      configurable: true,
    });
  });

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
