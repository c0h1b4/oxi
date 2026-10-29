import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const { send, save } = vi.hoisted(() => ({ send: vi.fn(), save: vi.fn() }));
vi.mock("@/hooks/useCompose", () => ({
  useSendMessage: () => ({ mutate: send, isPending: false }),
  useSaveDraft: () => ({ mutate: save, isPending: false }),
  useUploadAttachment: () => ({ mutate: vi.fn() }),
  useDeleteAttachment: () => ({ mutate: vi.fn() }),
  useDeleteDraft: () => ({ mutate: vi.fn() }),
}));
vi.mock("@/hooks/useIdentities", () => ({ useIdentities: () => ({ data: [] }) }));
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
