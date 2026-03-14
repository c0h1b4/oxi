/* eslint-disable react-hooks/refs, react/display-name */
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Children, isValidElement, useRef, type Key, type ReactNode } from "react";

vi.mock("framer-motion", async () => {
  const React = await import("react");

  function getKeyedChildren(children: ReactNode) {
    return Children.toArray(children).filter(
      (child): child is React.ReactElement => isValidElement(child) && child.key != null,
    );
  }

  function AnimatePresence({ children }: { children: ReactNode }) {
    const prevChildrenByKeyRef = useRef(new Map<Key, React.ReactElement>());
    const prevKeysRef = useRef(new Set<Key>());

    const keyedChildren = getKeyedChildren(children);
    const currentKeys = new Set(keyedChildren.map((child) => child.key as Key));

    const exitingChildren = Array.from(prevKeysRef.current)
      .filter((key) => !currentKeys.has(key))
      .map((key) => prevChildrenByKeyRef.current.get(key))
      .filter((child): child is React.ReactElement => child != null);

    for (const child of keyedChildren) {
      prevChildrenByKeyRef.current.set(child.key as Key, child);
    }
    prevKeysRef.current = currentKeys;

    return <>{[...keyedChildren, ...exitingChildren]}</>;
  }

  const MotionDiv = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div {...props}>{children}</div>
  );
  const MotionSpan = ({ children, ...props }: React.HTMLAttributes<HTMLSpanElement>) => (
    <span {...props}>{children}</span>
  );

  return {
    AnimatePresence,
    motion: {
      div: MotionDiv,
      span: MotionSpan,
    },
  };
});

const {
  mockUiState,
  mockComposeState,
  mockSendMessage,
  mockCreateFolder,
  mockUseMessage,
  mockBulkUids,
} = vi.hoisted(() => ({
  mockUiState: {
    effectiveAnimationMode: "medium" as "rich" | "medium" | "subtle" | "off",
    commandPaletteOpen: false,
    activeFolder: "INBOX",
    selectedMessageUid: 1 as number | null,
    selectedMessageUids: [101, 102] as number[],
    setCommandPaletteOpen: vi.fn(),
    setTheme: vi.fn(),
    setViewMode: vi.fn(),
    setSearchActive: vi.fn(),
    setShortcutsOpen: vi.fn(),
    clearBulkSelection: vi.fn(),
  },
  mockComposeState: {
    isOpen: true,
    to: "to@example.com",
    cc: "",
    bcc: "",
    subject: "Hello",
    body: "Body",
    inReplyTo: null as string | null,
    references: null as string | null,
    draftId: null as string | null,
    showCc: false,
    showBcc: false,
    attachments: [] as Array<{ id: string; filename: string; contentType: string; size: number }>,
    fromIdentityId: null as number | null,
    isHtml: false,
    signatureHtml: "",
    signatureEnabled: false,
    closeCompose: vi.fn(),
    setField: vi.fn(),
    setShowCc: vi.fn(),
    setShowBcc: vi.fn(),
    setDraftId: vi.fn(),
    setFromIdentityId: vi.fn(),
    setIsHtml: vi.fn(),
    setSignatureHtml: vi.fn(),
    setSignatureEnabled: vi.fn(),
    addAttachments: vi.fn(),
    removeAttachment: vi.fn(),
    reset: vi.fn(),
  },
  mockSendMessage: {
    isPending: false,
    mutate: vi.fn(),
  },
  mockCreateFolder: {
    isPending: false,
    isError: false,
    error: null as Error | null,
    mutate: vi.fn(),
    reset: vi.fn(),
  },
  mockUseMessage: vi.fn(),
  mockBulkUids: [101, 102] as number[],
}));

vi.mock("next/dynamic", () => ({
  default: () => () => <div data-testid="rich-text-editor" />,
}));

vi.mock("cmdk", () => {
  function CommandRoot({ children }: { children: ReactNode }) {
    return <div>{children}</div>;
  }
  CommandRoot.Input = ({ ...props }: React.InputHTMLAttributes<HTMLInputElement>) => (
    <input {...props} />
  );
  CommandRoot.List = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div {...props}>{children}</div>
  );
  CommandRoot.Empty = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div {...props}>{children}</div>
  );
  CommandRoot.Group = ({ children, ...props }: React.HTMLAttributes<HTMLDivElement>) => (
    <div {...props}>{children}</div>
  );
  CommandRoot.Item = ({ children, onSelect, ...props }: React.HTMLAttributes<HTMLDivElement> & { onSelect?: () => void }) => (
    <button type="button" onClick={onSelect} {...props}>{children}</button>
  );

  return { Command: CommandRoot };
});

vi.mock("@/stores/useUiStore", () => {
  const useUiStore = (selector: (state: typeof mockUiState) => unknown) => selector(mockUiState);
  useUiStore.getState = () => mockUiState;
  return { useUiStore };
});

vi.mock("@/stores/useComposeStore", () => {
  const useComposeStore = (selector?: (state: typeof mockComposeState) => unknown) =>
    selector ? selector(mockComposeState) : mockComposeState;
  useComposeStore.getState = () => ({
    openCompose: vi.fn(),
    openReply: vi.fn(),
    openForward: vi.fn(),
  });
  useComposeStore.setState = vi.fn();
  return {
    useComposeStore,
    replaceSignatureInBody: (body: string) => body,
    removeSignatureFromBody: (body: string) => body,
    injectSignature: (body: string) => body,
  };
});

vi.mock("@/hooks/useCompose", () => ({
  useSendMessage: () => mockSendMessage,
  useSaveDraft: () => ({ isPending: false, mutate: vi.fn() }),
  useUploadAttachment: () => ({ isPending: false, mutate: vi.fn() }),
  useDeleteAttachment: () => ({ mutate: vi.fn() }),
  useDeleteDraft: () => ({ mutate: vi.fn() }),
}));

vi.mock("@/hooks/useIdentities", () => ({
  useIdentities: () => ({ data: [] }),
}));

vi.mock("@/hooks/useFolders", () => ({
  useFolders: () => ({
    data: {
      folders: [
        { name: "INBOX" },
        { name: "Archive" },
      ],
    },
  }),
  useCreateFolder: () => mockCreateFolder,
}));

vi.mock("@/hooks/useDisplayPreferences", () => ({
  useUpdateDisplayPreferences: () => ({ mutate: vi.fn() }),
}));

vi.mock("@/hooks/useMessages", () => ({
  useMessage: mockUseMessage,
  useUpdateFlags: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(async () => ({})) }),
  useMoveMessage: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(async () => ({})) }),
  useDeleteMessage: () => ({ mutate: vi.fn(), mutateAsync: vi.fn(async () => ({})) }),
}));

vi.mock("@/hooks/useTags", () => ({
  useTags: () => ({ data: { tags: [{ id: "t-1", name: "Work", color: "#0ea5e9" }] } }),
  useBulkAddTag: () => ({ mutate: vi.fn() }),
}));

vi.mock("@/hooks/useClickOutside", () => ({
  useClickOutside: vi.fn(),
}));

vi.mock("@/stores/useAuthStore", () => ({
  useAuthStore: Object.assign(
    (selector: (state: { activeAccount: () => { email: string } | null }) => unknown) =>
      selector({ activeAccount: () => ({ email: "me@example.com" }) }),
    {
      getState: () => ({ activeAccount: () => ({ email: "me@example.com" }) }),
    },
  ),
}));

vi.mock("@tanstack/react-query", () => ({
  useQueryClient: () => ({ clear: vi.fn() }),
}));

vi.mock("@/lib/api", () => ({
  apiPost: vi.fn(async () => ({ account: { id: "a-1", email: "a@example.com", imapHost: "imap", smtpHost: "smtp" } })),
  fetchAccounts: vi.fn(async () => ({ accounts: [] })),
}));

vi.mock("../ComposeDialog/RecipientInput", () => ({
  RecipientInput: ({ value, onChange }: { value: string; onChange: (value: string) => void }) => (
    <input value={value} onChange={(event) => onChange(event.target.value)} />
  ),
}));

vi.mock("../ComposeDialog/index", () => ({
  countRecipients: () => 0,
  formatFileSize: () => "1 KB",
  stripHtml: (value: string) => value,
  generateId: () => "draft-1",
  DiscardAlertDialog: () => null,
  AttachmentPreviewDialog: () => null,
}));

vi.mock("../TagPicker", () => ({
  TagPicker: () => <div data-testid="tag-picker" />,
}));

vi.mock("@/components/ui/button", () => ({
  Button: ({ children, ...props }: React.ButtonHTMLAttributes<HTMLButtonElement>) => (
    <button {...props}>{children}</button>
  ),
}));

vi.mock("@/components/ui/input", () => ({
  Input: ({ ...props }: React.InputHTMLAttributes<HTMLInputElement>) => <input {...props} />,
}));

vi.mock("@/components/ui/label", () => ({
  Label: ({ children }: { children: ReactNode }) => <span>{children}</span>,
}));

import { ComposeDialog } from "../ComposeDialog";
import { AddAccountModal } from "../../accounts/AddAccountModal";
import { CreateFolderDialog } from "../CreateFolderDialog";
import { CommandPalette } from "../../shared/CommandPalette";
import { MoveToFolderMenu } from "../MoveToFolderMenu";
import { MessageActionBar } from "../MessageActionBar";
import { BulkActionBar } from "../BulkActionBar";

function latestByTestId(testId: string) {
  const elements = screen.queryAllByTestId(testId);
  if (elements.length === 0) return null;
  return elements[elements.length - 1] ?? null;
}

function setMessage(flags: string[] = ["\\Seen"]) {
  mockUseMessage.mockReturnValue({
    data: {
      uid: 1,
      flags,
      subject: "Subject",
      raw_headers: "Message-ID: <id@example.com>",
      from_address: "sender@example.com",
      to_addresses: [{ address: "me@example.com", name: "Me" }],
      cc_addresses: [],
      date: "2026-03-14T00:00:00Z",
      text: "hello",
      html: "<p>hello</p>",
    },
  });
}

describe("Modal and action motion transitions", () => {
  it("applies ComposeDialog modal transitions and send-state feedback for non-off modes", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockComposeState.isOpen = true;
    mockSendMessage.isPending = true;

    const { rerender } = render(<ComposeDialog />);

    expect(screen.getByTestId("compose-dialog-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("compose-dialog-content-transition")).toBeTruthy();
    expect(screen.getByTestId("compose-send-feedback-transition")).toBeTruthy();
    const modalSerialized = [
      screen.getByTestId("compose-dialog-overlay-transition").getAttribute("data-motion-props") ?? "",
      screen.getByTestId("compose-dialog-content-transition").getAttribute("data-motion-props") ?? "",
      screen.getByTestId("compose-send-feedback-transition").getAttribute("data-motion-props") ?? "",
    ].join(" ");
    expect(modalSerialized.includes("repeat")).toBe(false);

    mockComposeState.isOpen = false;
    rerender(<ComposeDialog />);

    expect(latestByTestId("compose-dialog-content-transition")).toBeNull();

    mockComposeState.isOpen = true;
    rerender(<ComposeDialog />);
    expect(latestByTestId("compose-dialog-content-transition")).toBeTruthy();
  });

  it("uses static ComposeDialog path when animation mode is off", () => {
    mockUiState.effectiveAnimationMode = "off";
    mockComposeState.isOpen = true;
    mockSendMessage.isPending = false;

    render(<ComposeDialog />);

    expect(screen.queryByTestId("compose-dialog-overlay-transition")).toBeNull();
    expect(screen.queryByTestId("compose-dialog-content-transition")).toBeNull();
    expect(screen.queryByTestId("compose-send-feedback-transition")).toBeNull();
    expect(screen.getByText("Send")).toBeTruthy();
  });

  it("applies AddAccountModal transitions and keeps static off-mode path", () => {
    mockUiState.effectiveAnimationMode = "medium";
    const { rerender } = render(<AddAccountModal open onClose={vi.fn()} />);

    expect(screen.getByTestId("add-account-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("add-account-content-transition")).toBeTruthy();

    rerender(<AddAccountModal open={false} onClose={vi.fn()} />);
    expect(screen.queryByTestId("add-account-content-transition")).toBeNull();

    mockUiState.effectiveAnimationMode = "off";
    rerender(<AddAccountModal open onClose={vi.fn()} />);
    expect(screen.queryByTestId("add-account-overlay-transition")).toBeNull();
  });

  it("applies CreateFolderDialog transitions and keeps static off-mode path", () => {
    mockUiState.effectiveAnimationMode = "medium";
    const { rerender } = render(<CreateFolderDialog open onClose={vi.fn()} />);

    expect(screen.getByTestId("create-folder-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("create-folder-content-transition")).toBeTruthy();

    rerender(<CreateFolderDialog open={false} onClose={vi.fn()} />);
    expect(screen.queryAllByTestId("create-folder-content-transition").length).toBeGreaterThanOrEqual(1);

    mockUiState.effectiveAnimationMode = "off";
    rerender(<CreateFolderDialog open onClose={vi.fn()} />);
    expect(screen.queryByTestId("create-folder-content-transition")).toBeNull();
  });

  it("applies CommandPalette transitions and keeps static off-mode path", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.commandPaletteOpen = true;
    const { rerender } = render(<CommandPalette />);

    expect(screen.getByTestId("command-palette-overlay-transition")).toBeTruthy();
    expect(screen.getByTestId("command-palette-content-transition")).toBeTruthy();

    mockUiState.commandPaletteOpen = false;
    rerender(<CommandPalette />);
    expect(screen.queryByTestId("command-palette-content-transition")).toBeNull();

    mockUiState.effectiveAnimationMode = "off";
    mockUiState.commandPaletteOpen = true;
    rerender(<CommandPalette />);
    expect(screen.queryByTestId("command-palette-content-transition")).toBeNull();
  });

  it("applies MoveToFolderMenu transitions and keeps static off-mode path", () => {
    mockUiState.effectiveAnimationMode = "medium";
    const { rerender } = render(
      <MoveToFolderMenu currentFolder="INBOX" onMove={vi.fn()} />,
    );

    fireEvent.click(screen.getByText("Move to..."));
    expect(screen.getByTestId("move-to-folder-menu-transition")).toBeTruthy();

    fireEvent.click(screen.getByText("Move to..."));
    rerender(<MoveToFolderMenu currentFolder="INBOX" onMove={vi.fn()} />);
    expect(screen.queryByTestId("move-to-folder-menu-transition")).toBeNull();

    mockUiState.effectiveAnimationMode = "off";
    rerender(<MoveToFolderMenu currentFolder="INBOX" onMove={vi.fn()} />);
    fireEvent.click(screen.getByText("Move to..."));
    expect(screen.queryByTestId("move-to-folder-menu-transition")).toBeNull();
  });

  it("applies MessageActionBar feedback transitions for read/star/delete/archive/move actions", () => {
    mockUiState.effectiveAnimationMode = "medium";
    setMessage(["\\Seen", "\\Flagged"]);
    const { rerender } = render(<MessageActionBar />);

    expect(screen.getByTestId("message-action-bar-transition")).toBeTruthy();
    expect(screen.getByTestId("message-action-star-feedback-transition")).toBeTruthy();
    expect(screen.getByTestId("message-action-read-feedback-transition")).toBeTruthy();
    expect(screen.getByTestId("message-action-delete-feedback-transition")).toBeTruthy();
    expect(screen.getByTestId("message-action-archive-feedback-transition")).toBeTruthy();
    expect(screen.getByTestId("message-action-move-feedback-transition")).toBeTruthy();

    fireEvent.click(screen.getByText("Delete"));
    fireEvent.click(screen.getByText("Archive"));
    fireEvent.click(screen.getByText("Move to..."));

    const actionSerialized = [
      latestByTestId("message-action-star-feedback-transition")?.getAttribute("data-motion-props") ?? "",
      latestByTestId("message-action-read-feedback-transition")?.getAttribute("data-motion-props") ?? "",
      latestByTestId("message-action-delete-feedback-transition")?.getAttribute("data-motion-props") ?? "",
      latestByTestId("message-action-archive-feedback-transition")?.getAttribute("data-motion-props") ?? "",
      latestByTestId("message-action-move-feedback-transition")?.getAttribute("data-motion-props") ?? "",
    ].join(" ");
    expect(actionSerialized.includes("repeat")).toBe(false);

    mockUiState.effectiveAnimationMode = "off";
    rerender(<MessageActionBar />);
    expect(screen.queryByTestId("message-action-bar-transition")).toBeNull();
    expect(screen.queryByTestId("message-action-star-feedback-transition")).toBeNull();
    expect(screen.queryByTestId("message-action-delete-feedback-transition")).toBeNull();
    expect(screen.queryByTestId("message-action-archive-feedback-transition")).toBeNull();
    expect(screen.queryByTestId("message-action-move-feedback-transition")).toBeNull();
  });

  it("applies BulkActionBar mount/unmount transitions and keeps static off-mode path", () => {
    mockUiState.effectiveAnimationMode = "medium";
    mockUiState.selectedMessageUids = [...mockBulkUids];
    const { rerender } = render(<BulkActionBar />);

    expect(screen.getByTestId("bulk-action-bar-transition")).toBeTruthy();

    mockUiState.selectedMessageUids = [];
    rerender(<BulkActionBar />);
    expect(screen.queryAllByTestId("bulk-action-bar-transition").length).toBeGreaterThanOrEqual(1);

    mockUiState.effectiveAnimationMode = "off";
    mockUiState.selectedMessageUids = [...mockBulkUids];
    rerender(<BulkActionBar />);
    expect(screen.queryByTestId("bulk-action-bar-transition")).toBeNull();
    expect(screen.getByText("2 selected")).toBeTruthy();
  });
});
