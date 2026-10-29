import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { MessageDetail } from "@/types/message";

const { mockUseMessage } = vi.hoisted(() => ({ mockUseMessage: vi.fn() }));
vi.mock("@/hooks/useMessages", () => ({
  useMessage: mockUseMessage,
  useUpdateFlags: () => ({ mutate: vi.fn() }),
}));
vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));
vi.mock("@/hooks/useContacts", () => ({
  useCreateContact: () => ({ mutate: vi.fn(), isPending: false }),
}));
vi.mock("framer-motion", () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => children,
}));

import { ReadingPane } from "../ReadingPane";
import { useAuthStore, type Account } from "@/stores/useAuthStore";
import { useUiStore } from "@/stores/useUiStore";
import { remoteResourceAccountScope, useRemoteResourceStore } from "@/stores/useRemoteResourceStore";

const accounts: Account[] = [
  { id: "acc-1", email: "one@example.com", imapHost: "imap.one", smtpHost: "smtp.one" },
  { id: "acc-2", email: "two@example.com", imapHost: "imap.two", smtpHost: "smtp.two" },
];

function setMessage(overrides: Partial<MessageDetail> = {}) {
  const data: MessageDetail = {
    uid: useUiStore.getState().selectedMessageUid ?? 1,
    folder: useUiStore.getState().activeFolder,
    subject: "Private message",
    from_address: "sender@example.com", from_name: "Sender Name",
    to_addresses: [{ name: "Recipient", address: "recipient@example.com" }],
    cc_addresses: [{ name: "CC Recipient", address: "cc@example.com" }],
    date: "2026-10-20T12:00:00Z", flags: ["\\Seen"],
    html: '<p>Hello</p><img src="https://remote.example.com/tracker.png">', text: "Hello",
    raw_headers: "From: sender@example.com", attachments: [], thread: [], ...overrides,
  };
  mockUseMessage.mockReturnValue({ data, isLoading: false, isError: false, refetch: vi.fn() });
}

function remoteImageSource() {
  const html = screen.getByTitle("Email content").getAttribute("srcdoc")!;
  return new DOMParser().parseFromString(html, "text/html").querySelector("img")?.getAttribute("src");
}

beforeEach(() => {
  vi.stubGlobal("localStorage", window.localStorage);
  localStorage.clear();
  vi.stubGlobal("matchMedia", vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })));
  useAuthStore.setState({ accounts, activeAccountId: "acc-1" });
  useUiStore.setState({ selectedMessageUid: 1, activeFolder: "INBOX", effectiveAnimationMode: "off" });
  useRemoteResourceStore.setState({ sendersByAccount: {} });
  setMessage();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("ReadingPane sender privacy", () => {
  it("blocks by default, remembers the From sender for future messages, and survives remount", () => {
    const view = render(<ReadingPane />);
    expect(remoteImageSource()).toBe("");
    fireEvent.click(screen.getByRole("button", { name: "Always allow from this sender" }));
    expect(remoteImageSource()).toBe("https://remote.example.com/tracker.png");
    expect(screen.queryByText(/remote resources have been blocked/)).toBeNull();

    act(() => useUiStore.setState({ selectedMessageUid: 2, activeFolder: "Archive" }));
    setMessage({ from_name: "Different Display Name" });
    view.rerender(<ReadingPane />);
    expect(remoteImageSource()).toBe("https://remote.example.com/tracker.png");

    view.unmount();
    useRemoteResourceStore.setState({ sendersByAccount: {} });
    render(<ReadingPane />);
    expect(remoteImageSource()).toBe("https://remote.example.com/tracker.png");
  });

  it("does not trust the same display name, domain, or folder/UID from an unrelated sender", () => {
    const view = render(<ReadingPane />);
    fireEvent.click(screen.getByRole("button", { name: "Always allow from this sender" }));
    for (const from_address of ["other@example.com", "sender@other.com", "sender+tag@example.com"]) {
      setMessage({ from_address });
      view.rerender(<ReadingPane />);
      expect(remoteImageSource()).toBe("");
      expect(screen.getByRole("button", { name: "Always allow from this sender" })).toBeTruthy();
    }
  });

  it("does not share trust across accounts even when folder, UID and sender match", () => {
    const view = render(<ReadingPane />);
    fireEvent.click(screen.getByRole("button", { name: "Always allow from this sender" }));
    act(() => useAuthStore.setState({ activeAccountId: "acc-2" }));
    view.rerender(<ReadingPane />);
    expect(remoteImageSource()).toBe("");
    act(() => useAuthStore.setState({ activeAccountId: "acc-1" }));
    view.rerender(<ReadingPane />);
    expect(remoteImageSource()).toBe("https://remote.example.com/tracker.png");
  });

  it("keeps resources blocked when the account is unknown or the From address is invalid", () => {
    const view = render(<ReadingPane />);
    act(() => useAuthStore.setState({ activeAccountId: null }));
    view.rerender(<ReadingPane />);
    expect((screen.getByRole("button", { name: "Always allow from this sender" }) as HTMLButtonElement).disabled).toBe(true);
    expect(remoteImageSource()).toBe("");
    act(() => useAuthStore.setState({ activeAccountId: "acc-1" }));
    setMessage({ from_address: "" });
    view.rerender(<ReadingPane />);
    expect((screen.getByRole("button", { name: "Always allow from this sender" }) as HTMLButtonElement).disabled).toBe(true);
    expect(remoteImageSource()).toBe("");
  });

  it("loads existing account-scoped trust without requiring another Allow click", () => {
    const scope = remoteResourceAccountScope(accounts[0])!;
    localStorage.setItem("oxi-remote-resource-trust:" + scope, JSON.stringify(["sender@example.com"]));
    render(<ReadingPane />);
    expect(remoteImageSource()).toBe("https://remote.example.com/tracker.png");
  });
});

describe("ReadingPane sender details", () => {
  it("shows sent-mail recipients without expanding sender details", () => {
    useUiStore.setState({ activeFolder: "Sent" });
    setMessage({ from_address: "one@example.com" });
    render(<ReadingPane />);
    expect(screen.getByText("To:")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Recipient" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();
  });

  it("starts closed and only opens from the sender-name button, not header clicks or focus", () => {
    render(<ReadingPane />);
    const sender = screen.getByRole("button", { name: "Sender Name" });
    expect(sender.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(screen.getByRole("heading", { name: "Private message" }));
    fireEvent.focus(sender);
    expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();

    fireEvent.click(sender);
    expect(sender.getAttribute("aria-expanded")).toBe("true");
    const details = screen.getByRole("region", { name: "Sender details" });
    expect(within(details).getByRole("button", { name: "Sender Name" })).toBeTruthy();
    expect(within(details).getByText("<sender@example.com>")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Recipient" })).toBeTruthy();
    expect(within(details).getByRole("button", { name: "CC Recipient" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    fireEvent.click(sender);
    expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();
  });

  it("keeps an explicit choice on refetch but closes details on message, folder or account change", () => {
    const view = render(<ReadingPane />);
    fireEvent.click(screen.getByRole("button", { name: "Sender Name" }));
    setMessage();
    view.rerender(<ReadingPane />);
    expect(screen.getByRole("region", { name: "Sender details" })).toBeTruthy();

    for (const nextSelection of [
      () => useUiStore.setState({ selectedMessageUid: 2 }),
      () => useUiStore.setState({ activeFolder: "Archive" }),
      () => useAuthStore.setState({ activeAccountId: "acc-2" }),
    ]) {
      act(() => nextSelection());
      setMessage();
      view.rerender(<ReadingPane />);
      expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();
      fireEvent.click(screen.getByRole("button", { name: "Sender Name" }));
    }
  });

  it("opens details by clicking the From address when there is no display name", () => {
    setMessage({ from_name: "" });
    render(<ReadingPane />);
    expect(screen.queryByRole("region", { name: "Sender details" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "sender@example.com" }));
    expect(screen.getByRole("region", { name: "Sender details" })).toBeTruthy();
  });
});
