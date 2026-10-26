import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Account } from "../useAuthStore";
import {
  normalizeSenderAddress,
  remoteResourceAccountScope,
  useRemoteResourceStore,
} from "../useRemoteResourceStore";

const account: Account = {
  id: "session-1", email: "me@example.com", imapHost: "imap.example.com", smtpHost: "smtp.example.com",
};
const scope = remoteResourceAccountScope(account)!;
const storageKey = "oxi-remote-resource-trust:" + scope;

beforeEach(() => {
  vi.stubGlobal("localStorage", window.localStorage);
  localStorage.clear();
  useRemoteResourceStore.setState({ sendersByAccount: {} });
});
afterEach(() => vi.unstubAllGlobals());

describe("remote resource sender trust", () => {
  it("persists exact From addresses across store reloads and account re-login", async () => {
    useRemoteResourceStore.getState().trustSender(scope, " sender@EXAMPLE.com ");
    expect(JSON.parse(localStorage.getItem(storageKey)!)).toEqual(["sender@example.com"]);
    const reauthenticated = remoteResourceAccountScope({ ...account, id: "session-2" });
    expect(reauthenticated).toBe(scope);

    vi.resetModules();
    const { useRemoteResourceStore: freshStore } = await import("../useRemoteResourceStore");
    freshStore.getState().loadAccount(reauthenticated!);
    expect(freshStore.getState().sendersByAccount[scope]).toEqual(["sender@example.com"]);
  });

  it("scopes trust by mailbox and IMAP host, not just a session ID", () => {
    useRemoteResourceStore.getState().trustSender(scope, "sender@example.com");
    for (const other of [
      { ...account, email: "other@example.com" },
      { ...account, imapHost: "other.imap.example.com" },
    ]) {
      const otherScope = remoteResourceAccountScope(other)!;
      useRemoteResourceStore.getState().loadAccount(otherScope);
      expect(useRemoteResourceStore.getState().sendersByAccount[otherScope]).toEqual([]);
    }
    expect(remoteResourceAccountScope(undefined)).toBeNull();
    expect(remoteResourceAccountScope({ ...account, imapHost: "" })).toBeNull();
  });

  it("normalizes only domain case and whitespace without broadening sender trust", () => {
    expect(normalizeSenderAddress(" Sender@EXAMPLE.COM ")).toBe("Sender@example.com");
    useRemoteResourceStore.getState().trustSender(scope, "Sender@example.com");
    const trusted = useRemoteResourceStore.getState().sendersByAccount[scope];
    for (const unrelated of ["sender@example.com", "other@example.com", "Sender@other.com", "Sender+tag@example.com"]) {
      expect(trusted).not.toContain(normalizeSenderAddress(unrelated));
    }
    for (const invalid of ["", "Sender", "Sender <sender@example.com>", "a@b.com,c@d.com", "a@b.com;", "a@b.com\nother@b.com"]) {
      expect(normalizeSenderAddress(invalid)).toBeNull();
    }
  });

  it("deduplicates and preserves previously saved sender choices", () => {
    useRemoteResourceStore.getState().trustSender(scope, "first@example.com");
    useRemoteResourceStore.getState().trustSender(scope, "second@example.com");
    useRemoteResourceStore.getState().trustSender(scope, "second@EXAMPLE.COM");
    expect(JSON.parse(localStorage.getItem(storageKey)!)).toEqual(["first@example.com", "second@example.com"]);
  });

  it.each(["{", "null", "true", '{"sender@example.com":true}', '"sender@example.com"'])(
    "fails closed on malformed stored data: %s", (raw) => {
      localStorage.setItem(storageKey, raw);
      useRemoteResourceStore.getState().loadAccount(scope);
      expect(useRemoteResourceStore.getState().sendersByAccount[scope]).toEqual([]);
    },
  );

  it("ignores invalid entries in stored sender lists", () => {
    localStorage.setItem(storageKey, JSON.stringify([null, 7, {}, "", "example.com", "sender@example.com", "sender@EXAMPLE.COM"]));
    useRemoteResourceStore.getState().loadAccount(scope);
    expect(useRemoteResourceStore.getState().sendersByAccount[scope]).toEqual(["sender@example.com"]);
  });

  it("handles denied browser storage and retains only the explicit in-memory choice", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => { throw new Error("denied"); },
      setItem: () => { throw new Error("denied"); },
    });
    useRemoteResourceStore.getState().loadAccount(scope);
    expect(useRemoteResourceStore.getState().sendersByAccount[scope]).toEqual([]);
    expect(() => useRemoteResourceStore.getState().trustSender(scope, "sender@example.com")).not.toThrow();
    expect(useRemoteResourceStore.getState().sendersByAccount[scope]).toEqual(["sender@example.com"]);
  });
});
