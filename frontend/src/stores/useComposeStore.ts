"use client";

import { create } from "zustand";
import { useUiStore } from "@/stores/useUiStore";
import type { Identity } from "@/types/identity";
import { htmlToPlainText, plainTextToHtml } from "@/lib/compose-format";

export interface ReplyParams {
  to: string;
  cc: string;
  subject: string;
  body: string;
  inReplyTo: string | null;
  references: string | null;
  fromIdentityId?: number | null;
  isHtml?: boolean;
}

export interface ForwardParams {
  subject: string;
  body: string;
  isHtml?: boolean;
}

export interface DraftResumeParams {
  id: string;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  inReplyTo: string | null;
  references: string | null;
  attachments: ComposeAttachment[];
  isHtml?: boolean;
}

export interface ComposeAttachment {
  id: string;
  filename: string;
  contentType: string;
  size: number;
}

interface ComposeState {
  isOpen: boolean;
  to: string;
  cc: string;
  bcc: string;
  subject: string;
  body: string;
  inReplyTo: string | null;
  references: string | null;
  draftId: string | null;
  showCc: boolean;
  showBcc: boolean;
  attachments: ComposeAttachment[];
  fromIdentityId: number | null;
  isHtml: boolean;
  signatureHtml: string;
  signatureEnabled: boolean;
  signatureInitialized: boolean;
  initializeSignature: (identities: Identity[]) => void;

  openCompose: () => void;
  openReply: (params: ReplyParams) => void;
  openForward: (params: ForwardParams) => void;
  openDraft: (params: DraftResumeParams) => void;
  closeCompose: () => void;
  setField: (field: "to" | "cc" | "bcc" | "subject" | "body", value: string) => void;
  setShowCc: (show: boolean) => void;
  setShowBcc: (show: boolean) => void;
  setDraftId: (id: string) => void;
  setFromIdentityId: (id: number | null) => void;
  setIsHtml: (v: boolean) => void;
  setSignatureHtml: (html: string) => void;
  setSignatureEnabled: (enabled: boolean) => void;
  addAttachments: (atts: ComposeAttachment[]) => void;
  removeAttachment: (id: string) => void;
  reset: () => void;
}

/** Wraps signature HTML in the standard signature block marker. */
export function wrapSignatureHtml(signatureHtml: string): string {
  if (!signatureHtml) return "";
  return `<div class="email-signature" data-signature="true"><p>-- </p>${signatureHtml}</div>`;
}

function signatureText(html: string): string {
  return htmlToPlainText(html).trim();
}

function plainSignature(signatureHtml: string): string {
  return `-- \n${signatureText(signatureHtml)}\n\n`;
}

function findPlainSignature(body: string, signatureHtml: string): { start: number; end: number } | null {
  if (!signatureHtml) return null;
  const signature = plainSignature(signatureHtml).replace(/\n+$/, "");
  const start = body.indexOf(signature);
  if (start === -1 || (start > 0 && body[start - 1] !== "\n")) return null;
  const end = start + signature.length;
  if (end < body.length && body[end] !== "\n") return null;
  return { start, end: end + (body.slice(end).match(/^\n{0,2}/)?.[0].length ?? 0) };
}

/** Keep the editable signature identifiable in both compose formats. */
export function convertBodyFormat(body: string, isHtml: boolean, signatureHtml: string): string {
  if (isHtml) {
    const doc = new DOMParser().parseFromString(body, "text/html");
    doc.querySelectorAll('[data-signature="true"]').forEach(el => {
      if (!el.closest("blockquote")) {
        // Tiptap normalizes away the delimiter's trailing space.
        const text = htmlToPlainText(el.innerHTML).replace(/^--[ \t]*\n/, "-- \n");
        el.replaceWith(doc.createTextNode(text + "\n\n"));
      }
    });
    return htmlToPlainText(doc.body.innerHTML);
  }
  const signature = findPlainSignature(body, signatureHtml);
  if (!signature) return plainTextToHtml(body);
  const signatureBody = plainTextToHtml(signatureText(signatureHtml));
  return plainTextToHtml(body.slice(0, signature.start))
    + wrapSignatureHtml(signatureBody)
    + plainTextToHtml(body.slice(signature.end));
}

/** Inject before the quote, keeping plain-text messages free of HTML. */
export function injectSignature(body: string, signatureHtml: string, isHtml = true): string {
  if (!signatureHtml) return body;
  if (!isHtml) {
    const signature = plainSignature(signatureHtml);
    const quoteIndex = body.search(/^(?:On .+ wrote:|> ?.*)$/m);
    return quoteIndex === -1
      ? `${body}\n\n${signature}`
      : body.slice(0, quoteIndex) + signature + body.slice(quoteIndex);
  }

  const doc = new DOMParser().parseFromString(body, "text/html");
  // A signature inside quoted mail belongs to the original sender.
  if (Array.from(doc.querySelectorAll('[data-signature="true"]')).some((el) => !el.closest("blockquote"))) {
    return body;
  }
  const quote = doc.querySelector("blockquote");
  if (quote) {
    quote.insertAdjacentHTML("beforebegin", wrapSignatureHtml(signatureHtml));
  } else {
    doc.body.insertAdjacentHTML("beforeend", wrapSignatureHtml(signatureHtml));
  }
  return doc.body.innerHTML;
}

/** Remove only our own signature, preserving quoted signatures and nested HTML. */
export function removeSignatureFromBody(body: string, isHtml = true, previousSignatureHtml = ""): string {
  if (!isHtml) {
    const signature = findPlainSignature(body, previousSignatureHtml);
    return signature ? body.slice(0, signature.start) + body.slice(signature.end) : body;
  }
  const doc = new DOMParser().parseFromString(body, "text/html");
  doc.querySelectorAll('[data-signature="true"]').forEach((el) => {
    if (!el.closest("blockquote")) el.remove();
  });
  return doc.body.innerHTML;
}

export function replaceSignatureInBody(body: string, newSignatureHtml: string, isHtml = true, previousSignatureHtml = ""): string {
  return injectSignature(removeSignatureFromBody(body, isHtml, previousSignatureHtml), newSignatureHtml, isHtml);
}

const initialState = {
  isOpen: false,
  to: "",
  cc: "",
  bcc: "",
  subject: "",
  body: "",
  inReplyTo: null as string | null,
  references: null as string | null,
  draftId: null as string | null,
  showCc: false,
  showBcc: false,
  attachments: [] as ComposeAttachment[],
  fromIdentityId: null as number | null,
  isHtml: true,
  signatureHtml: "",
  signatureEnabled: true,
  signatureInitialized: false,
};

export const useComposeStore = create<ComposeState>((set) => ({
  ...initialState,

  openCompose: () => set({
    ...initialState,
    isOpen: true,
    isHtml: useUiStore.getState().composeFormat !== "text",
  }),

  openReply: (params) =>
    set({
      ...initialState,
      isOpen: true,
      to: params.to,
      cc: params.cc,
      subject: params.subject,
      body: params.body,
      inReplyTo: params.inReplyTo,
      references: params.references,
      showCc: params.cc.length > 0,
      fromIdentityId: params.fromIdentityId ?? null,
      isHtml: params.isHtml ?? true,
    }),

  openForward: (params) =>
    set({
      ...initialState,
      isOpen: true,
      subject: params.subject,
      body: params.body,
      isHtml: params.isHtml ?? true,
    }),

  openDraft: (params) =>
    set({
      ...initialState,
      isOpen: true,
      draftId: params.id,
      to: params.to,
      cc: params.cc,
      bcc: params.bcc,
      subject: params.subject,
      body: params.body,
      inReplyTo: params.inReplyTo,
      references: params.references,
      showCc: params.cc.length > 0,
      showBcc: params.bcc.length > 0,
      attachments: params.attachments,
      isHtml: params.isHtml ?? true,
    }),

  initializeSignature: (identities) => set((state) => {
    if (!state.isOpen || state.signatureInitialized || identities.length === 0) return state;
    const identity = identities.find((i) => i.id === state.fromIdentityId)
      ?? identities.find((i) => i.is_default)
      ?? identities[0];
    return {
      fromIdentityId: identity.id,
      signatureHtml: identity.signature_html,
      signatureInitialized: true,
      // Drafts already contain the user's saved body; don't modify them on resume.
      body: !state.draftId && state.signatureEnabled
        ? injectSignature(state.body, identity.signature_html, state.isHtml)
        : state.body,
    };
  }),

  closeCompose: () => set({ isOpen: false }),

  setField: (field, value) => set({ [field]: value }),

  setShowCc: (show) => set({ showCc: show }),
  setShowBcc: (show) => set({ showBcc: show }),

  setDraftId: (id) => set({ draftId: id }),

  setFromIdentityId: (id) => set({ fromIdentityId: id }),

  setIsHtml: (v) => set({ isHtml: v }),

  setSignatureHtml: (html) => set({ signatureHtml: html }),

  setSignatureEnabled: (enabled) => set({ signatureEnabled: enabled }),

  addAttachments: (atts) =>
    set((state) => ({ attachments: [...state.attachments, ...atts] })),

  removeAttachment: (id) =>
    set((state) => ({
      attachments: state.attachments.filter((a) => a.id !== id),
    })),

  reset: () => set(initialState),
}));
