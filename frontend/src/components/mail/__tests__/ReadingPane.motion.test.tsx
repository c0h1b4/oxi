/* eslint-disable react-hooks/refs */
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
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
  EmailRenderer: () => <div data-testid="email-renderer" />,
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

function setMessage(uid: number) {
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
    },
    isLoading: false,
    isError: false,
    error: null,
    isPlaceholderData: false,
    refetch: vi.fn(),
  });
}

describe("ReadingPane motion transitions", () => {
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
