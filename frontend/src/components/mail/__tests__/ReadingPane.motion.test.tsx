import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { Children, isValidElement, type Key, type ReactNode } from "react";

vi.mock("framer-motion", async () => {
  const React = await import("react");
  const { useRef, useEffect, useState } = React;

  function getKeyedChildren(children: ReactNode) {
    return Children.toArray(children).filter(
      (child): child is React.ReactElement => isValidElement(child) && child.key != null,
    );
  }

  function AnimatePresence({ children }: { children: ReactNode }) {
    const prevChildrenByKeyRef = useRef(new Map<Key, React.ReactElement>());
    const prevKeysRef = useRef(new Set<Key>());
    const [displayChildren, setDisplayChildren] = useState<React.ReactElement[]>([]);

    useEffect(() => {
      const keyedChildren = getKeyedChildren(children);
      const currentKeys = new Set(keyedChildren.map((child) => child.key as Key));

      const exitingChildren = Array.from(prevKeysRef.current)
        .filter((key) => !currentKeys.has(key))
        .map((key) => prevChildrenByKeyRef.current.get(key))
        .filter((child): child is React.ReactElement => child != null);

      const nextChildren = [...keyedChildren, ...exitingChildren];
      setDisplayChildren(nextChildren);

      for (const child of keyedChildren) {
        prevChildrenByKeyRef.current.set(child.key as Key, child);
      }
      prevKeysRef.current = currentKeys;
    }, [children]);

    return <>{displayChildren}</>;
  }

  return {
    AnimatePresence,
    motion: {
      div: ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => <div {...props}>{children}</div>,
    },
  };
});

const { mockUiState, mockAuthState, mockUseMessage } = vi.hoisted(() => ({
  mockUiState: {
    activeFolder: "INBOX",
    selectedMessageUid: 1 as number | null,
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
    selectMessage: vi.fn(),
  },
  mockAuthState: {
    activeAccountId: "acc-1",
    accounts: [
      { id: "acc-1", email: "one@example.com", imapHost: "imap.one", smtpHost: "smtp.one" },
      { id: "acc-2", email: "two@example.com", imapHost: "imap.two", smtpHost: "smtp.two" },
    ],
  },
  mockUseMessage: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({
    invalidateQueries: vi.fn(),
  }),
}));

vi.mock("@/stores/useUiStore", () => ({
  useUiStore: (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState),
}));

vi.mock("@/stores/useAuthStore", () => ({
  useAuthStore: (selector: (state: typeof mockAuthState) => unknown) => selector(mockAuthState),
}));

vi.mock("@/hooks/useMessages", () => ({
  useMessage: mockUseMessage,
  useUpdateFlags: () => ({ mutate: vi.fn() }),
}));

vi.mock("../EmailRenderer", () => ({
  EmailRenderer: ({ theme }: { theme: string }) => <div data-testid="email-renderer" data-theme={theme} />,
  hasRemoteResources: () => false,
}));

vi.mock("../ThreadView", () => ({
  ThreadView: () => <div data-testid="thread-view" />,
}));

vi.mock("../ReadingPane/index", () => ({
  AddressChip: ({ address }: { address: string }) => <span>{address}</span>,
  AddressList: ({ addresses }: { addresses: Array<{ address: string }> }) => (
    <span>{addresses.map((item) => item.address).join(",")}</span>
  ),
  AttachmentPreviewer: () => null,
  HeaderSkeleton: () => <div data-testid="header-skeleton" />,
  BodySkeleton: () => <div data-testid="body-skeleton" />,
  formatFileSize: (size: number) => `${size} B`,
  humanizeDate: () => "date",
}));

import { ReadingPane } from "../ReadingPane";

function setMessage(uid: number, overrides: { folder?: string; html?: string; text?: string } = {}) {
  mockUseMessage.mockReturnValue({
    data: {
      uid,
      folder: "INBOX",
      subject: `Subject ${uid}`,
      from_address: "sender@example.com",
      from_name: "Sender",
      to_addresses: [{ name: null, address: "receiver@example.com" }],
      cc_addresses: [],
      date: "2026-03-14T00:00:00Z",
      flags: ["\\Seen"],
      html: "<p>Hello</p>",
      text: "Hello",
      raw_headers: "X-Test: yes",
      attachments: [],
      thread: [],
      ...overrides,
    },
    isLoading: false,
    isError: false,
    error: null,
    isPlaceholderData: false,
    refetch: vi.fn(),
  });
}

describe("ReadingPane motion transitions", () => {
  beforeEach(() => {
    mockUiState.activeFolder = "INBOX";
    mockAuthState.activeAccountId = "acc-1";
  });

  it("derives the plain-text default when message data finishes loading", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUid = 1;
    mockUseMessage.mockReturnValue({ isLoading: true });
    const { rerender } = render(<ReadingPane />);

    setMessage(1, { html: "", text: "Plain message" });
    rerender(<ReadingPane />);

    expect(screen.getByText("Plain message")).toBeTruthy();
    expect(screen.getByRole("button", { name: "HTML" })).toBeTruthy();
    expect(screen.queryByTestId("email-renderer")).toBeNull();
  });

  it("preserves view choices on refetch and resets them for each selected message", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUid = 1;
    setMessage(1);
    const { rerender } = render(<ReadingPane />);

    fireEvent.click(screen.getByRole("button", { name: "Plain text" }));
    fireEvent.click(screen.getByRole("button", { name: "Auto theme" }));
    setMessage(1, { text: "Updated text" });
    rerender(<ReadingPane />);
    expect(screen.getByText("Updated text")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Light mode" })).toBeTruthy();

    mockUiState.selectedMessageUid = 2;
    setMessage(2);
    rerender(<ReadingPane />);
    expect(screen.getByTestId("email-renderer").getAttribute("data-theme")).toBe("auto");
    expect(screen.getByRole("button", { name: "Plain text" })).toBeTruthy();
  });

  it("resets view choices across folders and accounts with the same UID", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUid = 1;
    setMessage(1);
    const { rerender } = render(<ReadingPane />);

    for (const selectNext of [
      () => { mockUiState.activeFolder = "Archive"; },
      () => { mockAuthState.activeAccountId = "acc-2"; },
    ]) {
      fireEvent.click(screen.getByRole("button", { name: "Plain text" }));
      fireEvent.click(screen.getByRole("button", { name: "Auto theme" }));
      selectNext();
      setMessage(1, { folder: mockUiState.activeFolder });
      rerender(<ReadingPane />);
      expect(screen.getByTestId("email-renderer").getAttribute("data-theme")).toBe("auto");
      expect(screen.getByRole("button", { name: "Plain text" })).toBeTruthy();
    }
  });

  it("animates content transition when selected UID changes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.selectedMessageUid = 1;
    setMessage(1);

    const { rerender } = render(<ReadingPane />);
    expect(screen.getByTestId("reading-pane-message-transition")).toBeTruthy();

    mockUiState.selectedMessageUid = 2;
    setMessage(2);
    rerender(<ReadingPane />);

    const wrappers = screen.getAllByTestId("reading-pane-message-transition");
    expect(wrappers.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Subject 2")).toBeTruthy();
  });

  it("renders static path in off mode without motion wrappers", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUid = 5;
    setMessage(5);

    render(<ReadingPane />);

    expect(screen.queryByTestId("reading-pane-message-transition")).toBeNull();
    expect(screen.getByText("Subject 5")).toBeTruthy();
  });

  it("uses transform/opacity-only transition declarations", () => {
    mockUiState.effectiveAnimationMode = "rich";
    mockUiState.selectedMessageUid = 7;
    setMessage(7);

    render(<ReadingPane />);

    const wrapper = screen.getByTestId("reading-pane-message-transition");
    const serialized = wrapper.getAttribute("data-motion-props") ?? "";

    for (const forbidden of ["width", "height", "top", "left"]) {
      expect(serialized.includes(forbidden)).toBe(false);
    }
  });

  it("keeps animated wrapper full width to avoid half-pane rendering", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.selectedMessageUid = 9;
    setMessage(9);

    render(<ReadingPane />);

    const wrapper = screen.getByTestId("reading-pane-message-transition");
    expect(wrapper.className).toContain("w-full");
    expect(wrapper.className).toContain("min-w-0");
  });
});

describe("ReadingPane message view state", () => {
  beforeEach(() => {
    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUid = 1;
    mockUiState.activeFolder = "INBOX";
    mockAuthState.activeAccountId = "acc-1";
  });

  it("starts plaintext-only messages in plain text mode", () => {
    setMessage(1, { html: "" });
    render(<ReadingPane />);

    expect(screen.getByRole("button", { name: "HTML" })).toBeTruthy();
    expect(screen.queryByTestId("email-renderer")).toBeNull();
  });

  it("selects plain text mode when message data finishes loading", () => {
    mockUseMessage.mockReturnValue({ isLoading: true });
    const { rerender } = render(<ReadingPane />);

    setMessage(1, { html: "" });
    rerender(<ReadingPane />);

    expect(screen.getByRole("button", { name: "HTML" })).toBeTruthy();
    expect(screen.queryByTestId("email-renderer")).toBeNull();
  });

  it("preserves manual mode and theme choices when the same message refreshes", () => {
    setMessage(1);
    const { rerender } = render(<ReadingPane />);
    fireEvent.click(screen.getByRole("button", { name: "Plain text" }));
    fireEvent.click(screen.getByRole("button", { name: "Auto theme" }));

    setMessage(1);
    rerender(<ReadingPane />);

    expect(screen.getByRole("button", { name: "HTML" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Light mode" })).toBeTruthy();
  });

  it.each(["uid", "folder", "account"])("resets mode and theme when the message %s changes", (identity) => {
    setMessage(1);
    const { rerender } = render(<ReadingPane />);
    fireEvent.click(screen.getByRole("button", { name: "Plain text" }));
    fireEvent.click(screen.getByRole("button", { name: "Auto theme" }));

    if (identity === "uid") {
      mockUiState.selectedMessageUid = 2;
      setMessage(2);
    } else if (identity === "folder") {
      mockUiState.activeFolder = "Archive";
      setMessage(1, { folder: "Archive" });
    } else {
      mockAuthState.activeAccountId = "acc-2";
      setMessage(1);
    }
    rerender(<ReadingPane />);

    expect(screen.getByRole("button", { name: "Plain text" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Auto theme" })).toBeTruthy();
    expect(screen.getByTestId("email-renderer")).toBeTruthy();
  });
});
