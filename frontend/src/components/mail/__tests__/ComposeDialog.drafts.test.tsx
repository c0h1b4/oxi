import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const { send, save, notify, identityQuery } = vi.hoisted(() => ({
  send: vi.fn(),
  notify: Object.assign(vi.fn(), { success: vi.fn(), error: vi.fn(), dismiss: vi.fn() }),
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
vi.mock("sonner", () => ({ toast: notify }));
vi.mock("../ComposeDialog/SubjectInput", () => ({ SubjectInput: () => <input aria-label="Subject" /> }));
vi.mock("../ComposeDialog/RecipientInput", () => ({ RecipientInput: () => <textarea aria-label="Recipients" /> }));
vi.mock("next/dynamic", () => ({ default: () => RichTextEditor }));
import { RichTextEditor } from "../RichTextEditor";
import type { Editor } from "@tiptap/react";
import { ComposeDialog } from "../ComposeDialog";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";

beforeEach(() => {
  vi.useFakeTimers();
  send.mockReset();
  save.mockReset();
  notify.mockReset();
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
  { isHtml: true, signature: "<p>My signature</p>", body: "" },
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

it.each(["close", "manual save", "autosave", "discard"])("preserves text typed inside the initialized signature on %s", (action) => {
  identityQuery.data = [{ id: 1, email: "me@example.com", display_name: "Me", signature_html: "<p>My signature</p>", is_default: true, created_at: "", updated_at: "" }];
  useComposeStore.getState().openCompose();
  useComposeStore.setState({ isHtml: true });
  const { container } = render(<ComposeDialog />);
  const editor = (container.ownerDocument.querySelector(".tiptap") as HTMLElement & { editor: Editor }).editor;
  expect(editor.getHTML()).toContain('data-signature="true"');
  act(() => {
    editor.commands.setTextSelection(2);
    editor.view.dispatch(editor.state.tr.insertText("My important message"));
  });
  const body = useComposeStore.getState().body;
  expect(new DOMParser().parseFromString(body, "text/html").querySelector('[data-signature="true"]')?.textContent).toContain("My important message");

  if (action === "discard") {
    fireEvent.click(screen.getByRole("button", { name: "Discard" }));
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expect(useComposeStore.getState().body).toBe(body);
  } else {
    if (action === "close") fireEvent.click(screen.getByTitle("Close"));
    if (action === "manual save") fireEvent.click(screen.getByTitle("Save draft (Ctrl+S)"));
    if (action === "autosave") act(() => vi.advanceTimersByTime(30000));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ htmlBody: body }), expect.anything());
  }
});

const listHtml = '<ol start="3"><li><p>First item</p><ul><li><p>Nested bullet</p></li></ul></li></ol><ul><li><p>Another bullet</p></li></ul>';

function openHtmlDraft(body = listHtml) {
  useComposeStore.setState({ isHtml: true, body });
  render(<ComposeDialog />);
}

function renderedEditor() {
  return (document.querySelector(".tiptap") as HTMLElement & { editor: Editor }).editor;
}

it.each(["new message", "reply", "forward", "draft", "reset"])("clears HTML source mode for a fresh %s", (nextCompose) => {
  openHtmlDraft();
  fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
  expect(useComposeStore.getState().editingHtmlSource).toBe(true);
  fireEvent.click(screen.getByTitle("Close"));

  act(() => {
    const store = useComposeStore.getState();
    if (nextCompose === "new message") store.openCompose();
    if (nextCompose === "reply") store.openReply({ to: "sender@example.com", cc: "", subject: "Re: Hello", body: "<p>Reply</p>", inReplyTo: null, references: null });
    if (nextCompose === "forward") store.openForward({ subject: "Fwd: Hello", body: "<p>Forward</p>" });
    if (nextCompose === "draft") store.openDraft({ id: "another-draft", to: "", cc: "", bcc: "", subject: "", body: "<p>Draft</p>", inReplyTo: null, references: null, attachments: [] });
    if (nextCompose === "reset") {
      store.reset();
      useComposeStore.setState({ isOpen: true });
    }
  });

  expect(useComposeStore.getState().editingHtmlSource).toBe(false);
  expect(screen.getByRole("button", { name: "HTML source" }).getAttribute("aria-pressed")).toBe("false");
  expect(screen.queryByRole("textbox", { name: "HTML source" })).toBeNull();
  expect(document.querySelector(".tiptap")).not.toBeNull();
});

it("preserves HTML source mode and raw HTML when undoing send", () => {
  openHtmlDraft();
  fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
  const html = '<table style="color: red"><tr><td>Keep raw HTML</td></tr></table>';
  fireEvent.change(screen.getByRole("textbox", { name: "HTML source" }), { target: { value: html } });
  fireEvent.click(screen.getByRole("button", { name: "Send" }));

  const undoSend = notify.mock.calls.find(([message]) => message === "Sending message...")![1];
  act(() => undoSend.action.onClick());
  act(() => vi.advanceTimersByTime(5000));

  expect(send).not.toHaveBeenCalled();
  expect(useComposeStore.getState().editingHtmlSource).toBe(true);
  expect((screen.getByRole("textbox", { name: "HTML source" }) as HTMLTextAreaElement).value).toBe(html);
});

it("explains the risk of rich-text edits beside the HTML source", () => {
  openHtmlDraft();
  fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
  const source = screen.getByRole("textbox", { name: "HTML source" });
  const hint = document.getElementById(source.getAttribute("aria-describedby")!);
  expect(hint?.textContent).toContain("Rich-text edits may remove unsupported HTML");
  expect(hint?.textContent).toContain("tables, styles, and custom attributes");
});

it("edits HTML source and rich text in both directions without losing lists", () => {
  openHtmlDraft();
  const toggle = screen.getByRole("button", { name: "HTML source" });
  expect(toggle.getAttribute("aria-pressed")).toBe("false");
  fireEvent.click(toggle);
  const source = screen.getByRole("textbox", { name: "HTML source" });
  expect((source as HTMLTextAreaElement).value).toBe(listHtml);
  expect(toggle.getAttribute("aria-pressed")).toBe("true");

  const editedHtml = '<p>Updated from source</p>' + listHtml;
  fireEvent.change(source, { target: { value: editedHtml } });
  expect(useComposeStore.getState().body).toBe(editedHtml);
  fireEvent.click(toggle);
  expect(document.querySelector(".tiptap ol")?.getAttribute("start")).toBe("3");
  expect(document.querySelector(".tiptap ol ul li")?.textContent).toBe("Nested bullet");
  expect(document.querySelector(".tiptap")?.textContent).toContain("Updated from source");

  const editor = renderedEditor();
  act(() => {
    editor.commands.setTextSelection(1);
    editor.commands.insertContent("Rich-text edit: ");
  });
  const richHtml = editor.getHTML();
  expect(richHtml).toContain("Rich-text edit: Updated from source");
  fireEvent.click(toggle);
  expect((screen.getByRole("textbox", { name: "HTML source" }) as HTMLTextAreaElement).value).toBe(richHtml);
  expect(richHtml).toContain('<ol start="3">');
  expect(richHtml).toContain("<ul>");
});

it("does not rewrite raw HTML when switching modes without rich-text edits", () => {
  openHtmlDraft();
  const toggle = screen.getByRole("button", { name: "HTML source" });
  fireEvent.click(toggle);
  const html = '<p style="color: red" data-custom="keep">Keep source formatting</p>' + listHtml;
  fireEvent.change(screen.getByRole("textbox", { name: "HTML source" }), { target: { value: html } });
  fireEvent.click(toggle);
  fireEvent.click(toggle);
  expect((screen.getByRole("textbox", { name: "HTML source" }) as HTMLTextAreaElement).value).toBe(html);
});

it.each(["source", "rich text"])("sends the latest list HTML while editing %s", (mode) => {
  openHtmlDraft();
  const toggle = screen.getByRole("button", { name: "HTML source" });
  fireEvent.click(toggle);
  const html = '<p>Latest source edit</p>' + listHtml;
  fireEvent.change(screen.getByRole("textbox", { name: "HTML source" }), { target: { value: html } });
  if (mode === "rich text") {
    fireEvent.click(toggle);
    act(() => {
      renderedEditor().commands.setTextSelection(1);
      renderedEditor().commands.insertContent("Latest rendered edit: ");
    });
  }
  const latestBody = useComposeStore.getState().body;
  fireEvent.click(screen.getByRole("button", { name: "Send" }));
  act(() => vi.advanceTimersByTime(5000));
  expect(send).toHaveBeenCalledWith(expect.objectContaining({
    htmlBody: latestBody,
    body: expect.stringContaining("Latest source edit"),
    draftId: "saved-draft",
  }), expect.anything());
  expect(latestBody).toContain("<ol");
  expect(latestBody).toContain("<ul>");
  if (mode === "rich text") expect(latestBody).toContain("Latest rendered edit:");
});

it.each(["manual save", "autosave", "close"])("saves the latest raw HTML on %s", (action) => {
  openHtmlDraft();
  fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
  const html = '<p>Latest unswitched source</p>' + listHtml;
  fireEvent.change(screen.getByRole("textbox", { name: "HTML source" }), { target: { value: html } });
  if (action === "manual save") fireEvent.click(screen.getByTitle("Save draft (Ctrl+S)"));
  if (action === "autosave") act(() => vi.advanceTimersByTime(30000));
  if (action === "close") fireEvent.click(screen.getByTitle("Close"));
  expect(save).toHaveBeenCalledWith(expect.objectContaining({ htmlBody: html }), expect.anything());
});

it("keeps plain-text conversion separate from HTML source editing", () => {
  openHtmlDraft();
  fireEvent.click(screen.getByRole("button", { name: "HTML source" }));
  fireEvent.click(screen.getByTitle("Switch to plain text"));
  expect(screen.queryByRole("button", { name: "HTML source" })).toBeNull();
  expect(useComposeStore.getState().isHtml).toBe(false);
  expect(screen.getByRole("textbox", { name: "Message body" })).toBeTruthy();
  fireEvent.click(screen.getByTitle("Switch to rich text"));
  expect(screen.getByRole("button", { name: "HTML source" }).getAttribute("aria-pressed")).toBe("false");
  expect(document.querySelector(".tiptap")).not.toBeNull();
});
