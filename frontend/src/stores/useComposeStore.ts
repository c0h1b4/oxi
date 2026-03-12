"use client";

import { create } from "zustand";
import { useUiStore } from "@/stores/useUiStore";

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

/** Injects a signature block into body HTML, before any quoted content or at the end. */
export function injectSignature(body: string, signatureHtml: string): string {
  const sigBlock = wrapSignatureHtml(signatureHtml);
  if (!sigBlock) return body;

  // Try to insert before blockquote (quoted reply/forward content)
  const bqIndex = body.indexOf("<blockquote");
  if (bqIndex !== -1) {
    return body.slice(0, bqIndex) + sigBlock + body.slice(bqIndex);
  }
  return body + sigBlock;
}

/** Replaces or removes the signature block in the body HTML. */
export function replaceSignatureInBody(body: string, newSignatureHtml: string): string {
  const sigRegex = /<div class="email-signature"[^>]*>[\s\S]*?<\/div>/;
  const newSigBlock = wrapSignatureHtml(newSignatureHtml);

  if (sigRegex.test(body)) {
    if (newSigBlock) {
      return body.replace(sigRegex, newSigBlock);
    }
    // Remove the signature block
    return body.replace(sigRegex, "");
  }

  // No existing signature — inject if we have one
  if (newSigBlock) {
    return injectSignature(body, newSignatureHtml);
  }
  return body;
}

/** Removes the signature block from the body HTML. */
export function removeSignatureFromBody(body: string): string {
  return body.replace(/<div class="email-signature"[^>]*>[\s\S]*?<\/div>/, "");
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
