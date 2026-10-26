import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Identity } from "@/types/identity";

const { identityQuery } = vi.hoisted(() => ({ identityQuery: { data: undefined as Identity[] | undefined } }));
vi.mock("@/hooks/useIdentities", () => ({ useIdentities: () => identityQuery }));
vi.mock("@/hooks/useCompose", () => {
  const mutation = () => ({ isPending: false, mutate: vi.fn() });
  return { useSendMessage: mutation, useSaveDraft: mutation, useUploadAttachment: mutation, useDeleteAttachment: mutation, useDeleteDraft: mutation };
});
vi.mock("next/dynamic", () => ({
  default: () => ({ content }: { content: string }) => <div data-testid="editor">{content}</div>,
}));
vi.mock("../ComposeDialog/RecipientInput", () => ({ RecipientInput: () => <input aria-label="Recipient" /> }));

import { ComposeDialog } from "../ComposeDialog";
import { useComposeStore } from "@/stores/useComposeStore";
import { useUiStore } from "@/stores/useUiStore";

const identities: Identity[] = [
  { id: 1, email: "me@example.com", display_name: "Me", signature_html: "<p>Main signature</p>", is_default: true, created_at: "", updated_at: "" },
  { id: 2, email: "alias@example.com", display_name: "Alias", signature_html: "<p>Alias signature</p>", is_default: false, created_at: "", updated_at: "" },
];
const reply = { to: "sender@example.com", cc: "", subject: "Re: Hello", body: "<p><br></p><blockquote><p>Original</p></blockquote>", inReplyTo: "<original>", references: null, fromIdentityId: 2 };

beforeEach(() => {
  useComposeStore.getState().reset();
  useUiStore.setState({ effectiveAnimationMode: "off" });
  identityQuery.data = identities;
});
afterEach(cleanup);

describe("ComposeDialog reply signatures", () => {
  it("initializes a matched reply when identities arrive and does not duplicate on rerender", () => {
    identityQuery.data = undefined;
    useComposeStore.getState().openReply(reply);
    const { rerender } = render(<ComposeDialog />);
    expect(useComposeStore.getState().body).not.toContain("signature");
    identityQuery.data = identities;
    rerender(<ComposeDialog />);
    expect(useComposeStore.getState().body).toContain("Alias signature");
    rerender(<ComposeDialog />);
    expect(useComposeStore.getState().body.match(/data-signature=/g)).toHaveLength(1);
  });

  it("initializes another reply even when the dialog stays open with the same identity", () => {
    useComposeStore.getState().openReply(reply);
    render(<ComposeDialog />);
    act(() => useComposeStore.getState().openReply({ ...reply, body: "<blockquote><p>Second message</p></blockquote>" }));
    expect(useComposeStore.getState().body).toContain("Second message");
    expect(useComposeStore.getState().body).toContain("Alias signature");
    expect(useComposeStore.getState().body.match(/data-signature=/g)).toHaveLength(1);
  });

  it.each([false, true])("removes and replaces a signature after format switching (round trip=%s)", (roundTrip) => {
    useComposeStore.getState().openReply(reply);
    render(<ComposeDialog />);
    fireEvent.click(screen.getByTitle("Switch to plain text"));
    if (roundTrip) fireEvent.click(screen.getByTitle("Switch to rich text"));
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "1" } });
    expect(useComposeStore.getState().body).not.toContain("Alias signature");
    expect(useComposeStore.getState().body.match(/Main signature/g)).toHaveLength(1);
    expect(useComposeStore.getState().body).toContain("Original");
    fireEvent.click(screen.getByTitle("Remove signature"));
    expect(useComposeStore.getState().body).not.toContain("Main signature");
    expect(useComposeStore.getState().body).toContain("Original");
    fireEvent.click(screen.getByTitle("Add signature"));
    expect(useComposeStore.getState().body.match(/Main signature/g)).toHaveLength(1);
  });

  it("preserves quoted signatures and literal text over a plain-text round trip", () => {
    useComposeStore.getState().openReply({ ...reply, isHtml: false,
      body: "Reply <literal> & text\n\nOn Monday, sender wrote:\n> Original\n> -- \n> Alias signature" });
    render(<ComposeDialog />);
    fireEvent.click(screen.getByTitle("Switch to rich text"));
    expect(useComposeStore.getState().body).toContain("&lt;literal&gt; &amp; text");
    expect(useComposeStore.getState().body.match(/data-signature=/g)).toHaveLength(1);
    fireEvent.click(screen.getByTitle("Switch to plain text"));
    fireEvent.click(screen.getByTitle("Remove signature"));
    const body = useComposeStore.getState().body;
    expect(body).toContain("Reply <literal> & text");
    expect(body).toContain("> Alias signature");
    expect(body.match(/Alias signature/g)).toHaveLength(1);
  });

  it("uses the newly selected identity when re-enabling the signature", () => {
    useComposeStore.getState().openReply(reply);
    render(<ComposeDialog />);
    fireEvent.click(screen.getByTitle("Remove signature"));
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "1" } });
    expect(useComposeStore.getState().body).not.toContain("signature");
    fireEvent.click(screen.getByTitle("Add signature"));
    expect(useComposeStore.getState().body).toContain("Main signature");
    expect(useComposeStore.getState().body).not.toContain("Alias signature");
  });
});
