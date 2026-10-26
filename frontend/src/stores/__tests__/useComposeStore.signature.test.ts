import { beforeEach, describe, expect, it } from "vitest";
import { useComposeStore, injectSignature, replaceSignatureInBody, removeSignatureFromBody } from "../useComposeStore";
import { buildReplyBody, buildReplyBodyHtml } from "@/lib/email-utils";
import type { Identity } from "@/types/identity";

const identities: Identity[] = [
  { id: 1, email: "me@example.com", display_name: "Me", signature_html: "<p>Default signature</p>", is_default: true, created_at: "", updated_at: "" },
  { id: 2, email: "alias@example.com", display_name: "Alias", signature_html: "<div><p>Alias signature</p><div>Company</div></div>", is_default: false, created_at: "", updated_at: "" },
];
const date = "2026-10-26T12:00:00Z";

beforeEach(() => useComposeStore.getState().reset());

describe("reply signature initialization", () => {
  it.each(["", "other@example.com"])("uses the matched identity for reply/reply-all (cc=%s)", (cc) => {
    const store = useComposeStore.getState();
    store.openReply({ to: "sender@example.com", cc, subject: "Re: Hello", body: buildReplyBodyHtml("<p>Original</p>", "sender@example.com", date), inReplyTo: "<original>", references: null, fromIdentityId: 2 });
    store.initializeSignature(identities);
    const { body, signatureHtml } = useComposeStore.getState();
    expect(body).toContain("Alias signature");
    expect(body).not.toContain("Default signature");
    expect(body.indexOf("Alias signature")).toBeLessThan(body.indexOf("<blockquote"));
    expect(signatureHtml).toBe(identities[1].signature_html);
    store.initializeSignature(identities);
    expect(useComposeStore.getState().body).toBe(body);
  });

  it("waits for identities and initializes the default exactly once across close/reopen", () => {
    const store = useComposeStore.getState();
    store.openCompose();
    store.initializeSignature([]);
    expect(useComposeStore.getState().signatureInitialized).toBe(false);
    store.initializeSignature(identities);
    const body = useComposeStore.getState().body;
    store.closeCompose();
    useComposeStore.setState({ isOpen: true });
    store.initializeSignature(identities);
    expect(useComposeStore.getState().body).toBe(body);
    expect(useComposeStore.getState().fromIdentityId).toBe(1);
  });

  it("adds readable plain-text signature before the quote", () => {
    const store = useComposeStore.getState();
    store.openReply({ to: "sender@example.com", cc: "", subject: "Re: Hello", body: buildReplyBody("Original", "sender@example.com", date), inReplyTo: null, references: null, fromIdentityId: 2, isHtml: false });
    store.initializeSignature(identities);
    const { body } = useComposeStore.getState();
    expect(body).toContain("-- \nAlias signature\nCompany");
    expect(body).not.toContain("<div");
    expect(body.indexOf("Alias signature")).toBeLessThan(body.indexOf("On "));
    expect(removeSignatureFromBody(body, false, identities[1].signature_html)).not.toContain("Alias signature");
  });

  it("does not modify a resumed draft or a disabled signature", () => {
    const store = useComposeStore.getState();
    store.openDraft({ id: "draft", to: "sender@example.com", cc: "", bcc: "", subject: "Re: Hello", body: "Saved edited body", inReplyTo: null, references: null, attachments: [] });
    store.initializeSignature(identities);
    expect(useComposeStore.getState().body).toBe("Saved edited body");
    store.openCompose();
    store.setSignatureEnabled(false);
    store.initializeSignature(identities);
    expect(useComposeStore.getState().body).toBe("");
    expect(useComposeStore.getState().signatureHtml).toContain("Default signature");
  });
});

describe("signature editing", () => {
  it("replaces/removes nested signatures without altering quoted signatures", () => {
    const quote = '<blockquote><p>Original</p><div data-signature="true"><p>Their signature</p></div></blockquote>';
    const body = injectSignature("<p>Reply</p>" + quote, identities[1].signature_html);
    expect(injectSignature(body, identities[1].signature_html)).toBe(body);
    const replaced = replaceSignatureInBody(body, identities[0].signature_html);
    expect(replaced).not.toContain("Alias signature");
    expect(replaced).not.toContain("Company");
    expect(replaced).toContain("Default signature");
    expect(replaced).toContain("Their signature");
    const removed = removeSignatureFromBody(replaced);
    expect(removed).toBe("<p>Reply</p>" + quote);
  });
});
