import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";

vi.hoisted(() => {
  vi.stubEnv("NEXT_PUBLIC_API_URL", "http://localhost:3001/oxi/api/");
  vi.stubEnv("NEXT_PUBLIC_BASE_PATH", "/frontend");
});

vi.mock("sonner", () => ({ toast: { error: vi.fn(), success: vi.fn() } }));

vi.mock("framer-motion", () => ({
  AnimatePresence: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock("@/lib/motion/AnimatedDiv", () => ({
  AnimatedDiv: ({ children, ...props }: React.ComponentProps<"div">) => <div {...props}>{children}</div>,
}));

vi.mock("@/hooks/useMessages", () => ({
  useMessage: () => ({ data: {
    uid: 7, folder: "Sent Items", subject: "Message", from_address: "a@test",
    to_addresses: [], cc_addresses: [], flags: ["\\Seen"], html: null, text: "Hello",
    date: "2026-01-01", attachments: [{ id: "image", filename: "received.png", content_type: "image/png", size: 1 }],
  }, refetch: vi.fn() }),
  useUpdateFlags: () => ({ mutate: vi.fn() }),
}));
vi.mock("@tanstack/react-query", () => ({ useQueryClient: () => ({ invalidateQueries: vi.fn() }) }));
vi.mock("../EmailRenderer", () => ({ EmailRenderer: () => null, hasRemoteResources: () => false }));
vi.mock("../ThreadView", () => ({ ThreadView: () => null }));
vi.mock("@/hooks/useIdentities", () => ({ useIdentities: () => ({ data: [] }) }));
vi.mock("../ComposeDialog/RecipientInput", () => ({ RecipientInput: () => null }));
vi.mock("@/hooks/useSubjectSuggestions", () => ({ useSubjectSuggestions: () => [] }));

vi.mock("@/hooks/useCompose", () => ({
  useSendMessage: () => ({ mutate: vi.fn() }),
  useSaveDraft: () => ({ mutate: vi.fn() }),
  useDeleteDraft: () => ({ mutate: vi.fn() }),
  useDeleteAttachment: () => ({ mutate: vi.fn() }),
  useUploadAttachment: () => ({ mutate: (_request: unknown, options: { onSuccess: (data: unknown) => void }) => {
    options.onSuccess({ attachments: [{ id: "inline", filename: "inline.png", content_type: "image/png", size: 1 }] });
  } }),
}));
vi.mock("next/dynamic", () => ({ default: () => function Editor({ onImageUpload }: { onImageUpload: (file: File) => Promise<string | null> }) {
  return <button onClick={async () => {
    const src = await onImageUpload(new File(["image"], "inline.png"));
    if (src) {
      const image = document.createElement("img");
      image.alt = "uploaded inline";
      image.src = src;
      document.body.append(image);
    }
  }}>Upload inline</button>;
} }));
vi.mock("@/hooks/useContacts", () => ({
  useContacts: () => ({ data: { contacts: [], total: 0 } }),
  useCreateContact: () => ({ mutate: vi.fn() }),
  useDeleteContact: () => ({ mutate: vi.fn() }),
  useImportContacts: () => ({ mutate: vi.fn() }),
}));
vi.mock("@/hooks/useContactGroups", () => ({
  useContactGroups: () => ({ data: { groups: [] } }),
  useGroupMembers: () => ({ data: [] }),
  useCreateGroup: () => ({ mutate: vi.fn() }),
  useUpdateGroup: () => ({ mutate: vi.fn() }),
  useDeleteGroup: () => ({ mutate: vi.fn() }),
}));
vi.mock("@/components/contacts/ContactDialog", () => ({ ContactDialog: () => null }));
vi.mock("@/components/contacts/GroupDialog", () => ({ GroupDialog: () => null }));
vi.mock("@/hooks/useCalendar", () => ({ useCreateEvent: () => ({ mutateAsync: vi.fn() }) }));

import { ApiDownloadLink } from "@/components/shared/ApiDownloadLink";
import { ReadingPane } from "../ReadingPane";
import { ComposeDialog } from "../ComposeDialog";
import { IcsPreview } from "../ReadingPane/IcsPreview";
import { ContactsPanel } from "@/components/contacts/ContactsPanel";
import { useAuthStore } from "@/stores/useAuthStore";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";

const apiRoot = "http://localhost:3001/oxi/api";
beforeEach(() => {
  useAuthStore.setState({ activeAccountId: "account+1" });
  useUiStore.setState({ effectiveAnimationMode: "off", activeFolder: "Sent Items", selectedMessageUid: 7 });
});
afterEach(() => {
  cleanup();
  document.querySelector('img[alt="uploaded inline"]')?.remove();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("separate-origin media and export smoke", () => {
  it("points received previews and downloads at the backend with the selected account", async () => {
    render(<ReadingPane />);
    fireEvent.click(screen.getByText("received.png"));
    const expected = `${apiRoot}/messages/Sent%20Items/7/attachments/image?account_id=account%2B1`;
    expect(screen.getByAltText("received.png").getAttribute("src")).toBe(expected);
    const download = screen.getByTitle("Download");
    expect(download.getAttribute("href")).toBe(expected);
    const blob = new Blob(["image"], { type: "image/png" });
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, blob: async () => blob });
    vi.stubGlobal("fetch", fetchMock);
    const createObjectURL = vi.fn().mockReturnValue("blob:download");
    const revokeObjectURL = vi.fn();
    vi.stubGlobal("URL", Object.assign(URL, { createObjectURL, revokeObjectURL }));
    const save = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
    fireEvent.click(download);
    await waitFor(() => expect(save).toHaveBeenCalled());
    expect(fetchMock).toHaveBeenCalledWith(expected, { credentials: "include" });
    expect(createObjectURL).toHaveBeenCalledWith(blob);
    expect((save.mock.instances[0] as HTMLAnchorElement).download).toBe("received.png");
    await waitFor(() => expect(revokeObjectURL).toHaveBeenCalledWith("blob:download"));
  });

  it("points draft previews, downloads and inline uploads at the backend", async () => {
    useComposeStore.setState({ isOpen: true, draftId: "draft", isHtml: true, signatureEnabled: false,
      attachments: [{ id: "image", filename: "draft.png", contentType: "image/png", size: 1 }] });
    render(<ComposeDialog />);
    fireEvent.click(screen.getByText("draft.png"));
    const expected = `${apiRoot}/drafts/draft/attachments/image/content?account_id=account%2B1`;
    expect(screen.getByAltText("draft.png").getAttribute("src")).toBe(expected);
    expect(screen.getByTitle("Download").getAttribute("href")).toBe(expected);
    fireEvent.click(within(screen.getByAltText("draft.png").closest('[role="dialog"]') as HTMLElement).getByTitle("Close"));
    fireEvent.click(screen.getByText("Upload inline"));
    await waitFor(() => expect(screen.getByAltText("uploaded inline").getAttribute("src"))
      .toBe(`${apiRoot}/drafts/draft/attachments/inline/content?account_id=account%2B1`));
  });

  it("exports contacts from the backend using the active account", () => {
    const open = vi.spyOn(window, "open").mockImplementation(() => null);
    render(<ContactsPanel />);
    fireEvent.click(screen.getByTitle("Export contacts (.vcf)"));
    expect(open).toHaveBeenCalledWith(`${apiRoot}/contacts/export?account_id=account%2B1`, "_blank");
  });

  it("reports a failed download rather than navigating away", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: false, status: 401 });
    vi.stubGlobal("fetch", fetchMock);
    render(<ApiDownloadLink href={`${apiRoot}/file`} download="file.pdf">Download file</ApiDownloadLink>);
    fireEvent.click(screen.getByText("Download file"));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("Download failed (401)"));
  });

  it("includes cookies when fetching an ICS preview", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: true, text: async () => "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Meeting\r\nDTSTART:20260101T120000Z\r\nEND:VEVENT\r\nEND:VCALENDAR" });
    vi.stubGlobal("fetch", fetchMock);
    const url = `${apiRoot}/messages/INBOX/7/attachments/event?account_id=account%2B1`;
    render(<IcsPreview url={url} filename="event.ics" />);
    await screen.findByText("Meeting");
    expect(fetchMock).toHaveBeenCalledWith(url, { credentials: "include" });
  });
});
