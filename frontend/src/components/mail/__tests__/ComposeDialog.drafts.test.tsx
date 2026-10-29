import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const { send, save, identityQuery } = vi.hoisted(() => ({
  send: vi.fn(),
  save: vi.fn(),
  identityQuery: { data: [] as import("@/types/identity").Identity[] },
}));
vi.mock("@/hooks/useCompose", () => ({
  useSendMessage: () => ({ mutate: send, isPending: false }),
  useSaveDraft: () => ({ mutate: save, isPending: false }),
  useUploadAttachment: () => ({ mutate: vi.fn() }),
  useDeleteAttachment: () => ({ mutate: vi.fn() }),
  useDeleteDraft: () => ({ mutate: vi.fn() }),
}));
vi.mock("@/hooks/useIdentities", () => ({ useIdentities: () => identityQuery }));
vi.mock("../ComposeDialog/SubjectInput", () => ({ SubjectInput: () => <input aria-label="Subject" /> }));
vi.mock("../ComposeDialog/RecipientInput", () => ({ RecipientInput: () => <textarea aria-label="Recipients" /> }));
vi.mock("next/dynamic", () => ({ default: () => () => <div /> }));
import { ComposeDialog } from "../ComposeDialog";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";

beforeEach(() => {
  vi.useFakeTimers();
  send.mockReset();
  save.mockReset();
  identityQuery.data = [];
  useUiStore.setState({ effectiveAnimationMode: "off" });
  useComposeStore.getState().openDraft({ id: "saved-draft", to: "recipient@example.com", cc: "", bcc: "", subject: "Saved", body: "Body", inReplyTo: null, references: null, attachments: [], isHtml: false });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

it("sends a resumed draft with its saved ID and does not save another copy during undo", () => {
  render(<ComposeDialog />);
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  expect(send).not.toHaveBeenCalled();
  act(() => vi.advanceTimersByTime(5000));
  expect(send).toHaveBeenCalledTimes(1);
  expect(send.mock.calls[0][0]).toMatchObject({ draftId: "saved-draft", subject: "Saved" });
  expect(save).not.toHaveBeenCalled();
  act(() => send.mock.calls[0][1].onSuccess());
  expect(useComposeStore.getState().draftId).toBeNull();
});

it("preserves a draft and footer actions when maximizing and restoring", () => {
  useComposeStore.setState({
    to: Array.from({ length: 12 }, (_, index) => `recipient${index}@example.com`).join(", "),
    showCc: true,
    showBcc: true,
    attachments: Array.from({ length: 8 }, (_, index) => ({
      id: `attachment-${index}`,
      filename: `long-attachment-name-${index}.pdf`,
      contentType: "application/pdf",
      size: 1024,
    })),
  });
  render(<ComposeDialog />);
  const body = screen.getByPlaceholderText("Write your message...");
  fireEvent.change(body, { target: { value: "Updated draft" } });

  fireEvent.click(screen.getByTitle("Maximize"));
  expect(screen.getByPlaceholderText("Write your message...")).toBe(body);
  expect(useComposeStore.getState().body).toBe("Updated draft");
  expect(screen.getAllByTitle("Remove attachment")).toHaveLength(8);
  expect(screen.getByText("You are sending to more than 10 recipients.")).toBeTruthy();

  fireEvent.click(screen.getByTitle("Minimize"));
  fireEvent.click(screen.getByTitle("Save draft (Ctrl+S)"));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ textBody: "Updated draft" }), expect.anything());
  fireEvent.click(screen.getByRole("button", { name: "Discard" }));
  expect(screen.getByRole("alertdialog")).toBeTruthy();
});


it.each([
  { isHtml: true, signature: "", body: "" },
  { isHtml: true, signature: "", body: "<p><br></p><p>&nbsp;</p>" },
  { isHtml: false, signature: "", body: " \n\t" },
  { isHtml: true, signature: "<p>My signature</p>", body: "<p><br></p>" },
  { isHtml: false, signature: "<p>My signature</p>", body: "" },
])("does not save an empty compose (HTML=$isHtml, signature=$signature)", ({ isHtml, signature, body }) => {
  identityQuery.data = [{ id: 1, email: "me@example.com", display_name: "Me", signature_html: signature, is_default: true, created_at: "", updated_at: "" }];
  useComposeStore.getState().openCompose();
  useComposeStore.setState({ isHtml, body });
  render(<ComposeDialog />);

  fireEvent.click(screen.getByTitle("Save draft (Ctrl+S)"));
  // Simulate an editor update so autosave also checks the changed body.
  act(() => useComposeStore.getState().setField("body", useComposeStore.getState().body + " "));
  act(() => vi.advanceTimersByTime(30000));
  fireEvent.click(screen.getByTitle("Close"));

  expect(save).not.toHaveBeenCalled();
  expect(useComposeStore.getState().draftId).toBeNull();
  expect(useComposeStore.getState().isOpen).toBe(false);
});

it.each([
  { to: "recipient@example.com" },
  { cc: "cc@example.com" },
  { bcc: "bcc@example.com" },
  { subject: "Subject only" },
  { body: "<p>Body only</p>" },
  { body: '<p><img src="https://example.com/image.png"></p>' },
  { attachments: [{ id: "attachment", filename: "file.txt", contentType: "text/plain", size: 10 }] },
])("saves a compose with content on close: %o", (content) => {
  useComposeStore.getState().openCompose();
  useComposeStore.setState(content);
  render(<ComposeDialog />);
  fireEvent.click(screen.getByTitle("Close"));

  expect(save).toHaveBeenCalledTimes(1);
  expect(useComposeStore.getState().draftId).not.toBeNull();
  expect(useComposeStore.getState().isOpen).toBe(false);
});

it("saves user text along with the automatic signature", () => {
  identityQuery.data = [{ id: 1, email: "me@example.com", display_name: "Me", signature_html: "<p>My signature</p>", is_default: true, created_at: "", updated_at: "" }];
  useComposeStore.getState().openCompose();
  useComposeStore.setState({ body: "<p>My message</p>" });
  render(<ComposeDialog />);
  const body = useComposeStore.getState().body;
  fireEvent.click(screen.getByTitle("Close"));

  expect(save).toHaveBeenCalledWith(expect.objectContaining({ htmlBody: body }), expect.anything());
  expect(body).toContain("My message");
  expect(body).toContain("My signature");
});
