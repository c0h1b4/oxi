"use client";

import { apiAccountUrl } from "@/lib/api-url";
import { useAuthStore } from "@/stores/useAuthStore";

import { Fragment, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Dialog } from "radix-ui";
import { AnimatePresence, motion } from "framer-motion";
import { AnimatedDiv } from "@/lib/motion/AnimatedDiv";
import {
  Send,
  X,
  ChevronUp,
  AlertTriangle,
  Paperclip,
  Loader2,
  Save,
  Maximize2,
  Minimize2,
  Upload,
  FileText,
  Code,
  PenLine,
} from "lucide-react";
import { toast } from "sonner";
import {
  useComposeStore,
  replaceSignatureInBody,
  removeSignatureFromBody,
  injectSignature,
  wrapSignatureHtml,
  convertBodyFormat,
} from "@/stores/useComposeStore";
import {
  useSendMessage,
  useSaveDraft,
  useUploadAttachment,
  useDeleteAttachment,
  useDeleteDraft,
} from "@/hooks/useCompose";
import { useIdentities } from "@/hooks/useIdentities";
import { cn } from "@/lib/utils";
import { useUiStore } from "@/stores/useUiStore";
import { createFadeSlideVariants, createScaleFadeVariants } from "@/lib/motion/variants";
import {
  countRecipients,
  formatFileSize,
  stripHtml,
  generateId,
  inlineImagesToCid,
  DiscardAlertDialog,
  AttachmentPreviewDialog,
} from "./ComposeDialog/index";
import { RecipientInput } from "./ComposeDialog/RecipientInput";
import { SubjectInput } from "./ComposeDialog/SubjectInput";
import dynamic from "next/dynamic";

const RichTextEditor = dynamic(
  () => import("@/components/mail/RichTextEditor").then((mod) => mod.RichTextEditor),
  { ssr: false }
);

export function ComposeDialog() {
  const activeAccountId = useAuthStore((s) => s.activeAccountId);
  const {
    isOpen,
    to,
    cc,
    bcc,
    subject,
    body,
    inReplyTo,
    references,
    draftId,
    showCc,
    showBcc,
    attachments,
    fromIdentityId,
    isHtml,
    signatureHtml,
    signatureEnabled,
    signatureInitialized,
    closeCompose,
    setField,
    setShowCc,
    setShowBcc,
    setDraftId,
    setFromIdentityId,
    setIsHtml,
    setSignatureHtml,
    setSignatureEnabled,
    initializeSignature,
    addAttachments,
    removeAttachment,
    reset,
  } = useComposeStore();

  const sendMutation = useSendMessage();
  const saveDraftMutation = useSaveDraft();
  const uploadMutation = useUploadAttachment();
  const deleteMutation = useDeleteAttachment();
  const deleteDraftMutation = useDeleteDraft();
  const { data: identities } = useIdentities();
  const [draftSaved, setDraftSaved] = useState(false);
  const [showDiscardAlert, setShowDiscardAlert] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [previewAttId, setPreviewAttId] = useState<string | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const dragCounterRef = useRef(0);
  const toInputRef = useRef<HTMLTextAreaElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const lastSavedHashRef = useRef<string>("");
  const sendScheduledRef = useRef(false);
  const effectiveAnimationMode = useUiStore((s) => s.effectiveAnimationMode);
  const shouldAnimate = effectiveAnimationMode !== "off";
  const overlayMotionProps = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "y"), [effectiveAnimationMode]);
  const contentMotionProps = useMemo(() => createScaleFadeVariants(effectiveAnimationMode), [effectiveAnimationMode]);
  const sendFeedbackMotionProps = useMemo(() => createFadeSlideVariants(effectiveAnimationMode, "y"), [effectiveAnimationMode]);
  const serializedSendFeedbackMotionProps = useMemo(() => JSON.stringify(sendFeedbackMotionProps), [sendFeedbackMotionProps]);

  const hasContent = useCallback(() => {
    if (to.trim() || cc.trim() || bcc.trim() || subject.trim() || attachments.length > 0) {
      return true;
    }
    // An automatic signature alone does not make a message worth saving.
    if (!isHtml) return !!removeSignatureFromBody(body, false, signatureHtml).trim();
    const doc = new DOMParser().parseFromString(body, "text/html");
    const expected = new DOMParser().parseFromString(wrapSignatureHtml(signatureHtml), "text/html");
    const signature = expected.querySelector('[data-signature="true"]');
    if (signature) {
      // Tiptap drops the delimiter's trailing space. Be conservative about all
      // other differences: the editable wrapper may contain the user's message.
      const signatureContent = (element: Element) => {
        const copy = element.cloneNode(true) as Element;
        const delimiter = copy.firstElementChild;
        if (delimiter?.innerHTML === "-- ") delimiter.textContent = "--";
        return copy.innerHTML;
      };
      const unchangedContent = signatureContent(signature);
      doc.querySelectorAll('[data-signature="true"]').forEach((element) => {
        if (!element.closest("blockquote") && signatureContent(element) === unchangedContent) {
          element.remove();
        }
      });
    }
    return !!(doc.body.textContent?.trim() || doc.body.querySelector("img"));
  }, [to, cc, bcc, subject, body, attachments, isHtml, signatureHtml]);

  // Initialize once for each compose, including replies with a preselected identity.
  useEffect(() => {
    if (isOpen && identities) initializeSignature(identities);
  }, [isOpen, identities, fromIdentityId, signatureInitialized, initializeSignature]);

  // Auto-focus the To field when dialog opens
  useEffect(() => {
    if (isOpen) {
      const focusInput = () => toInputRef.current?.focus();
      const rafId = requestAnimationFrame(() => {
        requestAnimationFrame(focusInput);
      });
      return () => cancelAnimationFrame(rafId);
    }
  }, [isOpen]);

  // Compute a simple hash of compose fields for dirty tracking
  const computeHash = useCallback(() => {
    return `${to}|${cc}|${bcc}|${subject}|${body}`;
  }, [to, cc, bcc, subject, body]);

  // Save draft function. When force=true, skip the hash guard (used for explicit save/close).
  const saveDraft = useCallback(
    (force = false) => {
      if (sendScheduledRef.current) return;
      const hash = computeHash();
      // Don't save if nothing changed (unless forced) or compose is empty
      if (!force && hash === lastSavedHashRef.current) return;
      if (!hasContent()) return;

      let currentDraftId = useComposeStore.getState().draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      saveDraftMutation.mutate(
        {
          id: currentDraftId,
          to,
          cc,
          bcc,
          subject,
          textBody: isHtml ? stripHtml(body) : body,
          htmlBody: isHtml ? body : null,
          inReplyTo: inReplyTo,
          references: references,
        },
        {
          onSuccess: () => {
            lastSavedHashRef.current = hash;
            setDraftSaved(true);
            setTimeout(() => setDraftSaved(false), 3000);
            toast.success("Draft saved");
          },
        }
      );
    },
    [
      computeHash,
      hasContent,
      to,
      cc,
      bcc,
      subject,
      body,
      isHtml,
      setDraftId,
      inReplyTo,
      references,
      saveDraftMutation,
    ]
  );

  // Auto-save every 30s when dialog is open
  useEffect(() => {
    if (!isOpen) return;
    const interval = setInterval(() => {
      saveDraft();
    }, 30000);
    return () => clearInterval(interval);
  }, [isOpen, saveDraft]);

  // Reset saved hash when dialog opens
  useEffect(() => {
    if (isOpen) {
      lastSavedHashRef.current = computeHash();
    }
  }, [isOpen]); // eslint-disable-line react-hooks/exhaustive-deps

  const doSend = useCallback((sendingDraftId: string | null) => {
    let plainText: string;
    let sendHtml: string | null;
    if (isHtml) {
      plainText = stripHtml(body);
      // Convert preview URLs back to cid: references for the email MIME body
      sendHtml = inlineImagesToCid(body, sendingDraftId);
    } else {
      plainText = body;
      sendHtml = null;
    }
    sendMutation.mutate(
      {
        to,
        cc,
        bcc,
        subject,
        body: plainText,
        htmlBody: sendHtml,
        inReplyTo,
        references,
        draftId: sendingDraftId,
        fromIdentityId,
      },
      {
        onSuccess: () => {
          sendScheduledRef.current = false;
          toast.success("Message sent");
          reset();
        },
        onError: (error) => {
          sendScheduledRef.current = false;
          useComposeStore.setState({ isOpen: true });
          toast.error(`Failed to send: ${error.message}`);
        },
      }
    );
  }, [
    to,
    cc,
    bcc,
    subject,
    body,
    isHtml,
    inReplyTo,
    references,
    fromIdentityId,
    sendMutation,
    reset,
  ]);

  const handleSend = useCallback(() => {
    if (sendScheduledRef.current || (!to.trim() && !cc.trim() && !bcc.trim())) return;
    sendScheduledRef.current = true;
    const sendingDraftId = useComposeStore.getState().draftId;
    closeCompose();

    // 5-second undo window via sonner
    const timer = setTimeout(() => {
      toast.dismiss("undo-send");
      doSend(sendingDraftId);
    }, 5000);

    toast("Sending message...", {
      id: "undo-send",
      duration: 5500,
      action: {
        label: "Undo",
        onClick: () => {
          clearTimeout(timer);
          sendScheduledRef.current = false;
          useComposeStore.setState({ isOpen: true });
        },
      },
    });
  }, [to, cc, bcc, closeCompose, doSend]);

  const handleDiscard = useCallback(() => {
    if (hasContent()) {
      setShowDiscardAlert(true);
    } else {
      reset();
    }
  }, [hasContent, reset]);

  const confirmDiscard = useCallback(() => {
    setShowDiscardAlert(false);
    if (draftId) {
      deleteDraftMutation.mutate(draftId);
    }
    reset();
  }, [draftId, deleteDraftMutation, reset]);

  // Save draft and close the dialog without discarding.
  const handleSaveAndClose = useCallback(() => {
    if (hasContent()) {
      saveDraft(true);
    }
    closeCompose();
  }, [hasContent, saveDraft, closeCompose]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
        e.preventDefault();
        handleSend();
      }
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        saveDraft(true);
      }
    },
    [handleSend, saveDraft]
  );

  const handleAttachFiles = useCallback(() => {
    fileInputRef.current?.click();
  }, []);

  const handleFileSelected = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = e.target.files;
      if (!files || files.length === 0) return;

      // Ensure we have a draft ID for uploads
      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      uploadMutation.mutate(
        { draftId: currentDraftId, files: Array.from(files) },
        {
          onSuccess: (data) => {
            addAttachments(
              data.attachments.map((a) => ({
                id: a.id,
                filename: a.filename,
                contentType: a.content_type,
                size: a.size,
              }))
            );
          },
          onError: (error) => {
            toast.error(`Upload failed: ${error.message}`);
          },
        }
      );

      // Reset the input so the same file can be re-selected
      e.target.value = "";
    },
    [draftId, setDraftId, uploadMutation, addAttachments]
  );

  const handleImageUpload = useCallback(
    async (file: File): Promise<string | null> => {
      // Ensure we have a draft ID for the upload
      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      return new Promise((resolve) => {
        uploadMutation.mutate(
          { draftId: currentDraftId!, files: [file] },
          {
            onSuccess: (data) => {
              if (data.attachments.length > 0) {
                const att = data.attachments[0];
                addAttachments([
                  {
                    id: att.id,
                    filename: att.filename,
                    contentType: att.content_type,
                    size: att.size,
                  },
                ]);
                // Return a preview URL that the browser can render.
                // The send flow converts these back to cid: references.
                resolve(
                  apiAccountUrl(`/drafts/${currentDraftId}/attachments/${att.id}/content`, activeAccountId)
                );
              } else {
                resolve(null);
              }
            },
            onError: (error) => {
              toast.error(`Image upload failed: ${error.message}`);
              resolve(null);
            },
          }
        );
      });
    },
    [draftId, setDraftId, uploadMutation, addAttachments, activeAccountId]
  );

  const handleRemoveAttachment = useCallback(
    (attachmentId: string) => {
      if (!draftId) return;
      deleteMutation.mutate(
        { draftId, attachmentId },
        {
          onSuccess: () => {
            removeAttachment(attachmentId);
          },
          onError: (error) => {
            toast.error(`Delete failed: ${error.message}`);
          },
        }
      );
    },
    [draftId, deleteMutation, removeAttachment]
  );

  const handleDragEnter = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current += 1;
    if (e.dataTransfer.types.includes("Files")) {
      setIsDragging(true);
    }
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    dragCounterRef.current -= 1;
    if (dragCounterRef.current === 0) {
      setIsDragging(false);
    }
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
      dragCounterRef.current = 0;

      const files = Array.from(e.dataTransfer.files);
      if (files.length === 0) return;

      let currentDraftId = draftId;
      if (!currentDraftId) {
        currentDraftId = generateId();
        setDraftId(currentDraftId);
      }

      uploadMutation.mutate(
        { draftId: currentDraftId, files },
        {
          onSuccess: (data) => {
            addAttachments(
              data.attachments.map((a) => ({
                id: a.id,
                filename: a.filename,
                contentType: a.content_type,
                size: a.size,
              }))
            );
            toast.success(`${data.attachments.length} file(s) attached`);
          },
          onError: (error) => {
            toast.error(`Upload failed: ${error.message}`);
          },
        }
      );
    },
    [draftId, setDraftId, uploadMutation, addAttachments]
  );

  const handleToggleHtml = useCallback(() => {
    setField("body", convertBodyFormat(body, isHtml, signatureHtml));
    setIsHtml(!isHtml);
  }, [isHtml, body, signatureHtml, setField, setIsHtml]);

  const previewAttachment = previewAttId
    ? attachments.find((a) => a.id === previewAttId)
    : null;

  return (
    <>
      <Dialog.Root
        open={isOpen}
        onOpenChange={(open) => {
          if (!open) {
            handleSaveAndClose();
          }
        }}
      >
        <Dialog.Portal>
          <AnimatePresence>
            {isOpen ? (
              <Fragment key="compose-dialog-open">
                <Dialog.Overlay forceMount asChild>
                  <AnimatedDiv
                    data-testid="compose-dialog-overlay-transition"
                    variants={overlayMotionProps}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    className="fixed inset-0 z-40 bg-black/40"
                  />
                </Dialog.Overlay>
                <Dialog.Content forceMount asChild>
                  <AnimatedDiv
                    data-testid="compose-dialog-content-transition"
                    variants={contentMotionProps}
                    initial="initial"
                    animate="animate"
                    exit="exit"
                    className={cn(
                      "fixed z-50 flex min-h-0 min-w-0 flex-col overflow-hidden rounded-xl border border-border bg-background shadow-2xl",
                      expanded
                        ? "inset-2 max-h-[calc(100dvh-1rem)] sm:inset-4 sm:left-20 sm:max-h-[calc(100dvh-2rem)]"
                        : "inset-x-2 bottom-2 max-h-[calc(100dvh-1rem)] sm:inset-x-auto sm:bottom-8 sm:left-20 sm:max-h-[calc(100dvh-4rem)] sm:w-[calc(100%-6rem)] sm:max-w-2xl"
                    )}
                    onKeyDown={handleKeyDown}
                    onDragEnter={handleDragEnter}
                    onDragLeave={handleDragLeave}
                    onDragOver={handleDragOver}
                    onDrop={handleDrop}
                  >
            {/* Drop overlay */}
            {isDragging && (
              <div className="absolute inset-0 z-10 flex flex-col items-center justify-center rounded-xl bg-background/90 backdrop-blur-sm">
                <Upload className="size-10 text-primary" />
                <p className="mt-3 text-sm font-medium text-foreground">
                  Drop files to attach
                </p>
                <p className="mt-1 text-xs text-muted-foreground">
                  Files will be uploaded as attachments
                </p>
              </div>
            )}

            {/* Header */}
            <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
              <Dialog.Title className="text-sm font-semibold">
                New Message
              </Dialog.Title>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => setExpanded((e) => !e)}
                  className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                  title={expanded ? "Minimize" : "Maximize"}
                >
                  {expanded ? (
                    <Minimize2 className="size-4" />
                  ) : (
                    <Maximize2 className="size-4" />
                  )}
                </button>
                <Dialog.Close asChild>
                  <button
                    className="rounded-md p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
                    title="Close"
                  >
                    <X className="size-4" />
                  </button>
                </Dialog.Close>
              </div>
            </div>

            <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto overscroll-contain">
              {/* Fields */}
              <div className="flex shrink-0 flex-col border-b border-border">
                {identities && identities.length > 1 && (
                  <div className="flex items-center border-b border-border/50 px-4">
                    <label className="w-12 shrink-0 text-xs text-muted-foreground">
                      From
                    </label>
                    <select
                      value={fromIdentityId ?? ""}
                      onChange={(e) => {
                        const newId = e.target.value ? Number(e.target.value) : null;
                        setFromIdentityId(newId);
                        const newIdentity = identities.find((i) => i.id === newId);
                        const newSig = newIdentity?.signature_html || "";
                        setSignatureHtml(newSig);
                        if (signatureEnabled) {
                          setField("body", replaceSignatureInBody(body, newSig, isHtml, signatureHtml));
                        }
                      }}
                      className="min-w-0 flex-1 bg-transparent py-2 text-sm outline-none"
                    >
                      {identities.map((identity) => (
                        <option key={identity.id} value={identity.id}>
                          {identity.display_name
                            ? `${identity.display_name} <${identity.email}>`
                            : identity.email}
                        </option>
                      ))}
                    </select>
                  </div>
                )}
                <div className="flex items-center border-b border-border/50 px-4">
                  <label className="w-12 shrink-0 text-xs text-muted-foreground">
                    To
                  </label>
                  <RecipientInput
                    inputRef={toInputRef}
                    value={to}
                    onChange={(v) => setField("to", v)}
                    placeholder="Recipients"
                  />
                  <button
                    className="ml-2 shrink-0 whitespace-nowrap text-xs text-muted-foreground hover:text-foreground"
                    onClick={() => {
                      if (!showCc && !showBcc) {
                        setShowCc(true);
                      } else {
                        setShowCc(!showCc);
                        setShowBcc(!showBcc);
                      }
                    }}
                  >
                    {showCc || showBcc ? (
                      <ChevronUp className="size-3.5" />
                    ) : (
                      <span>Cc Bcc</span>
                    )}
                  </button>
                </div>

                {showCc && (
                  <div className="flex items-center border-b border-border/50 px-4">
                    <label className="w-12 shrink-0 text-xs text-muted-foreground">
                      Cc
                    </label>
                    <RecipientInput
                      value={cc}
                      onChange={(v) => setField("cc", v)}
                    />
                  </div>
                )}

                {showBcc && (
                  <div className="flex items-center border-b border-border/50 px-4">
                    <label className="w-12 shrink-0 text-xs text-muted-foreground">
                      Bcc
                    </label>
                    <RecipientInput
                      value={bcc}
                      onChange={(v) => setField("bcc", v)}
                    />
                  </div>
                )}

                <div className="flex items-center px-4">
                  <label className="w-12 shrink-0 text-xs text-muted-foreground">
                    Subject
                  </label>
                  <SubjectInput
                    value={subject}
                    onChange={(value) => setField("subject", value)}
                    enabled={!inReplyTo}
                  />
                </div>
              </div>

              {/* Recipient count warning */}
              {countRecipients(to, cc, bcc) > 10 && (
                <div className="flex shrink-0 items-center gap-2 border-b border-yellow-300/50 bg-yellow-50 px-4 py-2 dark:border-yellow-700/50 dark:bg-yellow-950/30">
                  <AlertTriangle className="size-4 shrink-0 text-yellow-600 dark:text-yellow-500" />
                  <span className="text-xs text-yellow-700 dark:text-yellow-400">
                    You are sending to more than 10 recipients.
                  </span>
                </div>
              )}

              {/* Attachments */}
              {attachments.length > 0 && (
                <div className="flex shrink-0 flex-wrap gap-2 border-b border-border px-4 py-2">
                  {attachments.map((att) => (
                    <div
                      key={att.id}
                      className="flex min-w-0 max-w-full items-center gap-1.5 rounded-md border border-border bg-accent/50 px-2 py-1 text-xs"
                    >
                      <button
                        onClick={() => setPreviewAttId(att.id)}
                        className="flex min-w-0 items-center gap-1.5 hover:text-foreground"
                        title="Preview"
                      >
                        <Paperclip className="size-3 shrink-0 text-muted-foreground" />
                        <span
                          className="max-w-[150px] truncate"
                          title={att.filename}
                        >
                          {att.filename}
                        </span>
                        <span className="shrink-0 text-muted-foreground">
                          ({formatFileSize(att.size)})
                        </span>
                      </button>
                      <button
                        onClick={() => handleRemoveAttachment(att.id)}
                        className="ml-0.5 shrink-0 rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground"
                        title="Remove attachment"
                      >
                        <X className="size-3" />
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Body */}
              {isHtml ? (
                <RichTextEditor
                  content={body}
                  onChange={(html) => setField("body", html)}
                  onImageUpload={handleImageUpload}
                  placeholder="Write your message..."
                  className="min-h-60 flex-1 shrink-0"
                />
              ) : (
                <textarea
                  value={body}
                  onChange={(e) => setField("body", e.target.value)}
                  placeholder="Write your message..."
                  className="min-h-60 w-full min-w-0 flex-1 shrink-0 resize-none overflow-auto bg-transparent px-4 py-3 text-sm outline-none placeholder:text-muted-foreground/50"
                />
              )}
            </div>

            {/* Footer */}
            <div className="flex shrink-0 flex-wrap items-center justify-between gap-2 border-t border-border px-3 py-2 sm:px-4 sm:py-3">
              <div className="flex min-w-0 flex-wrap items-center gap-1 sm:gap-2">
                <button
                  onClick={handleSend}
                  disabled={
                    sendMutation.isPending ||
                    (!to.trim() && !cc.trim() && !bcc.trim())
                  }
                  className={cn(
                    "inline-flex items-center gap-2 rounded-lg px-4 py-2 text-sm font-medium transition-colors",
                    "bg-primary text-primary-foreground hover:bg-primary/90",
                    "disabled:cursor-not-allowed disabled:opacity-50"
                  )}
                >
                  {shouldAnimate ? (
                    <AnimatePresence mode="wait" initial={false}>
                      <motion.span
                        key={sendMutation.isPending ? "pending" : "idle"}
                        data-testid="compose-send-feedback-transition"
                        data-motion-props={serializedSendFeedbackMotionProps}
                        initial="initial"
                        animate="animate"
                        exit="exit"
                        variants={sendFeedbackMotionProps}
                        className="inline-flex items-center gap-2"
                      >
                        {sendMutation.isPending ? (
                          <>
                            <Loader2 className="size-4 animate-spin" />
                            Sending...
                          </>
                        ) : (
                          <>
                            <Send className="size-4" />
                            Send
                          </>
                        )}
                      </motion.span>
                    </AnimatePresence>
                  ) : sendMutation.isPending ? (
                    <>
                      <Loader2 className="size-4 animate-spin" />
                      Sending...
                    </>
                  ) : (
                    <>
                      <Send className="size-4" />
                      Send
                    </>
                  )}
                </button>
                <button
                  onClick={handleAttachFiles}
                  disabled={uploadMutation.isPending}
                  className="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50"
                  title="Attach files"
                >
                  {uploadMutation.isPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Paperclip className="size-4" />
                  )}
                </button>
                <button
                  onClick={() => saveDraft(true)}
                  disabled={saveDraftMutation.isPending}
                  className="rounded-lg p-2 text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50"
                  title="Save draft (Ctrl+S)"
                >
                  {saveDraftMutation.isPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Save className="size-4" />
                  )}
                </button>
                <button
                  onClick={handleToggleHtml}
                  className={cn(
                    "rounded-lg p-2 hover:bg-accent hover:text-foreground",
                    isHtml ? "text-muted-foreground" : "text-primary"
                  )}
                  title={isHtml ? "Switch to plain text" : "Switch to rich text"}
                >
                  {isHtml ? (
                    <FileText className="size-4" />
                  ) : (
                    <Code className="size-4" />
                  )}
                </button>
                <button
                  onClick={() => {
                    const newEnabled = !signatureEnabled;
                    setSignatureEnabled(newEnabled);
                    if (newEnabled) {
                      // Re-inject signature
                      if (signatureHtml) {
                        setField("body", injectSignature(removeSignatureFromBody(body, isHtml, signatureHtml), signatureHtml, isHtml));
                      }
                    } else {
                      // Remove signature from body
                      setField("body", removeSignatureFromBody(body, isHtml, signatureHtml));
                    }
                  }}
                  className={cn(
                    "rounded-lg p-2 hover:bg-accent hover:text-foreground",
                    signatureEnabled ? "text-primary" : "text-muted-foreground"
                  )}
                  title={signatureEnabled ? "Remove signature" : "Add signature"}
                >
                  <PenLine className="size-4" />
                </button>
                <input
                  ref={fileInputRef}
                  type="file"
                  multiple
                  className="hidden"
                  onChange={handleFileSelected}
                />
              </div>
              <div className="flex items-center gap-2">
                {draftSaved && (
                  <span className="text-xs text-muted-foreground">
                    Draft saved
                  </span>
                )}
                <button
                  onClick={handleDiscard}
                  className="rounded-lg px-3 py-2 text-sm text-muted-foreground hover:bg-accent hover:text-foreground"
                >
                  Discard
                </button>
              </div>
            </div>
                  </AnimatedDiv>
                </Dialog.Content>
              </Fragment>
            ) : null}
          </AnimatePresence>
        </Dialog.Portal>
      </Dialog.Root>

      <DiscardAlertDialog
        open={showDiscardAlert}
        onOpenChange={setShowDiscardAlert}
        onConfirm={confirmDiscard}
      />

      {/* Attachment preview dialog */}
      {previewAttachment && draftId && (
        <AttachmentPreviewDialog
          attachment={{
            id: previewAttachment.id,
            filename: previewAttachment.filename,
            contentType: previewAttachment.contentType,
            size: previewAttachment.size,
          }}
          previewUrl={apiAccountUrl(`/drafts/${draftId}/attachments/${previewAttachment.id}/content`, activeAccountId)}
          onClose={() => setPreviewAttId(null)}
        />
      )}
    </>
  );
}
